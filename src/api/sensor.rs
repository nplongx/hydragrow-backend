use crate::AppState;
use actix_web::{HttpResponse, Responder, web};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub start: Option<String>, // RFC3339
    pub end: Option<String>,
    pub limit: Option<usize>,
}

pub async fn get_latest(
    path: web::Path<String>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();

    // lấy dữ liệu trong 24h gần nhất
    let end = Utc::now().to_rfc3339();
    let start = (Utc::now() - Duration::hours(24)).to_rfc3339();

    match crate::db::influx::get_sensor_history(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &device_id,
        &start,
        &end,
        1, // chỉ lấy record mới nhất
    )
    .await
    {
        Ok(mut history) => {
            if let Some(data) = history.pop() {
                HttpResponse::Ok().json(data)
            } else {
                HttpResponse::NotFound()
                    .json(json!({"error": "No sensor data found for this device"}))
            }
        }

        Err(e) => {
            tracing::error!("Database query error: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": "Database error"}))
        }
    }
}

pub async fn get_history(
    path: web::Path<String>,
    query: web::Query<HistoryQuery>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();

    // mặc định 24h
    let end = query.end.clone().unwrap_or_else(|| Utc::now().to_rfc3339());

    let start = query
        .start
        .clone()
        .unwrap_or_else(|| (Utc::now() - Duration::days(1)).to_rfc3339());

    // giới hạn để tránh payload quá lớn
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);

    match crate::db::influx::get_sensor_history(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &device_id,
        &start,
        &end,
        limit,
    )
    .await
    {
        Ok(history) => HttpResponse::Ok().json(history),

        Err(e) => {
            tracing::error!("Failed to fetch history: {:?}", e);
            HttpResponse::InternalServerError()
                .json(json!({"error": "Failed to fetch historical data"}))
        }
    }
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/sensors")
            .route("/{device_id}/latest", web::get().to(get_latest))
            .route("/{device_id}/history", web::get().to(get_history)),
    );
}

