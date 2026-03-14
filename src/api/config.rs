use actix_web::{HttpResponse, Responder, web};
use chrono::Utc;
use rumqttc::QoS;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::AppState;
use crate::models::config::{
    DeviceConfig, DosingCalibration, SafetyConfig, SensorCalibration, WaterConfig,
};

// ─── API: Device Config ──────────────────────────────────────────────

#[instrument(skip(app_state, req))]
pub async fn update_device_config(
    path: web::Path<String>,
    req: web::Json<DeviceConfig>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let config = req.into_inner();
    let now = chrono::Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO device_config (
            device_id, ec_target, ec_tolerance, ph_target, ph_tolerance,
            temp_target, temp_tolerance, control_mode, is_enabled, last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            ec_target = excluded.ec_target, ec_tolerance = excluded.ec_tolerance,
            ph_target = excluded.ph_target, ph_tolerance = excluded.ph_tolerance,
            temp_target = excluded.temp_target, temp_tolerance = excluded.temp_tolerance,
            control_mode = excluded.control_mode, is_enabled = excluded.is_enabled,
            last_updated = excluded.last_updated
        "#,
        device_id,
        config.ec_target,
        config.ec_tolerance,
        config.ph_target,
        config.ph_tolerance,
        config.temp_target,
        config.temp_tolerance,
        config.control_mode,
        config.is_enabled,
        now
    )
    .execute(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(_) => HttpResponse::Ok().json(json!({"status": "success"})),
        Err(e) => {
            error!("Lỗi lưu DeviceConfig: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": "DB Error"}))
        }
    }
}

// ─── API: Safety Config ──────────────────────────────────────────────

#[instrument(skip(app_state, req))]
pub async fn update_safety_config(
    path: web::Path<String>,
    req: web::Json<SafetyConfig>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let config = req.into_inner();
    let now = chrono::Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO safety_config (
            device_id, max_ec_limit, min_ec_limit, min_ph_limit, max_ph_limit, max_ec_delta, max_ph_delta,
            max_dose_per_cycle, cooldown_sec, max_dose_per_hour, water_level_critical_min,
            max_refill_cycles_per_hour, max_drain_cycles_per_hour, max_refill_duration_sec,
            max_drain_duration_sec, max_temp_limit, min_temp_limit, emergency_shutdown, last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            max_ec_limit = excluded.max_ec_limit, min_ec_limit = excluded.min_ec_limit, min_ph_limit = excluded.min_ph_limit,
            max_ph_limit = excluded.max_ph_limit, max_ec_delta = excluded.max_ec_delta,
            max_ph_delta = excluded.max_ph_delta, max_dose_per_cycle = excluded.max_dose_per_cycle,
            cooldown_sec = excluded.cooldown_sec, max_dose_per_hour = excluded.max_dose_per_hour,
            water_level_critical_min = excluded.water_level_critical_min,
            max_refill_cycles_per_hour = excluded.max_refill_cycles_per_hour,
            max_drain_cycles_per_hour = excluded.max_drain_cycles_per_hour,
            max_refill_duration_sec = excluded.max_refill_duration_sec,
            max_drain_duration_sec = excluded.max_drain_duration_sec,
            max_temp_limit = excluded.max_temp_limit,
            min_temp_limit = excluded.min_temp_limit,
            emergency_shutdown = excluded.emergency_shutdown, last_updated = excluded.last_updated
        "#,
        device_id,
        config.max_ec_limit,
        config.min_ec_limit,
        config.min_ph_limit,
        config.max_ph_limit,
        config.max_ec_delta,
        config.max_ph_delta,
        config.max_dose_per_cycle,
        config.cooldown_sec,
        config.max_dose_per_hour,
        config.water_level_critical_min,
        config.max_refill_cycles_per_hour,
        config.max_drain_cycles_per_hour,
        config.max_refill_duration_sec,
        config.max_drain_duration_sec,
        config.max_temp_limit,
        config.min_temp_limit,
        config.emergency_shutdown,
        now
    )
    .execute(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(_) => HttpResponse::Ok().json(json!({"status": "success"})),
        Err(e) => {
            error!("Lỗi lưu SafetyConfig: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": "DB Error"}))
        }
    }
}

// ─── API: Sensor Calibration ──────────────────────────────────────────

#[instrument(skip(app_state, req))]
pub async fn update_sensor_calibration(
    path: web::Path<String>,
    req: web::Json<SensorCalibration>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let cal = req.into_inner();
    let now = chrono::Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO sensor_calibration (
            device_id, ph_v7, ph_v4, ec_factor, temp_offset, last_calibrated
        ) VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            ph_v7 = excluded.ph_v7, ph_v4 = excluded.ph_v4,
            ec_factor = excluded.ec_factor, temp_offset = excluded.temp_offset,
            last_calibrated = excluded.last_calibrated
        "#,
        device_id,
        cal.ph_v7,
        cal.ph_v4,
        cal.ec_factor,
        cal.temp_offset,
        now
    )
    .execute(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(_) => HttpResponse::Ok().json(json!({"status": "success"})),
        Err(e) => {
            error!("Lỗi lưu SensorCalibration: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": "DB Error"}))
        }
    }
}

/// Lấy cấu hình hiện tại của thiết bị từ SQLite
#[instrument(skip(app_state))]
pub async fn get_config(path: web::Path<String>, app_state: web::Data<AppState>) -> impl Responder {
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
    app_state: web::Data<AppState>,
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

// ─── API: Water Config ──────────────────────────────────────────────

/// GET /api/devices/{device_id}/config/water
#[instrument(skip(app_state))]
pub async fn get_water_config(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();

    let result = sqlx::query_as!(
        WaterConfig,
        r#"
        SELECT 
            device_id, water_level_min, water_level_target, water_level_max, 
            water_level_drain, circulation_mode, circulation_on_sec, 
            circulation_off_sec, last_updated
        FROM water_config 
        WHERE device_id = ?
        "#,
        device_id
    )
    .fetch_optional(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Config not found"})),
        Err(e) => {
            error!("Lỗi query WaterConfig cho {}: {:?}", device_id, e);
            HttpResponse::InternalServerError().json(json!({"error": "Database error"}))
        }
    }
}

/// POST /api/devices/{device_id}/config/water
#[instrument(skip(app_state, req))]
pub async fn update_water_config(
    path: web::Path<String>,
    req: web::Json<WaterConfig>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let config = req.into_inner();
    let now = Utc::now().to_rfc3339();

    // Dùng UPSERT: Nếu chưa có thì Insert, có rồi thì Update
    let result = sqlx::query!(
        r#"
        INSERT INTO water_config (
            device_id, water_level_min, water_level_target, water_level_max,
            water_level_drain, circulation_mode, circulation_on_sec,
            circulation_off_sec, last_updated
        ) 
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            water_level_min = excluded.water_level_min,
            water_level_target = excluded.water_level_target,
            water_level_max = excluded.water_level_max,
            water_level_drain = excluded.water_level_drain,
            circulation_mode = excluded.circulation_mode,
            circulation_on_sec = excluded.circulation_on_sec,
            circulation_off_sec = excluded.circulation_off_sec,
            last_updated = excluded.last_updated
        "#,
        device_id,
        config.water_level_min,
        config.water_level_target,
        config.water_level_max,
        config.water_level_drain,
        config.circulation_mode,
        config.circulation_on_sec,
        config.circulation_off_sec,
        now
    )
    .execute(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(_) => {
            info!("Đã cập nhật WaterConfig cho thiết bị {}", device_id);
            // Tùy chọn: Ở đây bạn có thể gọi MQTT Client để publish cấu hình mới xuống ESP32 ngay lập tức
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(e) => {
            error!("Lỗi lưu WaterConfig cho {}: {:?}", device_id, e);
            HttpResponse::InternalServerError().json(json!({"error": "Không thể lưu cấu hình"}))
        }
    }
}

// ─── API: Dosing Calibration ────────────────────────────────────────

/// GET /api/devices/{device_id}/calibration/dosing
#[instrument(skip(app_state))]
pub async fn get_dosing_calibration(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();

    let result = sqlx::query_as!(
        DosingCalibration,
        r#"
        SELECT 
            device_id, tank_volume_l, ec_gain_per_ml, ph_shift_up_per_ml, 
            ph_shift_down_per_ml, mixing_delay_sec, ec_step_ratio, 
            ph_step_ratio, last_calibrated
        FROM dosing_calibration 
        WHERE device_id = ?
        "#,
        device_id
    )
    .fetch_optional(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(Some(cal)) => HttpResponse::Ok().json(cal),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Calibration not found"})),
        Err(e) => {
            error!("Lỗi query DosingCalibration cho {}: {:?}", device_id, e);
            HttpResponse::InternalServerError().json(json!({"error": "Database error"}))
        }
    }
}

/// POST /api/devices/{device_id}/calibration/dosing
#[instrument(skip(app_state, req))]
pub async fn update_dosing_calibration(
    path: web::Path<String>,
    req: web::Json<DosingCalibration>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let cal = req.into_inner();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO dosing_calibration (
            device_id, tank_volume_l, ec_gain_per_ml, ph_shift_up_per_ml,
            ph_shift_down_per_ml, mixing_delay_sec, ec_step_ratio, ph_step_ratio, last_calibrated
        ) 
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            tank_volume_l = excluded.tank_volume_l,
            ec_gain_per_ml = excluded.ec_gain_per_ml,
            ph_shift_up_per_ml = excluded.ph_shift_up_per_ml,
            ph_shift_down_per_ml = excluded.ph_shift_down_per_ml,
            mixing_delay_sec = excluded.mixing_delay_sec,
            ec_step_ratio = excluded.ec_step_ratio,
            ph_step_ratio = excluded.ph_step_ratio,
            last_calibrated = excluded.last_calibrated
        "#,
        device_id,
        cal.tank_volume_l,
        cal.ec_gain_per_ml,
        cal.ph_shift_up_per_ml,
        cal.ph_shift_down_per_ml,
        cal.mixing_delay_sec,
        cal.ec_step_ratio,
        cal.ph_step_ratio,
        now
    )
    .execute(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(_) => {
            info!("Đã cập nhật DosingCalibration cho thiết bị {}", device_id);
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(e) => {
            error!("Lỗi lưu DosingCalibration cho {}: {:?}", device_id, e);
            HttpResponse::InternalServerError().json(json!({"error": "Không thể lưu hiệu chuẩn"}))
        }
    }
}

// Khởi tạo các routes cho module config
// Khởi tạo các routes cho module config
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/devices")
            // 1. Device Config (Cái này Tauri gọi bằng phương thức PUT)
            .route("/{device_id}/config", web::get().to(get_config))
            .route("/{device_id}/config", web::put().to(update_config))
            // 2. Safety Config
            .route(
                "/{device_id}/config/safety",
                web::post().to(update_safety_config),
            )
            // 3. Water Config
            .route("/{device_id}/config/water", web::get().to(get_water_config))
            .route(
                "/{device_id}/config/water",
                web::post().to(update_water_config),
            )
            // 4. Sensor Calibration
            .route(
                "/{device_id}/calibration/sensor",
                web::post().to(update_sensor_calibration),
            )
            // 5. Dosing Calibration
            .route(
                "/{device_id}/calibration/dosing",
                web::get().to(get_dosing_calibration),
            )
            .route(
                "/{device_id}/calibration/dosing",
                web::post().to(update_dosing_calibration),
            ),
    );
}
