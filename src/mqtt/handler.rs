use crate::AppState;
use crate::models::sensor::SensorData;
use anyhow::{Context, Result};
use chrono::{FixedOffset, Utc};
use rumqttc::Publish;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

/// Xử lý một message MQTT được publish tới broker
#[instrument(skip(app_state, publish))]
pub async fn process_message(publish: Publish, app_state: Arc<AppState>) {
    let topic = publish.topic;
    let payload = publish.payload;

    // Route topic: hydro/{device_id}/{action}
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() < 3 || parts[0] != "hydro" {
        debug!("Ignored unknown topic structure: {}", topic);
        return;
    }

    let device_id = parts[1];
    let action = parts[2];

    match action {
        "sensors" => {
            if let Err(e) = handle_sensor_data(device_id, &payload, app_state).await {
                error!(device_id, error = %e, "Failed to process sensor data");
            }
        }
        "status" => {
            // ESP32 gửi trạng thái online/offline hoặc IP local
            if let Ok(status_str) = std::str::from_utf8(&payload) {
                info!(device_id, status = status_str, "Device status updated");
            }
        }
        _ => {
            debug!(topic, "No handler defined for this topic action");
        }
    }
}

/// Xử lý payload sensor data, lưu vào InfluxDB và trigger rule engine
async fn handle_sensor_data(
    device_id: &str,
    payload: &[u8],
    app_state: Arc<AppState>,
) -> Result<()> {
    // 1. Parse JSON payload từ ESP32
    // Lưu ý: Nếu ESP32 không gửi timestamp, chúng ta sẽ tự động gán thời gian hiện tại của Server
    let mut sensor_data: SensorData = serde_json::from_slice(payload)
        .with_context(|| "Failed to parse JSON payload into SensorData")?;

    // Đảm bảo device_id trong payload khớp với topic để tránh giả mạo/nhầm lẫn
    sensor_data.device_id = device_id.to_string();

    // Nếu timestamp từ ESP là default/zero, đè bằng giờ Server (rất hữu ích vì ESP hay bị trôi giờ)
    if sensor_data.timestamp.timestamp() == 0 {
        sensor_data.timestamp = Utc::now().fixed_offset();
    }

    debug!(
        device_id,
        ec = sensor_data.ec_value,
        ph = sensor_data.ph_value,
        "Parsed sensor data"
    );

    // 2. Ghi vào InfluxDB
    crate::db::influx::write_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &sensor_data,
    )
    .await
    .with_context(|| "Failed to write sensor data to InfluxDB")?;

    // 3. (Optional) Check Alert/Safety Config ngay tại đây
    if let Err(e) = crate::services::alert::check_and_trigger_alerts(&app_state, &sensor_data).await
    {
        error!(device_id, error = %e, "Failed to run safety checks");
    }

    Ok(())
}
