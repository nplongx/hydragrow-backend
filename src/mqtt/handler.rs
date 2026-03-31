use actix_web::web;
use rumqttc::Publish;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

use crate::AppState; // Cấu trúc AppState sẽ được định nghĩa ở main.rs
use crate::db::influx::write_sensor_data;
use crate::models::sensor::SensorData;
use crate::services::tuya::send_tuya_command;

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

#[instrument(skip(app_state, publish))]
pub async fn process_message(publish: Publish, app_state: web::Data<AppState>) {
    let topic = publish.topic.clone();
    let payload_bytes = publish.payload;

    // Phân tích topic: AGITECH/{device_id}/{action}
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

    if let Err(e) = write_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &sensor_data,
    )
    .await
    {
        error!("Lỗi lưu SensorData vào InfluxDB ({}): {:?}", device_id, e);
    }

    // 3. (Optional) Gọi service Alert để check SafetyConfig và trigger cảnh báo
    // crate::services::alert::check_safety_rules(&sensor_data, &app_state).await;

    let ws_msg = json!({
        "type": "sensor_update",
        "device_id": device_id,
        "data": sensor_data
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
        let mut states = app_state.device_states.write().await; // Lock Write
        states.insert(device_id.clone(), new_state.clone());
    }

    // if new_state != "Monitoring" {
    //     warn!(
    //         "🚨 Trạng thái '{}' không an toàn cho bơm Tuần Hoàn! Yêu cầu ngắt khẩn cấp...",
    //         new_state
    //     );
    //
    //     match send_tuya_command(false).await {
    //         Ok(_) => {
    //             info!(
    //                 "✅ Đã ngắt bơm Tuya thành công để bảo vệ rễ cây khỏi nồng độ EC chưa ổn định!"
    //             );
    //         }
    //         Err(e) => {
    //             error!("❌ LỖI KHẨN CẤP: Không thể ngắt bơm Tuya: {:?}", e);
    //         }
    //     }
    // } else {
    //     info!(
    //         "Trạng thái '{}' an toàn. Chờ Scheduler quyết định lịch bơm.",
    //         new_state
    //     );
    // }

    let ws_msg = json!({
        "type": "fsm_update",
        "device_id": device_id,
        "current_state": new_state
    });
    let _ = app_state.alert_sender.send(ws_msg.to_string());
}

async fn handle_dosing_report(device_id: String, payload: &[u8], app_state: web::Data<AppState>) {
    // 1. Parse dữ liệu ESP32 gửi lên
    let report: DosingReportPayload = match serde_json::from_slice(payload) {
        Ok(data) => data,
        Err(e) => {
            error!("Lỗi parse DosingReport từ {}: {:?}", device_id, e);
            return;
        }
    };

    info!(
        "🌿 [{}] Báo cáo châm phân: A: {}ml, B: {}ml. Đang ghi lên Blockchain...",
        device_id, report.pump_a_ml, report.pump_b_ml
    );

    // 2. Tạo Payload hoàn chỉnh để ghi lên Blockchain (Thêm timestamp từ Server cho chuẩn xác)
    let blockchain_payload = json!({
        "device_id": device_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "dosing_data": report
    });

    let payload_str = blockchain_payload.to_string();

    // 3. GỌI HÀM SOLANA TẠI ĐÂY
    match app_state
        .solana_traceability
        .record_dosing_history(&payload_str)
        .await
    {
        Ok(tx_id) => {
            info!("✅ Đã ghi lên Solana thành công! TxID: {}", tx_id);
            info!(
                "🔍 Kiểm tra tại: https://solscan.io/tx/{}?cluster=devnet",
                tx_id
            );

            // Tùy chọn: Gửi TxID về Frontend qua WebSocket để hiện thông báo Xanh lá cây "Đã xác thực"
            let ws_msg = json!({
                "type": "blockchain_verified",
                "device_id": device_id,
                "tx_id": tx_id,
                "explorer_url": format!("https://solscan.io/tx/{}?cluster=devnet", tx_id)
            });
            let _ = app_state.alert_sender.send(ws_msg.to_string());

            // Tùy chọn 2: Bạn có thể INSERT thêm `tx_id` vào CSDL SQLite ở đây để lưu lịch sử
        }
        Err(e) => {
            error!("❌ Lỗi ghi Blockchain cho {}: {:?}", device_id, e);
        }
    }
}
