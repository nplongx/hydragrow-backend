use actix_web::web;
use chrono::DateTime;
use rumqttc::Publish;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info, instrument, warn};

use crate::AppState;
use crate::db::influx::write_sensor_data;
// 🟢 MỚI: Đảm bảo bạn đã import PumpStatus (giả định nó nằm cùng chỗ với SensorData)
use crate::models::sensor::{PumpStatus, SensorData};

#[derive(Debug, Deserialize, Serialize)]
pub struct DosingReportPayload {
    pub start_ec: f32,
    pub start_ph: f32,
    pub pump_a_ml: f32,
    pub pump_b_ml: f32,
    pub ph_up_ml: f32,
    pub ph_down_ml: f32,
    pub target_ec: f32,
    pub target_ph: f32,
}

#[derive(Deserialize)]
struct DeviceStatusPayload {
    online: bool,
}

#[derive(Deserialize)]
struct FsmPayload {
    pub current_state: String,
}

#[derive(Debug, Deserialize)]
pub struct IncomingSensorPayload {
    #[serde(rename = "temp_value")]
    pub temp: Option<f64>,

    #[serde(rename = "ec_value")]
    pub ec: Option<f64>,

    #[serde(rename = "ph_value")]
    pub ph: Option<f64>,

    pub water_level: Option<f64>,

    // 🟢 ĐÃ SỬA: Đổi lại thành timestamp_ms và kiểu u64 để khớp 100% với ESP32
    pub timestamp_ms: Option<u64>,

    pub pump_status: Option<PumpStatus>,
}

#[instrument(skip(app_state, publish))]
pub async fn process_message(publish: Publish, app_state: web::Data<AppState>) {
    let topic = publish.topic.clone();
    let payload_bytes = publish.payload;

    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 3 || parts[0] != "AGITECH" {
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
        "fsm" => {
            handle_fsm_state(device_id, &payload_bytes, app_state).await;
        }
        "dosing_report" => {
            handle_dosing_report(device_id, &payload_bytes, app_state).await;
        }
        _ => {
            debug!("Nhận được topic không quản lý: {}", topic);
        }
    }
}

async fn handle_sensor_data(device_id: String, payload: &[u8], app_state: web::Data<AppState>) {
    let incoming: IncomingSensorPayload = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(e) => {
            error!(
                "Lỗi parse JSON SensorData từ thiết bị {}: {:?}",
                device_id, e
            );
            return;
        }
    };

    // 🟢 ĐÃ SỬA: Convert từ số millis (u64) sang chuỗi ISO 8601
    let time = incoming
        .timestamp_ms
        .and_then(|ms| chrono::DateTime::from_timestamp_millis(ms as i64))
        .unwrap_or_else(|| chrono::Utc::now())
        .to_rfc3339();

    // Khởi tạo struct SensorData chuẩn của toàn hệ thống
    let sensor_data = SensorData {
        device_id: device_id.clone(),
        temp_value: incoming.temp.unwrap_or(0.0),
        ec_value: incoming.ec.unwrap_or(0.0),
        ph_value: incoming.ph.unwrap_or(0.0),
        water_level: incoming.water_level.unwrap_or(0.0),
        pump_status: incoming.pump_status.unwrap_or_default(),
        time,
    };

    debug!(
        "Nhận dữ liệu cảm biến từ {}: ph={:.2}, ec={:.2}",
        device_id, sensor_data.ph_value, sensor_data.ec_value
    );

    // Ghi dữ liệu vào InfluxDB
    if let Err(e) = write_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &sensor_data,
    )
    .await
    {
        error!("Lỗi lưu SensorData vào InfluxDB ({}): {:?}", device_id, e);
    }

    // 🟢 ĐÃ SỬA: Đóng gói JSON chuẩn cấu trúc cho Tauri
    // Dùng key "payload" và ném nguyên cục sensor_data vào để Tauri mapping vào struct SensorData
    let ws_msg = json!({
        "type": "sensor_update",
        "payload": sensor_data
    });

    let _ = app_state.sensor_sender.send(sensor_data);
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

    let ws_msg = json!({
        "type": "device_status",
        "device_id": device_id,
        "online": status.online
    });

    let _ = app_state.alert_sender.send(ws_msg.to_string());
}

async fn handle_fsm_state(device_id: String, payload: &[u8], app_state: web::Data<AppState>) {
    let fsm_data: FsmPayload = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(e) => {
            error!("Lỗi parse FsmPayload từ {}: {:?}", device_id, e);
            return;
        }
    };

    let new_state = fsm_data.current_state;
    info!(
        "ESP32 [{}] vừa chuyển sang trạng thái FSM: {}",
        device_id, new_state
    );

    {
        let mut states = app_state.device_states.write().await;
        states.insert(device_id.clone(), new_state.clone());
    }

    let ws_msg = json!({
        "type": "fsm_update",
        "device_id": device_id,
        "current_state": new_state
    });
    let _ = app_state.alert_sender.send(ws_msg.to_string());
}

async fn handle_dosing_report(device_id: String, payload: &[u8], app_state: web::Data<AppState>) {
    let report: DosingReportPayload = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(e) => {
            error!("Lỗi parse DosingReport từ {}: {:?}", device_id, e);
            return;
        }
    };

    info!(
        "🌿 [{}] Báo cáo châm phân: A: {:.2}ml, B: {:.2}ml. Đang ghi lên Blockchain...",
        device_id, report.pump_a_ml, report.pump_b_ml
    );

    let blockchain_payload = json!({
        "device_id": device_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "dosing_data": report
    });

    let payload_str = blockchain_payload.to_string();

    match app_state
        .solana_traceability
        .record_dosing_history(&payload_str)
        .await
    {
        Ok(tx_id) => {
            info!("✅ Đã ghi lên Solana thành công! TxID: {}", tx_id);
            let ws_msg = json!({
                "type": "blockchain_verified",
                "device_id": device_id,
                "tx_id": tx_id,
                "explorer_url": format!("https://solscan.io/tx/{}?cluster=devnet", tx_id)
            });
            let _ = app_state.alert_sender.send(ws_msg.to_string());
        }
        Err(e) => {
            error!("❌ Lỗi ghi Blockchain cho {}: {:?}", device_id, e);
        }
    }
}
