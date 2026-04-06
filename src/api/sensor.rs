use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;
use serde_json::json;
use tracing::{error, instrument};

use crate::AppState;
use crate::db::influx::get_latest_sensor_data;

/// Tham số Query String cho API lịch sử (VD: ?range=24h)
#[derive(Deserialize, Debug)]
pub struct HistoryQuery {
    pub range: Option<String>,
}

/// GET /api/devices/{device_id}/sensors/latest
/// Lấy trạng thái và thông số cảm biến mới nhất của thiết bị
#[instrument(skip(app_state))]
pub async fn get_latest(path: web::Path<String>, app_state: web::Data<AppState>) -> impl Responder {
    let device_id = path.into_inner();

    match get_latest_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &device_id,
    )
    .await
    {
        Ok(data) => HttpResponse::Ok().json(json!({
            "status": "success",
            "data": data
        })),
        Err(e) => {
            error!(
                "Lỗi khi lấy dữ liệu sensor mới nhất cho {}: {:?}",
                device_id, e
            );
            HttpResponse::NotFound().json(json!({
                "error": "Not Found",
                "message": "Không tìm thấy dữ liệu cho thiết bị này"
            }))
        }
    }
}

#[instrument(skip(app_state))]
pub async fn get_history(
    path: web::Path<String>,
    query: web::Query<HistoryQuery>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let range = query.range.clone().unwrap_or_else(|| "24h".to_string());

    // [DEBUG 1] Ghi log thông số đầu vào
    tracing::info!(
        "Bắt đầu lấy lịch sử cho thiết bị: {}, range: {}",
        device_id,
        range
    );

    // Chuyển sang cú pháp InfluxQL (InfluxDB v1.x)
    // Lưu ý: time >= now() - 24h là chuẩn của InfluxQL
    let influx_query = format!(
        "SELECT * FROM sensor_data WHERE device_id = '{}' AND time >= now() - {} ORDER BY time DESC LIMIT 100",
        device_id, range
    );

    // [DEBUG 2] In ra nguyên văn câu lệnh InfluxQL
    tracing::info!("Câu lệnh InfluxQL được tạo ra:\n{}", influx_query);

    let read_query = influxdb::ReadQuery::new(influx_query.clone());

    match app_state.influx_client.query(read_query).await {
        Ok(query_result_str) => {
            // Parse chuỗi JSON trả về từ thư viện influxdb v0.8.0
            match serde_json::from_str::<serde_json::Value>(&query_result_str) {
                Ok(parsed) => {
                    let mut history_data = Vec::new();

                    // Cấu trúc InfluxDB trả về: parsed["results"][0]["series"][0]
                    if let Some(series_arr) = parsed["results"][0]["series"].as_array() {
                        if let Some(first_series) = series_arr.first() {
                            if let (Some(columns), Some(values_arr)) = (
                                first_series["columns"].as_array(),
                                first_series["values"].as_array(),
                            ) {
                                for val_row in values_arr {
                                    if let Some(row_values) = val_row.as_array() {
                                        let mut obj = serde_json::Map::new();

                                        for (i, col) in columns.iter().enumerate() {
                                            if let Some(col_name) = col.as_str() {
                                                let col_val = row_values[i].clone();

                                                // Nếu là pump_status, parse ngược lại thành JSON object để API trả về đẹp hơn
                                                if col_name == "pump_status" {
                                                    if let Some(ps_str) = col_val.as_str() {
                                                        if let Ok(ps_obj) = serde_json::from_str::<
                                                            serde_json::Value,
                                                        >(
                                                            ps_str
                                                        ) {
                                                            obj.insert(
                                                                col_name.to_string(),
                                                                ps_obj,
                                                            );
                                                            continue;
                                                        }
                                                    }
                                                }

                                                obj.insert(col_name.to_string(), col_val);
                                            }
                                        }
                                        history_data.push(serde_json::Value::Object(obj));
                                    }
                                }
                            }
                        }
                    }

                    // [DEBUG 3] In ra số lượng bản ghi trả về
                    tracing::info!("Query thành công! Trả về {} bản ghi.", history_data.len());

                    HttpResponse::Ok().json(json!({
                        "status": "success",
                        "data": history_data
                    }))
                }
                Err(e) => {
                    tracing::error!("Lỗi Parse JSON từ InfluxDB: {:?}", e);
                    HttpResponse::InternalServerError().json(json!({
                        "error": "Parse Error",
                        "message": "Không thể xử lý dữ liệu trả về từ Database"
                    }))
                }
            }
        }
        Err(e) => {
            // [DEBUG 4] Ghi log lỗi kèm theo câu lệnh gây lỗi
            tracing::error!(
                "Lỗi khi query biểu đồ từ InfluxDB cho {}: {:?}",
                device_id,
                e
            );
            tracing::error!("Câu lệnh InfluxQL gây lỗi:\n{}", influx_query);

            HttpResponse::InternalServerError().json(json!({
                "error": "Database Error",
                "message": "Không thể truy xuất dữ liệu lịch sử"
            }))
        }
    }
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/devices/{device_id}/sensors")
            .route("/latest", web::get().to(get_latest))
            .route("/history", web::get().to(get_history)),
    );
}

