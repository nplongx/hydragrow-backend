use anyhow::{Context, Result};
use sqlx::SqlitePool;
use tracing::instrument;

use crate::models::config::{DeviceConfig, SafetyConfig};

/// --- DEVICE CONFIG ---

#[instrument(skip(pool))]
pub async fn get_device_config(pool: &SqlitePool, device_id: &str) -> Result<DeviceConfig> {
    let config = sqlx::query_as!(
        DeviceConfig,
        r#"SELECT * FROM device_config WHERE device_id = ?"#,
        device_id
    )
    .fetch_one(pool)
    .await
    .context(format!("Failed to fetch device_config for {}", device_id))?;

    Ok(config)
}

#[instrument(skip(pool, config))]
pub async fn upsert_device_config(pool: &SqlitePool, config: &DeviceConfig) -> Result<()> {
    // Upsert pattern trong SQLite: ON CONFLICT DO UPDATE
    sqlx::query!(
        r#"
        INSERT INTO device_config (
            device_id, ec_target, ec_tolerance, ph_min, ph_max, ph_tolerance, 
            temp_min, temp_max, control_mode, is_enabled, last_updated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(device_id) DO UPDATE SET
            ec_target = excluded.ec_target,
            ec_tolerance = excluded.ec_tolerance,
            ph_min = excluded.ph_min,
            ph_max = excluded.ph_max,
            ph_tolerance = excluded.ph_tolerance,
            temp_min = excluded.temp_min,
            temp_max = excluded.temp_max,
            control_mode = excluded.control_mode,
            is_enabled = excluded.is_enabled,
            last_updated = excluded.last_updated
        "#,
        config.device_id,
        config.ec_target,
        config.ec_tolerance,
        config.ph_min,
        config.ph_max,
        config.ph_tolerance,
        config.temp_min,
        config.temp_max,
        config.control_mode,
        config.is_enabled,
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
            min_ph_limit,
            max_ph_limit,
            max_ec_delta,
            max_ph_delta,

            max_dose_per_cycle,
            cooldown_sec,
            max_dose_per_hour,

            water_level_critical_min,
            water_level_target,
            water_level_max,

            max_refill_cycles_per_hour,
            max_drain_cycles_per_hour,
            max_refill_duration_sec,
            max_drain_duration_sec,

            emergency_shutdown,
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

        max_ec_limit,
        min_ph_limit,
        max_ph_limit,
        max_ec_delta,
        max_ph_delta,

        max_dose_per_cycle,
        cooldown_sec,
        max_dose_per_hour,

        water_level_critical_min,
        water_level_target,
        water_level_max,

        max_refill_cycles_per_hour,
        max_drain_cycles_per_hour,

        max_refill_duration_sec,
        max_drain_duration_sec,

        emergency_shutdown,
        last_updated
    )
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)

    ON CONFLICT(device_id) DO UPDATE SET

        max_ec_limit = excluded.max_ec_limit,
        min_ph_limit = excluded.min_ph_limit,
        max_ph_limit = excluded.max_ph_limit,
        max_ec_delta = excluded.max_ec_delta,
        max_ph_delta = excluded.max_ph_delta,

        max_dose_per_cycle = excluded.max_dose_per_cycle,
        cooldown_sec = excluded.cooldown_sec,
        max_dose_per_hour = excluded.max_dose_per_hour,

        water_level_critical_min = excluded.water_level_critical_min,
        water_level_target = excluded.water_level_target,
        water_level_max = excluded.water_level_max,

        max_refill_cycles_per_hour = excluded.max_refill_cycles_per_hour,
        max_drain_cycles_per_hour = excluded.max_drain_cycles_per_hour,

        max_refill_duration_sec = excluded.max_refill_duration_sec,
        max_drain_duration_sec = excluded.max_drain_duration_sec,

        emergency_shutdown = excluded.emergency_shutdown,
        last_updated = excluded.last_updated
    "#,
        config.device_id,
        config.max_ec_limit,
        config.min_ph_limit,
        config.max_ph_limit,
        config.max_ec_delta,
        config.max_ph_delta,
        config.max_dose_per_cycle,
        config.cooldown_sec,
        config.max_dose_per_hour,
        config.water_level_critical_min,
        config.water_level_target,
        config.water_level_max,
        config.max_refill_cycles_per_hour,
        config.max_drain_cycles_per_hour,
        config.max_refill_duration_sec,
        config.max_drain_duration_sec,
        config.emergency_shutdown,
        config.last_updated
    )
    .execute(pool)
    .await?;

    Ok(())
}
