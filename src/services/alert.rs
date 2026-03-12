use crate::models::{config::SafetyConfig, sensor::SensorData};
use crate::{AlertMessage, AppState};
use anyhow::Result;
use std::sync::Arc;
use tracing::{error, instrument, warn};

#[instrument(skip(app_state, sensor_data))]
pub async fn check_and_trigger_alerts(
    app_state: &Arc<AppState>,
    sensor_data: &SensorData,
) -> Result<()> {
    let device_id = &sensor_data.device_id;

    // 1. Lấy Safety Config từ SQLite
    // Lưu ý: Trong thực tế, bạn có thể cache cái này trong bộ nhớ (DashMap) để giảm tải DB
    // Nhưng với SQLite (file local) và hệ thống single-user, query trực tiếp vẫn cực kỳ nhanh.
    let safety_config = sqlx::query_as!(
        SafetyConfig,
        r#"
        SELECT 
            device_id, max_ec_limit, min_ph_limit, max_ph_limit, max_ec_delta, max_ph_delta,
            max_dose_per_cycle, cooldown_sec, max_dose_per_hour, emergency_shutdown,
            last_updated as "last_updated: _"
        FROM safety_config 
        WHERE device_id = ?
        "#,
        device_id
    )
    .fetch_optional(&app_state.sqlite_pool)
    .await?;

    let config = match safety_config {
        Some(c) => c,
        None => return Ok(()), // Chưa có cấu hình thì bỏ qua kiểm tra
    };

    // 2. Kiểm tra các điều kiện an toàn & Gửi cảnh báo
    let mut alerts = Vec::new();

    // -- Kiểm tra EC --
    if sensor_data.ec_value > config.max_ec_limit {
        alerts.push(AlertMessage {
            alert_type: "safety_breach".to_string(),
            device_id: device_id.clone(),
            metric: "ec".to_string(),
            value: sensor_data.ec_value,
            severity: "critical".to_string(),
        });
    }

    // -- Kiểm tra pH --
    if sensor_data.ph_value < config.min_ph_limit {
        alerts.push(AlertMessage {
            alert_type: "safety_breach".to_string(),
            device_id: device_id.clone(),
            metric: "ph".to_string(),
            value: sensor_data.ph_value,
            severity: "critical".to_string(),
        });
    } else if sensor_data.ph_value > config.max_ph_limit {
        alerts.push(AlertMessage {
            alert_type: "safety_breach".to_string(),
            device_id: device_id.clone(),
            metric: "ph".to_string(),
            value: sensor_data.ph_value,
            severity: "critical".to_string(),
        });
    }

    // 3. Broadcast các cảnh báo qua WebSocket
    for alert in alerts {
        warn!(
            device_id = %alert.device_id,
            metric = %alert.metric,
            value = %alert.value,
            "Safety limit breached! Triggering alert."
        );

        // Gửi qua channel. Bỏ qua lỗi nếu không có client nào đang kết nối (WS trống)
        let _ = app_state.alert_sender.send(alert);

        // NẾU vi phạm quá nghiêm trọng (hoặc emergency_shutdown == 1), ra lệnh ngắt bơm
        if config.emergency_shutdown == 1 {
            if let Err(e) =
                crate::services::command::trigger_emergency_stop(app_state, device_id).await
            {
                error!("Failed to trigger emergency stop: {}", e);
            }
        }
    }

    Ok(())
}
