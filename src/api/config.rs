use actix_web::{HttpResponse, Responder, web};
use rumqttc::QoS;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::AppState;
use crate::models::config::DeviceConfig;

/// Lấy cấu hình hiện tại của thiết bị từ SQLite
#[instrument(skip(app_state))]
pub async fn get_config(
    path: web::Path<String>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();

    // Tận dụng hàm get_device_config đã viết ở src/db/sqlite.rs
    match crate::db::sqlite::get_device_config(&app_state.sqlite_pool, &device_id).await {
        Ok(config) => HttpResponse::Ok().json(config),
        Err(e) => {
            tracing::warn!("Config not found or DB error: {:?}", e);
            HttpResponse::NotFound()
                .json(json!({"error": "Configuration not found for this device"}))
        }
    }
}

/// Cập nhật cấu hình vào SQLite và đồng bộ xuống ESP32 qua MQTT
#[instrument(skip(app_state, payload))]
pub async fn update_config(
    path: web::Path<String>,
    payload: web::Json<DeviceConfig>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut config = payload.into_inner();

    // Đảm bảo device_id trong payload khớp với URL parameter để tránh ghi nhầm
    if config.device_id != device_id {
        config.device_id = device_id.clone();
    }

    // Cập nhật timestamp mới nhất
    // Cập nhật timestamp mới nhất
    config.last_updated = chrono::Utc::now().to_rfc3339();

    // 1. Lưu vào SQLite
    if let Err(e) = crate::db::sqlite::upsert_device_config(&app_state.sqlite_pool, &config).await {
        error!("Failed to update config in DB: {:?}", e);
        return HttpResponse::InternalServerError()
            .json(json!({"error": "Failed to save configuration"}));
    }

    // 2. Publish cấu hình mới xuống ESP32 qua MQTT
    let mqtt_topic = format!("hydro/{}/config", device_id);
    let mqtt_payload = match serde_json::to_vec(&config) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to serialize config for MQTT: {:?}", e);
            return HttpResponse::InternalServerError()
                .json(json!({"error": "Failed to serialize payload"}));
        }
    };

    // QoS::AtLeastOnce (QoS 1) đảm bảo message chắc chắn đến được Broker
    // retain = true (Gắn cờ Retain): Giúp ESP32 nhận lại config ngay lập tức nếu nó vừa khởi động lại và subscribe vào topic này.
    let publish_result = app_state
        .mqtt_client
        .publish(&mqtt_topic, QoS::AtLeastOnce, true, mqtt_payload)
        .await;

    if let Err(e) = publish_result {
        error!("Failed to publish config to MQTT: {:?}", e);
        // Trả về Multi-Status hoặc Partial Success báo cho Frontend biết DB đã lưu nhưng thiết bị có thể chưa nhận được ngay
        return HttpResponse::Accepted().json(json!({
            "status": "partial_success",
            "message": "Configuration saved to database, but failed to sync to device immediately.",
            "config": config
        }));
    }

    info!("Configuration updated and synced to device {}", device_id);

    // 3. Trả về cấu hình mới nhất cho Frontend
    HttpResponse::Ok().json(json!({
        "status": "success",
        "message": "Configuration updated and synced",
        "config": config
    }))
}

// Khởi tạo các routes cho module config
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/devices")
            .route("/{device_id}/config", web::get().to(get_config))
            .route("/{device_id}/config", web::put().to(update_config)),
    );
}
