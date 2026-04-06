use actix_web::web;
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

// 🟢 ĐÃ SỬA: Bổ sung pump_status vào struct trung gian
#[derive(Debug, Deserialize)]
pub struct IncomingSensorPayload {
    pub temp: Option<f64>,
    pub ec: Option<f64>,
    pub ph: Option<f64>,
    pub water_level: Option<f64>,
    pub timestamp_ms: Option<u64>,
    pub pump_status: Option<PumpStatus>, // Hứng trạng thái bơm từ ESP32
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
    // 1. Parse JSON khuyết bằng IncomingSensorPayload
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

    let current_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let sensor_data = SensorData {
        device_id: device_id.clone(),
        temp_value: incoming.temp.unwrap_or(0.0),
        ec_value: incoming.ec.unwrap_or(0.0),
        ph_value: incoming.ph.unwrap_or(0.0),
        water_level: incoming.water_level.unwrap_or(0.0),
        pump_status: incoming.pump_status.unwrap_or_default(), // Lấy trạng thái bơm, nếu không có thì mặc định false
        timestamp: incoming
            .timestamp_ms
            .unwrap_or(current_ms as u64)
            .to_string(),
    };

    // 🟢 ĐÃ SỬA: unwrap_or(0) thành unwrap_or(0.0) để đúng kiểu float (f32)
    debug!(
        "Nhận dữ liệu cảm biến từ {}: ph={:.2}, ec={:.2}",
        device_id,
        incoming.ph.unwrap_or(0.0),
        incoming.ec.unwrap_or(0.0)
    );

    // 3. Ghi dữ liệu vào InfluxDB
    if let Err(e) = write_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &sensor_data, // Truyền struct chuẩn vào DB Influx
    )
    .await
    {
        error!("Lỗi lưu SensorData vào InfluxDB ({}): {:?}", device_id, e);
    }

    // 4. 🟢 ĐÃ SỬA: Bắn cả pump_status qua WebSocket cho Frontend
    let ws_msg = json!({
        "type": "sensor_update",
        "device_id": device_id,
        "data": {
            "temp": incoming.temp,
            "ec": incoming.ec,
            "ph": incoming.ph,
            "water_level": incoming.water_level,
            "pump_status": sensor_data.pump_status // Frontend sẽ dùng cái này để sáng/tắt đèn báo bơm
        }
    });

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
