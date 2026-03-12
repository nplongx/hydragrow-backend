use crate::db::{DbError, DbResult};
use crate::models::config::DeviceConfig;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{error, instrument};

#[instrument(skip(pool))]
pub async fn get_device_config(pool: &SqlitePool, device_id: &str) -> DbResult<DeviceConfig> {
    let config = sqlx::query_as!(
        DeviceConfig,
        r#"
        SELECT 
            device_id, ec_target, ec_tolerance, ph_min, ph_max, ph_tolerance,
            temp_min, temp_max, control_mode, is_enabled,
            last_updated as "last_updated: _" 
        FROM device_config 
        WHERE device_id = ?
        "#,
        device_id
    )
    .fetch_one(pool)
    .await?;

    Ok(config)
}

#[instrument(skip(pool, config))]
pub async fn update_device_config(pool: &SqlitePool, config: &DeviceConfig) -> DbResult<()> {
    let now = Utc::now();

    // Upsert query: Update nếu đã tồn tại, Insert nếu chưa có
    sqlx::query!(
        r#"
        INSERT INTO device_config (
            device_id, ec_target, ec_tolerance, ph_min, ph_max, ph_tolerance,
            temp_min, temp_max, control_mode, is_enabled, last_updated
        ) 
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        now
    )
    .execute(pool)
    .await?;

    Ok(())
}
