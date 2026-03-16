use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, instrument};

use crate::AppState;
use crate::db::influx::get_latest_sensor_data;
use crate::models::sensor::SensorDataRow;

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

    // LƯU Ý 1: Đã thêm dấu trừ (-) trước {} ở hàm range để lấy quá khứ (VD: -24h)
    // LƯU Ý 2: Bạn nên kiểm tra lại dòng `limit(n: 1)`. Nếu đây là API biểu đồ thì nên bỏ dòng đó đi,
    // hoặc dùng `aggregateWindow` nếu data quá lớn.
    let flux_query = format!(
        r#"
        from(bucket: "{}")
        |> range(start: -{}) 
        |> filter(fn: (r) => r["_measurement"] == "sensor_data")
        |> filter(fn: (r) => r.device_id == "{}")
        |> sort(columns: ["_time"], desc: true)
        |> limit(n: 100)     
        "#,
        app_state.influx_bucket, range, device_id
    );

    // [DEBUG 2] In ra nguyên văn câu lệnh Flux.
    // Mẹo: Bạn có thể copy câu này paste thẳng vào Data Explorer của InfluxDB UI để test chạy thử.
    tracing::info!("Câu lệnh Flux Query được tạo ra:\n{}", flux_query);

    let query_obj = influxdb2::models::Query::new(flux_query.clone());

    match app_state
        .influx_client
        .query::<SensorDataRow>(Some(query_obj))
        .await
    {
        Ok(tables) => {
            // [DEBUG 3] In ra số lượng bản ghi trả về
            tracing::info!("Query thành công! Trả về {} bản ghi.", tables.len());

            // [DEBUG 4] In ra dữ liệu thô (nếu data quá nhiều, bạn có thể cân nhắc comment dòng này lại sau khi fix xong)
            tracing::debug!("Dữ liệu thô từ InfluxDB: {:#?}", tables);

            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "data": tables
            }))
        }
        Err(e) => {
            // [DEBUG 5] Ghi log lỗi kèm theo câu lệnh gây lỗi để dễ debug
            tracing::error!(
                "Lỗi khi query biểu đồ từ InfluxDB cho {}: {:?}",
                device_id,
                e
            );
            tracing::error!("Câu lệnh Flux gây lỗi:\n{}", flux_query);

            HttpResponse::InternalServerError().json(serde_json::json!({
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
