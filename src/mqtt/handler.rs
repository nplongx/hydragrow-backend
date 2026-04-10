use std::time::UNIX_EPOCH;

use actix_web::web;
use chrono::{DateTime, Utc};
use rumqttc::Publish;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info, instrument, warn};

use crate::AppState;
use crate::db::influx::write_sensor_data;
use crate::models::alert::AlertMessage;
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
    pub online: bool,
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

    let time = incoming
        .timestamp_ms
        .and_then(|ms| chrono::DateTime::from_timestamp_millis(ms as i64))
        .unwrap_or_else(|| chrono::Utc::now())
        .to_rfc3339();

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

    if let Err(e) = write_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &sensor_data,
    )
    .await
    {
        error!("Lỗi lưu SensorData vào InfluxDB ({}): {:?}", device_id, e);
    }

    // Gửi sang luồng WebSocket (ws.rs sẽ tự động bọc {"type": "sensor_update"} cho Frontend)
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

    // 🟢 SỬA THÀNH ALERT MESSAGE CHUẨN ĐỂ WS.RS XỬ LÝ KHÔNG BỊ LỖI TYPE
    let alert = AlertMessage {
        level: if status.online {
            "success".to_string()
        } else {
            "warning".to_string()
        },
        title: "Trạng thái thiết bị".to_string(),
        message: format!(
            "Thiết bị {} vừa {}",
            device_id,
            if status.online {
                "Trực tuyến"
            } else {
                "Mất kết nối"
            }
        ),
        device_id: device_id.clone(),
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    };

    let _ = app_state.alert_sender.send(alert);
}

async fn handle_fsm_state(device_id: String, payload: &[u8], app_state: web::Data<AppState>) {
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&payload) {
        if let Some(state) = json["current_state"].as_str() {
            let mut alert = None;

            // 1. Phân tích loại lỗi
            if state.starts_with("SystemFault:") {
                let reason = state.replace("SystemFault:", "");
                alert = Some(AlertMessage {
                    level: "critical".to_string(),
                    title: "Lỗi Hệ Thống!".to_string(),
                    message: format!("Phát hiện lỗi phần cứng: {}. Vui lòng kiểm tra!", reason),
                    device_id: device_id.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                });
            } else if state == "EmergencyStop" {
                alert = Some(AlertMessage {
                    level: "critical".to_string(),
                    title: "Dừng Khẩn Cấp!".to_string(),
                    message: "Hệ thống đã bị ngắt khẩn cấp do vi phạm ngưỡng an toàn.".to_string(),
                    device_id: device_id.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                });
            }

            // 2. Nếu có lỗi, xử lý 2 luồng: WebSocket (React) & FCM (Android)
            if let Some(alert_msg) = alert {
                // -> Gửi lên React App đang mở trên màn hình
                let _ = app_state.alert_sender.send(alert_msg.clone());

                // -> Gửi lên Firebase Cloud Messaging (Chạy nền không block MQTT)
                let tokens = app_state.fcm_tokens.lock().unwrap().clone();
                if !tokens.is_empty() {
                    tokio::spawn(async move {
                        crate::services::fcm::send_push_notification(
                            &alert_msg.title,
                            &alert_msg.message,
                            tokens,
                        )
                        .await;
                    });
                }
            }
        }
    }
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

            // 🟢 SỬA THÀNH ALERT MESSAGE ĐỂ BÁO VỀ UI REACT
            let alert = AlertMessage {
                level: "success".to_string(),
                title: "Ghi Blockchain Thành Công".to_string(),
                message: format!(
                    "Mẻ phân bón đã được lưu trữ vĩnh viễn trên Solana.\nTxID: {}",
                    tx_id
                ),
                device_id: device_id.clone(),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            };
            let _ = app_state.alert_sender.send(alert);
        }
        Err(e) => {
            error!("❌ Lỗi ghi Blockchain cho {}: {:?}", device_id, e);

            let alert = AlertMessage {
                level: "warning".to_string(),
                title: "Lỗi Ghi Blockchain".to_string(),
                message: format!(
                    "Mẻ phân bón hoàn tất nhưng không thể đồng bộ Solana. Lỗi: {:?}",
                    e
                ),
                device_id: device_id.clone(),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            };
            let _ = app_state.alert_sender.send(alert);
        }
    }
}
