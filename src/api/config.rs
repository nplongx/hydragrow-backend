use actix_web::{HttpResponse, Responder, web};
use chrono::Utc;
use rumqttc::QoS;
use serde_json::json;
use tracing::{error, info, instrument};

use crate::AppState;
use crate::models::config::{
    DeviceConfig, DosingCalibration, Esp32AggregatedConfig, SafetyConfig, SensorCalibration,
    WaterConfig,
};

// ==========================================
// HELPER FUNCTIONS
// ==========================================

/// Hàm dùng chung để gom toàn bộ dữ liệu Config từ Database
async fn fetch_aggregated_config(
    pool: &sqlx::SqlitePool,
    device_id: &str,
) -> Result<Esp32AggregatedConfig, String> {
    // 1. Lấy Base Config (Bắt buộc phải có)
    let base = sqlx::query_as!(
        DeviceConfig,
        "SELECT * FROM device_config WHERE device_id = ?",
        device_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("DB Error: {}", e))?
    .ok_or_else(|| "Device base config not found".to_string())?;

    // 2. Lấy Water Config (fallback về Default nếu chưa có)
    let w_cfg = sqlx::query_as!(
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
    let s_cfg = sqlx::query_as!(
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
    let d_cfg = sqlx::query_as!(
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

    // Mapping sang Esp32AggregatedConfig (ĐÃ CẬP NHẬT TRƯỜNG MỚI)
    Ok(Esp32AggregatedConfig {
        device_id: device_id.to_string(),
        control_mode: base.control_mode.to_lowercase(),
        is_enabled: base.is_enabled == 1,

        ec_target: base.ec_target,
        ec_tolerance: base.ec_tolerance,
        ph_target: base.ph_target,
        ph_tolerance: base.ph_tolerance,

        water_level_min: w_cfg.water_level_min,
        water_level_target: w_cfg.water_level_target,
        water_level_max: w_cfg.water_level_max,
        water_level_tolerance: w_cfg.water_level_tolerance,
        auto_refill_enabled: w_cfg.auto_refill_enabled == 1,
        auto_drain_overflow: w_cfg.auto_drain_overflow == 1,
        auto_dilute_enabled: w_cfg.auto_dilute_enabled == 1,
        dilute_drain_amount_cm: w_cfg.dilute_drain_amount_cm,
        scheduled_water_change_enabled: w_cfg.scheduled_water_change_enabled == 1,
        water_change_interval_sec: w_cfg.water_change_interval_sec,
        scheduled_drain_amount_cm: w_cfg.scheduled_drain_amount_cm,

        emergency_shutdown: s_cfg.emergency_shutdown == 1,
        max_ec_limit: s_cfg.max_ec_limit,
        min_ph_limit: s_cfg.min_ph_limit,
        max_ph_limit: s_cfg.max_ph_limit,
        max_ec_delta: s_cfg.max_ec_delta,
        max_ph_delta: s_cfg.max_ph_delta,
        max_dose_per_cycle: s_cfg.max_dose_per_cycle,
        water_level_critical_min: s_cfg.water_level_critical_min,
        max_refill_duration_sec: s_cfg.max_refill_duration_sec,
        max_drain_duration_sec: s_cfg.max_drain_duration_sec,

        ec_gain_per_ml: d_cfg.ec_gain_per_ml,
        ph_shift_up_per_ml: d_cfg.ph_shift_up_per_ml,
        ph_shift_down_per_ml: d_cfg.ph_shift_down_per_ml,

        // --- CẬP NHẬT MỚI Ở ĐÂY ---
        active_mixing_sec: d_cfg.active_mixing_sec,
        sensor_stabilize_sec: d_cfg.sensor_stabilize_sec,
        ec_step_ratio: d_cfg.ec_step_ratio,
        ph_step_ratio: d_cfg.ph_step_ratio,
        dosing_pump_capacity_ml_per_sec: d_cfg.dosing_pump_capacity_ml_per_sec,
    })
}

/// Gom cấu hình và bắn MQTT xuống ESP32
pub async fn sync_full_config_to_esp32(
    app_state: &web::Data<AppState>,
    device_id: &str,
) -> Result<(), String> {
    let aggregated = fetch_aggregated_config(&app_state.sqlite_pool, device_id).await?;

    let mqtt_topic = format!("AGITECH/{}/config", device_id);
    let mqtt_payload =
        serde_json::to_vec(&aggregated).map_err(|e| format!("Lỗi serialize payload: {:?}", e))?;

    app_state
        .mqtt_client
        .publish(&mqtt_topic, QoS::AtLeastOnce, true, mqtt_payload)
        .await
        .map_err(|e| format!("Lỗi gửi MQTT: {:?}", e))?;

    info!("Đã đồng bộ toàn bộ cấu hình xuống ESP32 ({})", device_id);
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
    if let Err(e) = sync_full_config_to_esp32(&app_state, &device_id).await {
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
            last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            water_level_min = excluded.water_level_min, water_level_target = excluded.water_level_target,
            water_level_max = excluded.water_level_max, water_level_drain = excluded.water_level_drain,
            circulation_mode = excluded.circulation_mode, circulation_on_sec = excluded.circulation_on_sec,
            circulation_off_sec = excluded.circulation_off_sec, water_level_tolerance = excluded.water_level_tolerance,
            auto_refill_enabled = excluded.auto_refill_enabled, auto_drain_overflow = excluded.auto_drain_overflow,
            auto_dilute_enabled = excluded.auto_dilute_enabled, dilute_drain_amount_cm = excluded.dilute_drain_amount_cm,
            scheduled_water_change_enabled = excluded.scheduled_water_change_enabled,
            water_change_interval_sec = excluded.water_change_interval_sec,
            scheduled_drain_amount_cm = excluded.scheduled_drain_amount_cm, last_updated = excluded.last_updated
        "#,
        device_id, config.water_level_min, config.water_level_target, config.water_level_max,
        config.water_level_drain, config.circulation_mode, config.circulation_on_sec,
        config.circulation_off_sec, config.water_level_tolerance, config.auto_refill_enabled,
        config.auto_drain_overflow, config.auto_dilute_enabled, config.dilute_drain_amount_cm,
        config.scheduled_water_change_enabled, config.water_change_interval_sec,
        config.scheduled_drain_amount_cm, now
    ).execute(&app_state.sqlite_pool).await;

    match result {
        Ok(_) => {
            let _ = sync_full_config_to_esp32(&app_state, &device_id).await; // ĐỒNG BỘ MQTT
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
            max_drain_duration_sec, max_temp_limit, min_temp_limit, emergency_shutdown, last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            max_ec_limit = excluded.max_ec_limit, min_ec_limit = excluded.min_ec_limit, min_ph_limit = excluded.min_ph_limit,
            max_ph_limit = excluded.max_ph_limit, max_ec_delta = excluded.max_ec_delta, max_ph_delta = excluded.max_ph_delta,
            max_dose_per_cycle = excluded.max_dose_per_cycle, cooldown_sec = excluded.cooldown_sec,
            max_dose_per_hour = excluded.max_dose_per_hour, water_level_critical_min = excluded.water_level_critical_min,
            max_refill_cycles_per_hour = excluded.max_refill_cycles_per_hour, max_drain_cycles_per_hour = excluded.max_drain_cycles_per_hour,
            max_refill_duration_sec = excluded.max_refill_duration_sec, max_drain_duration_sec = excluded.max_drain_duration_sec,
            max_temp_limit = excluded.max_temp_limit, min_temp_limit = excluded.min_temp_limit,
            emergency_shutdown = excluded.emergency_shutdown, last_updated = excluded.last_updated
        "#,
        device_id, config.max_ec_limit, config.min_ec_limit, config.min_ph_limit, config.max_ph_limit,
        config.max_ec_delta, config.max_ph_delta, config.max_dose_per_cycle, config.cooldown_sec,
        config.max_dose_per_hour, config.water_level_critical_min, config.max_refill_cycles_per_hour,
        config.max_drain_cycles_per_hour, config.max_refill_duration_sec, config.max_drain_duration_sec,
        config.max_temp_limit, config.min_temp_limit, config.emergency_shutdown, now
    ).execute(&app_state.sqlite_pool).await;

    match result {
        Ok(_) => {
            let _ = sync_full_config_to_esp32(&app_state, &device_id).await; // ĐỒNG BỘ MQTT
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
        Ok(_) => {
            // Note: Cân nhắc xem Sensor Calibration có nằm trong Aggregated payload không,
            // nếu không nằm trong thì bạn gọi hàm publish riếng giống code cũ của bạn nhé.
            let _ = sync_full_config_to_esp32(&app_state, &device_id).await;
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": "DB Error"})),
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
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": "Database error"})),
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

    // SQL ĐÃ ĐƯỢC CẬP NHẬT THEO TRƯỜNG MỚI
    let result = sqlx::query!(
        r#"
        INSERT INTO dosing_calibration (
            device_id, tank_volume_l, ec_gain_per_ml, ph_shift_up_per_ml,
            ph_shift_down_per_ml, active_mixing_sec, sensor_stabilize_sec, ec_step_ratio, ph_step_ratio, 
            dosing_pump_capacity_ml_per_sec, last_calibrated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            tank_volume_l = excluded.tank_volume_l, ec_gain_per_ml = excluded.ec_gain_per_ml,
            ph_shift_up_per_ml = excluded.ph_shift_up_per_ml, ph_shift_down_per_ml = excluded.ph_shift_down_per_ml,
            active_mixing_sec = excluded.active_mixing_sec, sensor_stabilize_sec = excluded.sensor_stabilize_sec,
            ec_step_ratio = excluded.ec_step_ratio, ph_step_ratio = excluded.ph_step_ratio, 
            dosing_pump_capacity_ml_per_sec = excluded.dosing_pump_capacity_ml_per_sec,
            last_calibrated = excluded.last_calibrated
        "#,
        device_id, cal.tank_volume_l, cal.ec_gain_per_ml, cal.ph_shift_up_per_ml,
        cal.ph_shift_down_per_ml, cal.active_mixing_sec, cal.sensor_stabilize_sec, cal.ec_step_ratio,
        cal.ph_step_ratio, cal.dosing_pump_capacity_ml_per_sec, now
    ).execute(&app_state.sqlite_pool).await;

    match result {
        Ok(_) => {
            let _ = sync_full_config_to_esp32(&app_state, &device_id).await; // ĐỒNG BỘ MQTT
            HttpResponse::Ok().json(json!({"status": "success"}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": "DB Error"})),
    }
}

// ==========================================
// API HANDLERS - AGGREGATED
// ==========================================

#[instrument(skip(app_state))]
pub async fn get_esp32_aggregated_config(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    // Tái sử dụng hàm helper ở trên
    match fetch_aggregated_config(&app_state.sqlite_pool, &device_id).await {
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
            // Aggregated config
            .route(
                "/{device_id}/config/aggregated",
                web::get().to(get_esp32_aggregated_config),
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

