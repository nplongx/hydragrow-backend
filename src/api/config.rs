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
// HELPER FUNCTIONS (DB & MQTT)
// ==========================================

/// Hàm gom dữ liệu từ 5 bảng DB để tạo payload DUY NHẤT gửi xuống ESP32
async fn fetch_unified_mqtt_config(
    pool: &sqlx::SqlitePool,
    device_id: &str,
) -> Result<MqttConfigPayload, String> {
    // 1. Lấy Base Config (Bắt buộc phải có)
    let dev = sqlx::query_as::<_, DeviceConfig>("SELECT * FROM device_config WHERE device_id = ?")
        .bind(device_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("DB Error: {}", e))?
        .ok_or_else(|| "Device base config not found".to_string())?;

    // 2. Lấy Water Config (fallback về Default)
    let water = sqlx::query_as::<_, WaterConfig>("SELECT * FROM water_config WHERE device_id = ?")
        .bind(device_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| WaterConfig {
            device_id: device_id.to_string(),
            ..Default::default()
        });

    // 3. Lấy Safety Config (fallback về Default)
    let safe = sqlx::query_as::<_, SafetyConfig>("SELECT * FROM safety_config WHERE device_id = ?")
        .bind(device_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| SafetyConfig {
            device_id: device_id.to_string(),
            ..Default::default()
        });

    // 4. Lấy Dosing Calibration (fallback về Default)
    let dose = sqlx::query_as::<_, DosingCalibration>(
        "SELECT * FROM dosing_calibration WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| DosingCalibration {
        device_id: device_id.to_string(),
        ..Default::default()
    });

    // 5. Lấy Sensor Calibration (fallback về Default)
    let sens = sqlx::query_as::<_, SensorCalibration>(
        "SELECT * FROM sensor_calibration WHERE device_id = ?",
    )
    .bind(device_id)
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
// DB UPSERT HELPERS (Chống lặp code SQL)
// ==========================================

async fn upsert_water_db(
    pool: &sqlx::SqlitePool,
    config: &WaterConfig,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
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
    )
    .bind(&config.device_id)
    .bind(config.water_level_min)
    .bind(config.water_level_target)
    .bind(config.water_level_max)
    .bind(config.water_level_drain)
    .bind(&config.circulation_mode)
    .bind(config.circulation_on_sec)
    .bind(config.circulation_off_sec)
    .bind(config.water_level_tolerance)
    .bind(config.auto_refill_enabled)
    .bind(config.auto_drain_overflow)
    .bind(config.auto_dilute_enabled)
    .bind(config.dilute_drain_amount_cm)
    .bind(config.scheduled_water_change_enabled)
    .bind(config.water_change_interval_sec)
    .bind(config.scheduled_drain_amount_cm)
    .bind(config.misting_on_duration_ms)
    .bind(config.misting_off_duration_ms)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

async fn upsert_sensor_db(
    pool: &sqlx::SqlitePool,
    cal: &SensorCalibration,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO sensor_calibration (
            device_id, ph_v7, ph_v4, ec_factor, ec_offset, temp_offset,
            temp_compensation_beta, sampling_interval, publish_interval, moving_average_window,
            is_ph_enabled, is_ec_enabled, is_temp_enabled, is_water_level_enabled, last_calibrated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            ph_v7 = excluded.ph_v7, ph_v4 = excluded.ph_v4, ec_factor = excluded.ec_factor,
            ec_offset = excluded.ec_offset, temp_offset = excluded.temp_offset,
            temp_compensation_beta = excluded.temp_compensation_beta, sampling_interval = excluded.sampling_interval,
            publish_interval = excluded.publish_interval, moving_average_window = excluded.moving_average_window,
            is_ph_enabled = excluded.is_ph_enabled, is_ec_enabled = excluded.is_ec_enabled,
            is_temp_enabled = excluded.is_temp_enabled, is_water_level_enabled = excluded.is_water_level_enabled,
            last_calibrated = excluded.last_calibrated
        "#
    )
    .bind(&cal.device_id)
    .bind(cal.ph_v7)
    .bind(cal.ph_v4)
    .bind(cal.ec_factor)
    .bind(cal.ec_offset)
    .bind(cal.temp_offset)
    .bind(cal.temp_compensation_beta)
    .bind(cal.sampling_interval)
    .bind(cal.publish_interval)
    .bind(cal.moving_average_window)
    .bind(cal.is_ph_enabled)
    .bind(cal.is_ec_enabled)
    .bind(cal.is_temp_enabled)
    .bind(cal.is_water_level_enabled)
    .bind(now)
    .execute(pool).await?;
    Ok(())
}

async fn upsert_dosing_db(
    pool: &sqlx::SqlitePool,
    cal: &DosingCalibration,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO dosing_calibration (
            device_id, tank_volume_l, ec_gain_per_ml, ph_shift_up_per_ml,
            ph_shift_down_per_ml, active_mixing_sec, sensor_stabilize_sec, ec_step_ratio, ph_step_ratio, 
            dosing_pump_capacity_ml_per_sec, soft_start_duration, last_calibrated, 
            scheduled_mixing_interval_sec, scheduled_mixing_duration_sec,
            dosing_pwm_percent, osaka_mixing_pwm_percent, osaka_misting_pwm_percent
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            tank_volume_l = excluded.tank_volume_l, ec_gain_per_ml = excluded.ec_gain_per_ml,
            ph_shift_up_per_ml = excluded.ph_shift_up_per_ml, ph_shift_down_per_ml = excluded.ph_shift_down_per_ml,
            active_mixing_sec = excluded.active_mixing_sec, sensor_stabilize_sec = excluded.sensor_stabilize_sec,
            ec_step_ratio = excluded.ec_step_ratio, ph_step_ratio = excluded.ph_step_ratio, 
            dosing_pump_capacity_ml_per_sec = excluded.dosing_pump_capacity_ml_per_sec,
            soft_start_duration = excluded.soft_start_duration, scheduled_mixing_interval_sec = excluded.scheduled_mixing_interval_sec,
            scheduled_mixing_duration_sec = excluded.scheduled_mixing_duration_sec, dosing_pwm_percent = excluded.dosing_pwm_percent,
            osaka_mixing_pwm_percent = excluded.osaka_mixing_pwm_percent, osaka_misting_pwm_percent = excluded.osaka_misting_pwm_percent,
            last_calibrated = excluded.last_calibrated
        "#
    )
    .bind(&cal.device_id)
    .bind(cal.tank_volume_l)
    .bind(cal.ec_gain_per_ml)
    .bind(cal.ph_shift_up_per_ml)
    .bind(cal.ph_shift_down_per_ml)
    .bind(cal.active_mixing_sec)
    .bind(cal.sensor_stabilize_sec)
    .bind(cal.ec_step_ratio)
    .bind(cal.ph_step_ratio)
    .bind(cal.dosing_pump_capacity_ml_per_sec)
    .bind(cal.soft_start_duration)
    .bind(now)
    .bind(cal.scheduled_mixing_interval_sec)
    .bind(cal.scheduled_mixing_duration_sec)
    .bind(cal.dosing_pwm_percent)
    .bind(cal.osaka_mixing_pwm_percent)
    .bind(cal.osaka_misting_pwm_percent)
    .execute(pool).await?;
    Ok(())
}

// ==========================================
// 🟢 API HANDLER: CẬP NHẬT GỘP (UNIFIED)
// Chỉ tạo 1 request, lưu toàn bộ và bắn MQTT 1 lần
// ==========================================

#[derive(serde::Deserialize)]
pub struct UnifiedConfigRequest {
    pub device_config: DeviceConfig,
    pub water_config: WaterConfig,
    pub safety_config: SafetyConfig,
    pub sensor_calibration: SensorCalibration,
    pub dosing_calibration: DosingCalibration,
}

#[instrument(skip(app_state, req))]
pub async fn update_unified_config(
    path: web::Path<String>,
    req: web::Json<UnifiedConfigRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut payload = req.into_inner();
    let now = Utc::now().to_rfc3339();

    // 1. Lưu Base Config
    payload.device_config.device_id = device_id.clone();
    payload.device_config.last_updated = now.clone();
    if let Err(e) =
        crate::db::sqlite::upsert_device_config(&app_state.sqlite_pool, &payload.device_config)
            .await
    {
        error!("Failed to update device config: {:?}", e);
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error: Device"}));
    }

    // 2. Lưu Safety Config
    payload.safety_config.device_id = device_id.clone();
    payload.safety_config.last_updated = now.clone();
    if let Err(e) =
        crate::db::sqlite::upsert_safety_config(&app_state.sqlite_pool, &payload.safety_config)
            .await
    {
        error!("Failed to update safety config: {:?}", e);
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error: Safety"}));
    }

    // 3. Lưu Water Config
    payload.water_config.device_id = device_id.clone();
    if let Err(e) = upsert_water_db(&app_state.sqlite_pool, &payload.water_config, &now).await {
        error!("Failed to update water config: {:?}", e);
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error: Water"}));
    }

    // 4. Lưu Sensor Config
    payload.sensor_calibration.device_id = device_id.clone();
    if let Err(e) =
        upsert_sensor_db(&app_state.sqlite_pool, &payload.sensor_calibration, &now).await
    {
        error!("Failed to update sensor config: {:?}", e);
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error: Sensor"}));
    }

    // 5. Lưu Dosing Config
    payload.dosing_calibration.device_id = device_id.clone();
    if let Err(e) =
        upsert_dosing_db(&app_state.sqlite_pool, &payload.dosing_calibration, &now).await
    {
        error!("Failed to update dosing config: {:?}", e);
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error: Dosing"}));
    }

    // 6. GỌI MQTT ĐÚNG 1 LẦN DUY NHẤT SAU KHI UPDATE XONG TOÀN BỘ DB
    if let Err(e) = sync_config_to_esp32(&app_state, &device_id).await {
        error!("Lưu DB thành công nhưng lỗi MQTT: {}", e);
        return HttpResponse::Accepted().json(json!({
            "status": "partial_success",
            "message": "Đã lưu CSDL nhưng không thể đồng bộ tới thiết bị do mất kết nối mạng."
        }));
    }

    HttpResponse::Ok().json(json!({"status": "success"}))
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
    let _ = sync_config_to_esp32(&app_state, &device_id).await;
    HttpResponse::Ok().json(json!({"status": "success"}))
}

#[instrument(skip(app_state))]
pub async fn get_water_config(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result = sqlx::query_as::<_, WaterConfig>("SELECT * FROM water_config WHERE device_id = ?")
        .bind(device_id)
        .fetch_optional(&app_state.sqlite_pool)
        .await;
    match result {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Not found"})),
        Err(_) => HttpResponse::InternalServerError().json(json!({"error": "DB Error"})),
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
    if let Err(_) = upsert_water_db(&app_state.sqlite_pool, &config, &now).await {
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error"}));
    }
    let _ = sync_config_to_esp32(&app_state, &device_id).await;
    HttpResponse::Ok().json(json!({"status": "success"}))
}

#[instrument(skip(app_state))]
pub async fn get_safety_config(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result =
        sqlx::query_as::<_, SafetyConfig>("SELECT * FROM safety_config WHERE device_id = ?")
            .bind(device_id)
            .fetch_optional(&app_state.sqlite_pool)
            .await;
    match result {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Not found"})),
        Err(_) => HttpResponse::InternalServerError().json(json!({"error": "DB Error"})),
    }
}

#[instrument(skip(app_state, req))]
pub async fn update_safety_config(
    path: web::Path<String>,
    req: web::Json<SafetyConfig>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut config = req.into_inner();
    config.device_id = device_id.clone();
    config.last_updated = Utc::now().to_rfc3339();
    if let Err(_) = crate::db::sqlite::upsert_safety_config(&app_state.sqlite_pool, &config).await {
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error"}));
    }
    let _ = sync_config_to_esp32(&app_state, &device_id).await;
    HttpResponse::Ok().json(json!({"status": "success"}))
}

#[instrument(skip(app_state))]
pub async fn get_sensor_calibration(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result = sqlx::query_as::<_, SensorCalibration>(
        "SELECT * FROM sensor_calibration WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_optional(&app_state.sqlite_pool)
    .await;
    match result {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Not found"})),
        Err(_) => HttpResponse::InternalServerError().json(json!({"error": "DB Error"})),
    }
}

#[instrument(skip(app_state, req))]
pub async fn update_sensor_calibration(
    path: web::Path<String>,
    req: web::Json<SensorCalibration>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let config = req.into_inner();
    let now = Utc::now().to_rfc3339();
    if let Err(_) = upsert_sensor_db(&app_state.sqlite_pool, &config, &now).await {
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error"}));
    }
    let _ = sync_config_to_esp32(&app_state, &device_id).await;
    HttpResponse::Ok().json(json!({"status": "success"}))
}

#[instrument(skip(app_state))]
pub async fn get_dosing_calibration(
    path: web::Path<String>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let result = sqlx::query_as::<_, DosingCalibration>(
        "SELECT * FROM dosing_calibration WHERE device_id = ?",
    )
    .bind(device_id)
    .fetch_optional(&app_state.sqlite_pool)
    .await;
    match result {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Not found"})),
        Err(_) => HttpResponse::InternalServerError().json(json!({"error": "DB Error"})),
    }
}

#[instrument(skip(app_state, req))]
pub async fn update_dosing_calibration(
    path: web::Path<String>,
    req: web::Json<DosingCalibration>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let config = req.into_inner();
    let now = Utc::now().to_rfc3339();
    if let Err(_) = upsert_dosing_db(&app_state.sqlite_pool, &config, &now).await {
        return HttpResponse::InternalServerError().json(json!({"error": "DB Error"}));
    }
    let _ = sync_config_to_esp32(&app_state, &device_id).await;
    HttpResponse::Ok().json(json!({"status": "success"}))
}

// ==========================================
// ROUTES SETUP
// ==========================================

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // 🟢 NEW: API gộp (Lưu 1 lần duy nhất)
        .route("/config/unified", web::put().to(update_unified_config))
        .route("/config/unified", web::get().to(get_unified_device_config))
        // Base config
        .route("/config", web::get().to(get_config))
        .route("/config", web::put().to(update_config))
        // Safety config
        .route("/safety", web::get().to(get_safety_config))
        .route("/config/safety", web::post().to(update_safety_config))
        // Water config
        .route("/config/water", web::get().to(get_water_config))
        .route("/config/water", web::post().to(update_water_config))
        // Sensor Calibration
        .route("/calibration/sensor", web::get().to(get_sensor_calibration))
        .route(
            "/calibration/sensor",
            web::post().to(update_sensor_calibration),
        )
        // Dosing Calibration
        .route("/calibration/dosing", web::get().to(get_dosing_calibration))
        .route(
            "/calibration/dosing",
            web::post().to(update_dosing_calibration),
        );
}

