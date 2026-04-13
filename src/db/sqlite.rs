use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Error, FromRow, SqlitePool};
use tracing::instrument;

use crate::models::config::{DeviceConfig, SafetyConfig};

// 🟢 1. KHAI BÁO STRUCT BLOCKCHAIN RECORD ĐỂ HỨNG DỮ LIỆU TỪ DB
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct BlockchainRecord {
    pub id: i64,
    pub device_id: String,
    pub season_id: Option<String>,
    pub action: String,
    pub tx_id: String,
    // Trả về dạng chuỗi (String) cho dễ map với SQLite DATETIME
    pub created_at: String,
}

/// --- DEVICE CONFIG ---

#[instrument(skip(pool))]
pub async fn get_device_config(pool: &SqlitePool, device_id: &str) -> Result<DeviceConfig> {
    let config = sqlx::query_as!(
        DeviceConfig,
        r#"SELECT
            device_id, ec_target, ec_tolerance, ph_target, ph_tolerance, 
            temp_target, temp_tolerance, control_mode, is_enabled, 
            pump_a_capacity_ml_per_sec, pump_b_capacity_ml_per_sec, delay_between_a_and_b_sec,
            last_updated
        FROM device_config WHERE device_id = ?"#,
        device_id
    )
    .fetch_one(pool)
    .await
    .context(format!("Failed to fetch device_config for {}", device_id))?;

    Ok(config)
}

#[instrument(skip(pool, config))]
pub async fn upsert_device_config(pool: &SqlitePool, config: &DeviceConfig) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO device_config (
            device_id, ec_target, ec_tolerance, ph_target, ph_tolerance, 
            temp_target, temp_tolerance, control_mode, is_enabled, 
            pump_a_capacity_ml_per_sec, pump_b_capacity_ml_per_sec, delay_between_a_and_b_sec,
            last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            ec_target = excluded.ec_target,
            ec_tolerance = excluded.ec_tolerance,
            ph_target = excluded.ph_target,
            temp_target = excluded.temp_target,
            temp_tolerance = excluded.temp_tolerance,
            control_mode = excluded.control_mode,
            is_enabled = excluded.is_enabled,
            pump_a_capacity_ml_per_sec = excluded.pump_a_capacity_ml_per_sec,
            pump_b_capacity_ml_per_sec = excluded.pump_b_capacity_ml_per_sec,
            delay_between_a_and_b_sec = excluded.delay_between_a_and_b_sec,   
            last_updated = excluded.last_updated
        "#,
        config.device_id,
        config.ec_target,
        config.ec_tolerance,
        config.ph_target,
        config.ph_tolerance,
        config.temp_target,
        config.temp_tolerance,
        config.control_mode,
        config.is_enabled,
        config.pump_a_capacity_ml_per_sec,
        config.pump_b_capacity_ml_per_sec,
        config.delay_between_a_and_b_sec,
        config.last_updated
    )
    .execute(pool)
    .await
    .context("Failed to upsert device_config")?;

    Ok(())
}

/// --- SAFETY CONFIG ---

#[instrument(skip(pool))]
pub async fn get_safety_config(pool: &SqlitePool, device_id: &str) -> Result<SafetyConfig> {
    let config = sqlx::query_as!(
        SafetyConfig,
        r#"
        SELECT
            device_id,
            max_ec_limit,
            min_ec_limit,
            min_ph_limit,
            max_ph_limit,
            max_ec_delta,
            max_ph_delta,
            max_dose_per_cycle,
            cooldown_sec,
            max_dose_per_hour,
            water_level_critical_min,
            max_refill_cycles_per_hour,
            max_drain_cycles_per_hour,
            max_refill_duration_sec,
            max_drain_duration_sec,
            min_temp_limit,
            max_temp_limit,
            emergency_shutdown,
            ec_ack_threshold,     
            ph_ack_threshold,     
            water_ack_threshold,  
            last_updated
        FROM safety_config
        WHERE device_id = ?
        "#,
        device_id
    )
    .fetch_one(pool)
    .await
    .context(format!("Failed to fetch safety_config for {}", device_id))?;

    Ok(config)
}

#[instrument(skip(pool, config))]
pub async fn upsert_safety_config(pool: &SqlitePool, config: &SafetyConfig) -> Result<()> {
    sqlx::query!(
        r#"
    INSERT INTO safety_config (
        device_id, 
        max_ec_limit, min_ec_limit, min_ph_limit, max_ph_limit, max_ec_delta, max_ph_delta,
        max_dose_per_cycle, cooldown_sec, max_dose_per_hour, water_level_critical_min,
        max_refill_cycles_per_hour, max_drain_cycles_per_hour, max_refill_duration_sec,
        max_drain_duration_sec, min_temp_limit, max_temp_limit, emergency_shutdown,
        ec_ack_threshold, ph_ack_threshold, water_ack_threshold, last_updated
    )
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)

    ON CONFLICT(device_id) DO UPDATE SET
        max_ec_limit = excluded.max_ec_limit,
        min_ec_limit = excluded.min_ec_limit,
        min_ph_limit = excluded.min_ph_limit,
        max_ph_limit = excluded.max_ph_limit,
        max_ec_delta = excluded.max_ec_delta,
        max_ph_delta = excluded.max_ph_delta,
        max_dose_per_cycle = excluded.max_dose_per_cycle,
        cooldown_sec = excluded.cooldown_sec,
        max_dose_per_hour = excluded.max_dose_per_hour,
        water_level_critical_min = excluded.water_level_critical_min,
        max_refill_cycles_per_hour = excluded.max_refill_cycles_per_hour,
        max_drain_cycles_per_hour = excluded.max_drain_cycles_per_hour,
        max_refill_duration_sec = excluded.max_refill_duration_sec,
        max_drain_duration_sec = excluded.max_drain_duration_sec,
        min_temp_limit = excluded.min_temp_limit,
        max_temp_limit = excluded.max_temp_limit,
        emergency_shutdown = excluded.emergency_shutdown,
        ec_ack_threshold = excluded.ec_ack_threshold,
        ph_ack_threshold = excluded.ph_ack_threshold,
        water_ack_threshold = excluded.water_ack_threshold,
        last_updated = excluded.last_updated
    "#,
        config.device_id,
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
        config.min_temp_limit,
        config.max_temp_limit,
        config.emergency_shutdown,
        config.ec_ack_threshold,
        config.ph_ack_threshold,
        config.water_ack_threshold,
        config.last_updated
    )
    .execute(pool)
    .await?;

    Ok(())
}

// 🟢 2. SỬ DỤNG sqlx::query (không chấm than) và .bind() ĐỂ BYPASS LỖI COMPILE
pub async fn insert_blockchain_tx(
    pool: &SqlitePool,
    device_id: &str,
    season_id: &str,
    action: &str,
    tx_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO blockchain_logs (device_id, season_id, action, tx_id)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(device_id)
    .bind(season_id)
    .bind(action)
    .bind(tx_id)
    .execute(pool)
    .await?;
    Ok(())
}

// 🟢 3. SỬ DỤNG sqlx::query_as::<_, BlockchainRecord> (không chấm than)
pub async fn get_device_blockchain_history(
    pool: &SqlitePool,
    device_id: &str,
    season_id: Option<String>,
) -> Result<Vec<BlockchainRecord>, sqlx::Error> {
    match season_id {
        Some(s_id) => {
            sqlx::query_as::<_, BlockchainRecord>(
                r#"SELECT * FROM blockchain_logs WHERE device_id = ? AND season_id = ? ORDER BY created_at DESC"#
            )
            .bind(device_id)
            .bind(s_id)
            .fetch_all(pool)
            .await
        },
        None => {
            sqlx::query_as::<_, BlockchainRecord>(
                r#"SELECT * FROM blockchain_logs WHERE device_id = ? ORDER BY created_at DESC LIMIT 100"#
            )
            .bind(device_id)
            .fetch_all(pool)
            .await
        }
    }
}

use crate::models::crop_season::{CreateCropSeasonRequest, CropSeason};

// Lấy mùa vụ đang chạy
pub async fn get_active_crop_season(
    pool: &SqlitePool,
    device_id: &str,
) -> Result<Option<CropSeason>, sqlx::Error> {
    let season = sqlx::query_as::<_, CropSeason>(
        "SELECT id, device_id, name, plant_type, CAST(start_time AS TEXT) as start_time, CAST(end_time AS TEXT) as end_time, status 
         FROM crop_seasons WHERE device_id = ? AND status = 'active' LIMIT 1"
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;
    Ok(season)
}

// Lấy lịch sử mùa vụ
pub async fn get_crop_seasons_history(
    pool: &SqlitePool,
    device_id: &str,
) -> Result<Vec<CropSeason>, sqlx::Error> {
    let seasons = sqlx::query_as::<_, CropSeason>(
        "SELECT id, device_id, name, plant_type, CAST(start_time AS TEXT) as start_time, CAST(end_time AS TEXT) as end_time, status 
         FROM crop_seasons WHERE device_id = ? ORDER BY start_time DESC"
    )
    .bind(device_id)
    .fetch_all(pool)
    .await?;
    Ok(seasons)
}

// Tạo mùa vụ mới
pub async fn create_crop_season(
    pool: &SqlitePool,
    device_id: &str,
    req: CreateCropSeasonRequest,
) -> Result<Option<CropSeason>, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string(); // Đảm bảo bạn đã thêm `uuid = { version = "1.0", features = ["v4"] }` vào Cargo.toml
    sqlx::query(
        "INSERT INTO crop_seasons (id, device_id, name, plant_type, status) VALUES (?, ?, ?, ?, 'active')"
    )
    .bind(&id)
    .bind(device_id)
    .bind(&req.name)
    .bind(&req.plant_type)
    .execute(pool)
    .await?;

    get_active_crop_season(pool, device_id).await
}

// Kết thúc mùa vụ
pub async fn end_active_crop_season(pool: &SqlitePool, device_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE crop_seasons SET status = 'completed', end_time = CURRENT_TIMESTAMP WHERE device_id = ? AND status = 'active'"
    )
    .bind(device_id)
    .execute(pool)
    .await?;
    Ok(())
}
