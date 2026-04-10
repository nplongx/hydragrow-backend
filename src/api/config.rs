use actix_web::{HttpResponse, Responder, web};
use chrono::Utc;
use rumqttc::QoS;
use serde_json::json;
use tracing::{error, info, instrument};

use crate::AppState;
use crate::models::config::{
    DeviceConfig, DosingCalibration, MqttConfigPayload, SafetyConfig, SensorCalibration,
    WaterConfig,
};

// ==========================================
// HELPER FUNCTIONS
// ==========================================

/// Hàm gom dữ liệu từ 5 bảng DB để tạo payload DUY NHẤT gửi xuống ESP32
async fn fetch_unified_mqtt_config(
    pool: &sqlx::SqlitePool,
    device_id: &str,
) -> Result<MqttConfigPayload, String> {
    // 1. Lấy Base Config (Bắt buộc phải có)
    let dev = sqlx::query_as!(
        DeviceConfig,
        "SELECT * FROM device_config WHERE device_id = ?",
        device_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB Error: {}", e))?
    .ok_or_else(|| "Device base config not found".to_string())?;

    // 2. Lấy Water Config (fallback về Default)
    let water = sqlx::query_as!(
        WaterConfig,
        "SELECT * FROM water_config WHERE device_id = ?",
        device_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| WaterConfig {
        device_id: device_id.to_string(),
        ..Default::default()
    });

    // 3. Lấy Safety Config (fallback về Default)
    let safe = sqlx::query_as!(
        SafetyConfig,
        "SELECT * FROM safety_config WHERE device_id = ?",
        device_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| SafetyConfig {
        device_id: device_id.to_string(),
        ..Default::default()
    });

    // 4. Lấy Dosing Calibration (fallback về Default)
    let dose = sqlx::query_as!(
        DosingCalibration,
        "SELECT * FROM dosing_calibration WHERE device_id = ?",
        device_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| DosingCalibration {
        device_id: device_id.to_string(),
        ..Default::default()
    });

    // 5. Lấy Sensor Calibration (fallback về Default)
    let sens = sqlx::query_as!(
        SensorCalibration,
        "SELECT * FROM sensor_calibration WHERE device_id = ?",
        device_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| SensorCalibration {
        device_id: device_id.to_string(),
        ph_v7: 1650.0,
        ph_v4: 1846.4,
        ec_factor: 880.0,
        ec_offset: 0.0,
        temp_offset: 0.0,
        temp_compensation_beta: 0.02,
        sampling_interval: 1000,
        publish_interval: 5000,
        moving_average_window: 10,
        is_ph_enabled: 1,
        is_ec_enabled: 1,
        is_temp_enabled: 1,
        is_water_level_enabled: 1,
        last_calibrated: String::new(),
    });

    // Chuyển 5 struct DB thành 1 struct phẳng cho ESP32
    Ok(MqttConfigPayload::from_db_rows(
        &dev, &water, &safe, &dose, &sens,
    ))
}

/// Bắn MQTT cấu hình hợp nhất xuống ESP32
pub async fn sync_config_to_esp32(
    app_state: &web::Data<AppState>,
    device_id: &str,
) -> Result<(), String> {
    let payload = fetch_unified_mqtt_config(&app_state.sqlite_pool, device_id).await?;

    // Mạch ESP32 đang Subscribe đúng topic này (src/main.rs)
    let mqtt_topic = format!("AGITECH/{}/controller/config", device_id);

    let mqtt_bytes =
        serde_json::to_vec(&payload).map_err(|e| format!("Lỗi serialize payload: {:?}", e))?;

    app_state
        .mqtt_client
        .publish(&mqtt_topic, QoS::AtLeastOnce, true, mqtt_bytes)
        .await
        .map_err(|e| format!("Lỗi gửi MQTT: {:?}", e))?;

    info!(
        "✅ Đã đồng bộ cấu hình FULL hợp nhất xuống ESP32 ({})",
        device_id
    );
    Ok(())
}

// ==========================================
// API HANDLERS - DEVICE CONFIG
// ==========================================

#[instrument(skip(app_state))]
pub async fn get_config(path: web::Path<String>, app_state: web::Data<AppState>) -> impl Responder {
    let device_id = path.into_inner();
    match crate::db::sqlite::get_device_config(&app_state.sqlite_pool, &device_id).await {
        Ok(config) => HttpResponse::Ok().json(config),
        Err(e) => {
            tracing::warn!("Config not found or DB error: {:?}", e);
            HttpResponse::NotFound().json(json!({"error": "Configuration not found"}))
        }
    }
}

#[instrument(skip(app_state, payload))]
pub async fn update_config(
    path: web::Path<String>,
    payload: web::Json<DeviceConfig>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut config = payload.into_inner();

    config.device_id = device_id.clone();
    config.last_updated = Utc::now().to_rfc3339();

    if let Err(e) = crate::db::sqlite::upsert_device_config(&app_state.sqlite_pool, &config).await {
        error!("Failed to update base config in DB: {:?}", e);
        return HttpResponse::InternalServerError()
            .json(json!({"error": "Failed to save configuration"}));
    }

    // ĐỒNG BỘ MQTT
    if let Err(e) = sync_config_to_esp32(&app_state, &device_id).await {
        error!("Lưu DB thành công nhưng lỗi đồng bộ MQTT: {}", e);
        return HttpResponse::Accepted().json(json!({
            "status": "partial_success",
            "message": "Saved to DB but failed to sync to device",
        }));
    }

    HttpResponse::Ok().json(json!({"status": "success"}))
}

// ==========================================
// API HANDLERS - WATER CONFIG
// ==========================================

#[instrument(skip(app_state))]
pub async fn get_water_config(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result = sqlx::query_as!(
        WaterConfig,
        "SELECT * FROM water_config WHERE device_id = ?",
        device_id
    )
    .fetch_optional(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Water config not found"})),
        Err(e) => {
            error!("Lỗi DB: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": "Database error"}))
        }
    }
}

#[instrument(skip(app_state, req))]
pub async fn update_water_config(
    path: web::Path<String>,
    req: web::Json<WaterConfig>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let config = req.into_inner();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO water_config (
            device_id, water_level_min, water_level_target, water_level_max,
            water_level_drain, circulation_mode, circulation_on_sec,
            circulation_off_sec, water_level_tolerance, auto_refill_enabled,
            auto_drain_overflow, auto_dilute_enabled, dilute_drain_amount_cm,
            scheduled_water_change_enabled, water_change_interval_sec, scheduled_drain_amount_cm,
            misting_on_duration_ms, misting_off_duration_ms, last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            water_level_min = excluded.water_level_min, 
            water_level_target = excluded.water_level_target,
            water_level_max = excluded.water_level_max, 
            water_level_drain = excluded.water_level_drain,
            circulation_mode = excluded.circulation_mode, 
            circulation_on_sec = excluded.circulation_on_sec,
            circulation_off_sec = excluded.circulation_off_sec, 
            water_level_tolerance = excluded.water_level_tolerance,
            auto_refill_enabled = excluded.auto_refill_enabled, 
            auto_drain_overflow = excluded.auto_drain_overflow,
            auto_dilute_enabled = excluded.auto_dilute_enabled, 
            dilute_drain_amount_cm = excluded.dilute_drain_amount_cm,
            scheduled_water_change_enabled = excluded.scheduled_water_change_enabled,
            water_change_interval_sec = excluded.water_change_interval_sec,
            scheduled_drain_amount_cm = excluded.scheduled_drain_amount_cm, 
            misting_on_duration_ms = excluded.misting_on_duration_ms,
            misting_off_duration_ms = excluded.misting_off_duration_ms,
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
        config.water_level_tolerance,
        config.auto_refill_enabled,
        config.auto_drain_overflow,
        config.auto_dilute_enabled,
        config.dilute_drain_amount_cm,
        config.scheduled_water_change_enabled,
        config.water_change_interval_sec,
        config.scheduled_drain_amount_cm,
        config.misting_on_duration_ms,
        config.misting_off_duration_ms,
        now
    )
    .execute(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(_) => {
            let _ = sync_config_to_esp32(&app_state, &device_id).await;
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(e) => {
            error!("Lỗi DB Water Config: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": "DB Error"}))
        }
    }
}

// ==========================================
// API HANDLERS - SAFETY CONFIG
// ==========================================

#[instrument(skip(app_state))]
pub async fn get_safety_config(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result = sqlx::query_as!(
        SafetyConfig,
        "SELECT * FROM safety_config WHERE device_id = ?",
        device_id
    )
    .fetch_optional(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Safety config not found"})),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({"error": "Database error", "details": e.to_string()})),
    }
}

#[instrument(skip(app_state, req))]
pub async fn update_safety_config(
    path: web::Path<String>,
    req: web::Json<SafetyConfig>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let config = req.into_inner();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO safety_config (
            device_id, max_ec_limit, min_ec_limit, min_ph_limit, max_ph_limit, max_ec_delta, max_ph_delta,
            max_dose_per_cycle, cooldown_sec, max_dose_per_hour, water_level_critical_min,
            max_refill_cycles_per_hour, max_drain_cycles_per_hour, max_refill_duration_sec,
            max_drain_duration_sec, max_temp_limit, min_temp_limit, emergency_shutdown, 
            ec_ack_threshold, ph_ack_threshold, water_ack_threshold, last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            max_ec_limit = excluded.max_ec_limit, min_ec_limit = excluded.min_ec_limit, min_ph_limit = excluded.min_ph_limit,
            max_ph_limit = excluded.max_ph_limit, max_ec_delta = excluded.max_ec_delta, max_ph_delta = excluded.max_ph_delta,
            max_dose_per_cycle = excluded.max_dose_per_cycle, cooldown_sec = excluded.cooldown_sec,
            max_dose_per_hour = excluded.max_dose_per_hour, water_level_critical_min = excluded.water_level_critical_min,
            max_refill_cycles_per_hour = excluded.max_refill_cycles_per_hour, max_drain_cycles_per_hour = excluded.max_drain_cycles_per_hour,
            max_refill_duration_sec = excluded.max_refill_duration_sec, max_drain_duration_sec = excluded.max_drain_duration_sec,
            max_temp_limit = excluded.max_temp_limit, min_temp_limit = excluded.min_temp_limit,
            emergency_shutdown = excluded.emergency_shutdown, 
            ec_ack_threshold = excluded.ec_ack_threshold,
            ph_ack_threshold = excluded.ph_ack_threshold,
            water_ack_threshold = excluded.water_ack_threshold,
            last_updated = excluded.last_updated
        "#,
        device_id, config.max_ec_limit, config.min_ec_limit, config.min_ph_limit, config.max_ph_limit,
        config.max_ec_delta, config.max_ph_delta, config.max_dose_per_cycle, config.cooldown_sec,
        config.max_dose_per_hour, config.water_level_critical_min, config.max_refill_cycles_per_hour,
        config.max_drain_cycles_per_hour, config.max_refill_duration_sec, config.max_drain_duration_sec,
        config.max_temp_limit, config.min_temp_limit, config.emergency_shutdown,
        config.ec_ack_threshold, config.ph_ack_threshold, config.water_ack_threshold, now
    ).execute(&app_state.sqlite_pool).await;

    match result {
        Ok(_) => {
            let _ = sync_config_to_esp32(&app_state, &device_id).await;
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({"error": "DB Error", "details": e.to_string()})),
    }
}

// ==========================================
// API HANDLERS - SENSOR CALIBRATION
// ==========================================

#[instrument(skip(app_state))]
pub async fn get_sensor_calibration(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result = sqlx::query_as!(
        SensorCalibration,
        "SELECT * FROM sensor_calibration WHERE device_id = ?",
        device_id
    )
    .fetch_optional(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(Some(cal)) => HttpResponse::Ok().json(cal),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Sensor calibration not found"})),
        Err(e) => HttpResponse::InternalServerError()
            .json(json!({"error": "Database error", "details": e.to_string()})),
    }
}

#[instrument(skip(app_state, req))]
pub async fn update_sensor_calibration(
    path: web::Path<String>,
    req: web::Json<SensorCalibration>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let cal = req.into_inner();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query!(
        r#"
        INSERT INTO sensor_calibration (
            device_id, ph_v7, ph_v4, ec_factor, ec_offset, temp_offset,
            temp_compensation_beta, sampling_interval, publish_interval, moving_average_window,
            is_ph_enabled, is_ec_enabled, is_temp_enabled, is_water_level_enabled, last_calibrated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            ph_v7 = excluded.ph_v7,
            ph_v4 = excluded.ph_v4,
            ec_factor = excluded.ec_factor,
            ec_offset = excluded.ec_offset,
            temp_offset = excluded.temp_offset,
            temp_compensation_beta = excluded.temp_compensation_beta,
            sampling_interval = excluded.sampling_interval,
            publish_interval = excluded.publish_interval,
            moving_average_window = excluded.moving_average_window,
            is_ph_enabled = excluded.is_ph_enabled,
            is_ec_enabled = excluded.is_ec_enabled,
            is_temp_enabled = excluded.is_temp_enabled,
            is_water_level_enabled = excluded.is_water_level_enabled,
            last_calibrated = excluded.last_calibrated
        "#,
        device_id,
        cal.ph_v7,
        cal.ph_v4,
        cal.ec_factor,
        cal.ec_offset,
        cal.temp_offset,
        cal.temp_compensation_beta,
        cal.sampling_interval,
        cal.publish_interval,
        cal.moving_average_window,
        cal.is_ph_enabled,
        cal.is_ec_enabled,
        cal.is_temp_enabled,
        cal.is_water_level_enabled,
        now
    )
    .execute(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(_) => {
            let _ = sync_config_to_esp32(&app_state, &device_id).await;
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(e) => {
            error!("Lỗi DB Update Sensor: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": "DB Error"}))
        }
    }
}

// ==========================================
// API HANDLERS - DOSING CALIBRATION
// ==========================================

#[instrument(skip(app_state))]
pub async fn get_dosing_calibration(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result = sqlx::query_as!(
        DosingCalibration,
        "SELECT * FROM dosing_calibration WHERE device_id = ?",
        device_id
    )
    .fetch_optional(&app_state.sqlite_pool)
    .await;

    match result {
        Ok(Some(cal)) => HttpResponse::Ok().json(cal),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Dosing calibration not found"})),
        Err(_) => HttpResponse::InternalServerError().json(json!({"error": "Database error"})),
    }
}

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
            ph_shift_down_per_ml, active_mixing_sec, sensor_stabilize_sec, ec_step_ratio, ph_step_ratio, 
            dosing_pump_capacity_ml_per_sec, soft_start_duration, last_calibrated, 
            scheduled_mixing_interval_sec, scheduled_mixing_duration_sec
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            tank_volume_l = excluded.tank_volume_l, ec_gain_per_ml = excluded.ec_gain_per_ml,
            ph_shift_up_per_ml = excluded.ph_shift_up_per_ml, ph_shift_down_per_ml = excluded.ph_shift_down_per_ml,
            active_mixing_sec = excluded.active_mixing_sec, sensor_stabilize_sec = excluded.sensor_stabilize_sec,
            ec_step_ratio = excluded.ec_step_ratio, ph_step_ratio = excluded.ph_step_ratio, 
            dosing_pump_capacity_ml_per_sec = excluded.dosing_pump_capacity_ml_per_sec,
            soft_start_duration = excluded.soft_start_duration,
            scheduled_mixing_interval_sec = excluded.scheduled_mixing_interval_sec,
            scheduled_mixing_duration_sec = excluded.scheduled_mixing_duration_sec,
            last_calibrated = excluded.last_calibrated
        "#,
        device_id, cal.tank_volume_l, cal.ec_gain_per_ml, cal.ph_shift_up_per_ml,
        cal.ph_shift_down_per_ml, cal.active_mixing_sec, cal.sensor_stabilize_sec, cal.ec_step_ratio,
        cal.ph_step_ratio, cal.dosing_pump_capacity_ml_per_sec, cal.soft_start_duration,
        now, cal.scheduled_mixing_interval_sec, cal.scheduled_mixing_duration_sec
    ).execute(&app_state.sqlite_pool).await;

    match result {
        Ok(_) => {
            let _ = sync_config_to_esp32(&app_state, &device_id).await;
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(_) => HttpResponse::InternalServerError().json(json!({"error": "DB Error"})),
    }
}

// ==========================================
// API HANDLERS - AGGREGATED VIEWS (CHO FRONTEND)
// ==========================================

#[instrument(skip(app_state))]
pub async fn get_unified_device_config(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    match fetch_unified_mqtt_config(&app_state.sqlite_pool, &device_id).await {
        Ok(aggregated) => HttpResponse::Ok().json(aggregated),
        Err(e) => HttpResponse::NotFound().json(json!({"error": e})),
    }
}

// ==========================================
// ROUTES SETUP
// ==========================================

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/devices")
            // Base config
            .route("/{device_id}/config", web::get().to(get_config))
            .route("/{device_id}/config", web::put().to(update_config))
            // Aggregated configs (Frontend xem trước JSON tổng quát bắn xuống ESP32)
            .route(
                "/{device_id}/config/unified",
                web::get().to(get_unified_device_config),
            )
            // Safety config
            .route(
                "/{device_id}/config/safety",
                web::get().to(get_safety_config),
            )
            .route(
                "/{device_id}/config/safety",
                web::post().to(update_safety_config),
            )
            // Water config
            .route("/{device_id}/config/water", web::get().to(get_water_config))
            .route(
                "/{device_id}/config/water",
                web::post().to(update_water_config),
            )
            // Sensor Calibration
            .route(
                "/{device_id}/calibration/sensor",
                web::get().to(get_sensor_calibration),
            )
            .route(
                "/{device_id}/calibration/sensor",
                web::post().to(update_sensor_calibration),
            )
            // Dosing Calibration
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

