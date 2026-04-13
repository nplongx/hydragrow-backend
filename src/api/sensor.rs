use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;
use serde_json::json;
use tracing::{error, instrument};

use crate::AppState;
use crate::db::influx::get_latest_sensor_data;
use crate::models::sensor::SensorDataRow;

#[derive(Deserialize, Debug)]
pub struct HistoryQuery {
    pub range: Option<String>,
}

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
        Ok(data) => HttpResponse::Ok().json(json!({ "status": "success", "data": data })),
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

    // 🟢 ĐÃ THÊM HÀM PIVOT. Nếu không có hàm này, InfluxDB Cloud sẽ trả về dữ liệu rải rác từng trường.
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

    tracing::info!("Câu lệnh Flux Query:\n{}", flux_query);
    let query_obj = influxdb2::models::Query::new(flux_query.clone());

    match app_state
        .influx_client
        .query::<SensorDataRow>(Some(query_obj))
        .await
    {
        Ok(tables) => {
            tracing::info!("Query thành công! Trả về {} bản ghi.", tables.len());
            HttpResponse::Ok().json(json!({ "status": "success", "data": tables }))
        }
        Err(e) => {
            tracing::error!("Lỗi khi query từ InfluxDB Cloud cho {}: {:?}", device_id, e);
            HttpResponse::InternalServerError().json(json!({
                "error": "Database Error",
                "message": "Không thể truy xuất dữ liệu lịch sử"
            }))
        }
    }
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/latest", web::get().to(get_latest))
        .route("/history", web::get().to(get_history));
}
