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
pub async fn get_latest(
    path: web::Path<String>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
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

/// GET /api/devices/{device_id}/sensors/history
/// Lấy dữ liệu lịch sử để vẽ biểu đồ trên Frontend
#[instrument(skip(app_state))]
pub async fn get_history(
    path: web::Path<String>,
    query: web::Query<HistoryQuery>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();
    // Mặc định lấy 24h qua nếu user không truyền param
    let range = query.range.clone().unwrap_or_else(|| "24h".to_string());

    // CHÚ Ý TỐI ƯU (Downsampling):
    // Thiết bị gửi 1s/lần -> 1 ngày có 86,400 điểm. Nếu trả hết về Frontend, trình duyệt sẽ bị treo (crash).
    // Ta dùng aggregateWindow(every: 5m, fn: mean) để tính trung bình mỗi 5 phút thành 1 điểm.
    let flux_query = format!(
        r#"
        from(bucket: "{}")
            |> range(start: -{})
            |> filter(fn: (r) => r["_measurement"] == "sensor_data" and r["device_id"] == "{}")
            |> filter(fn: (r) => r["_field"] == "ec_value" or r["_field"] == "ph_value" or r["_field"] == "temp_value" or r["_field"] == "water_level")
            |> aggregateWindow(every: 5m, fn: mean, createEmpty: false)
            |> yield(name: "mean")
        "#,
        app_state.influx_bucket, range, device_id
    );

    let query_obj = influxdb2::models::Query::new(flux_query);

    match app_state
        .influx_client
        .query::<SensorDataRow>(Some(query_obj))
        .await
    {
        Ok(tables) => {
            // Trả raw data từ InfluxDB về.
            // Ở Frontend, bạn sẽ lặp qua mảng này để map vào thư viện vẽ Chart (như Recharts, Chart.js).
            HttpResponse::Ok().json(json!({
                "status": "success",
                "data": tables
            }))
        }
        Err(e) => {
            error!(
                "Lỗi khi query biểu đồ từ InfluxDB cho {}: {:?}",
                device_id, e
            );
            HttpResponse::InternalServerError().json(json!({
                "error": "Database Error",
                "message": "Không thể truy xuất dữ liệu lịch sử"
            }))
        }
    }
}

// Hàm khởi tạo để nhúng vào main.rs
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/devices/{device_id}/sensors")
            .route("/latest", web::get().to(get_latest))
            .route("/history", web::get().to(get_history)),
    );
}

