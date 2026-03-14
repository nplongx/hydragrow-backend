use actix_web::web;
use rumqttc::Publish;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

use crate::AppState; // Cấu trúc AppState sẽ được định nghĩa ở main.rs
use crate::db::influx::write_sensor_data;
use crate::models::sensor::SensorData;

#[derive(Deserialize)]
struct DeviceStatusPayload {
    online: bool,
}

/// Xử lý tất cả các message MQTT đẩy lên từ thiết bị
/// LƯU Ý: Không bao giờ được dùng .unwrap() hay return Err() gây panic ở đây,
/// vì nó sẽ làm sập (crash) toàn bộ vòng lặp sự kiện MQTT của hệ thống.
#[instrument(skip(app_state, publish))]
pub async fn process_message(publish: Publish, app_state: web::Data<AppState>) {
    let topic = publish.topic.clone();
    let payload_bytes = publish.payload;

    // Phân tích topic: hydro/{device_id}/{action}
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 3 || parts[0] != "hydro" {
        warn!("Bỏ qua topic không đúng chuẩn hệ thống: {}", topic);
        return;
    }

    let device_id = parts[1].to_string();
    let action = parts[2];

    match action {
        "sensors" => {
            handle_sensor_data(device_id, &payload_bytes, app_state).await;
        }
        "status" => {
            handle_device_status(device_id, &payload_bytes, app_state).await;
        }
        _ => {
            debug!("Nhận được topic không quản lý: {}", topic);
        }
    }
}

async fn handle_sensor_data(device_id: String, payload: &[u8], app_state: web::Data<AppState>) {
    // 1. Parse JSON payload
    let sensor_data: SensorData = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(e) => {
            error!(
                "Lỗi parse JSON SensorData từ thiết bị {}: {:?}",
                device_id, e
            );
            return;
        }
    };

    debug!(
        "Nhận dữ liệu cảm biến từ {}: ph={:?}, ec={:?}",
        device_id, sensor_data.ph_value, sensor_data.ec_value
    );

    // 2. Ghi dữ liệu vào InfluxDB
    if let Err(e) = write_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &sensor_data,
    )
    .await
    {
        error!("Lỗi lưu SensorData vào InfluxDB ({}): {:?}", device_id, e);
        // Không return ở đây để vẫn có thể đẩy data lên WebSocket cho user xem real-time
    }

    // 3. (Optional) Gọi service Alert để check SafetyConfig và trigger cảnh báo
    // crate::services::alert::check_safety_rules(&sensor_data, &app_state).await;

    // 4. Đẩy dữ liệu ra WebSocket (Frontend)
    // Giả sử app_state.ws_sender là một tokio::sync::broadcast::Sender<String>
    let ws_msg = json!({
        "type": "sensor_update",
        "device_id": device_id,
        "data": sensor_data
    });

    // Bỏ qua lỗi nếu không có client nào đang kết nối (channel empty)
    let _ = app_state.alert_sender.send(ws_msg.to_string());
}

async fn handle_device_status(device_id: String, payload: &[u8], app_state: web::Data<AppState>) {
    let status: DeviceStatusPayload = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(e) => {
            error!("Lỗi parse DeviceStatus từ {}: {:?}", device_id, e);
            return;
        }
    };

    info!(
        "Thiết bị {} vừa chuyển trạng thái online: {}",
        device_id, status.online
    );

    // Đẩy thông báo trạng thái thiết bị ra WebSocket
    let ws_msg = json!({
        "type": "device_status",
        "device_id": device_id,
        "online": status.online
    });

    let _ = app_state.alert_sender.send(ws_msg.to_string());
}
