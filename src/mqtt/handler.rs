use std::any::Any;
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

// Trong src/mqtt/handler.rs (Backend)
#[derive(Debug, Deserialize)]
pub struct IncomingSensorPayload {
    pub temp: Option<f64>,

    pub ec: Option<f64>,

    pub ph: Option<f64>,

    pub water_level: Option<f64>,

    // SỬA Ở ĐÂY: map JSON key "last_update_ms" của ESP32 vào biến này
    #[serde(rename = "last_update_ms")]
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
    // 🟢 1. In ra RAW DATA ngay lập tức để chắc chắn Backend đã nhận được tin nhắn
    let raw_payload = std::str::from_utf8(payload).unwrap_or("Lỗi UTF-8");
    info!("📥 [MQTT-FSM] {} gửi gói tin: {}", device_id, raw_payload);

    // 🟢 2. Bắt lỗi Parse JSON rõ ràng
    match serde_json::from_slice::<serde_json::Value>(payload) {
        Ok(json) => {
            // 🟢 3. Kiểm tra xem có trường "current_state" không
            if let Some(state) = json["current_state"].as_str() {
                let fsm_sync_msg = AlertMessage {
                    level: "FSM_UPDATE".to_string(),
                    title: "FSM_SYNC".to_string(),
                    message: state.to_string(), // Ví dụ: "SystemFault:SENSOR_DISCONNECTED"
                    device_id: device_id.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                };
                let _ = app_state.alert_sender.send(fsm_sync_msg);

                let mut alert = None;

                // 🟢 4. Phân tích chi tiết các trạng thái từ ESP32
                match state {
                    // --- CÁC TRẠNG THÁI MỚI BỔ SUNG ---
                    "SystemBooting" => {
                        alert = Some(AlertMessage {
                            level: "success".to_string(),
                            title: "Khởi Động Hệ Thống".to_string(),
                            message: "Trạm điều khiển vừa được cấp nguồn và đang hoạt động."
                                .to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "ManualMode" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Điều Khiển Thủ Công".to_string(),
                            message: "Đang ở chế độ Manual (Thủ công). Hệ thống tắt tự động hóa."
                                .to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "CleaningMode" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Chế Độ Súc Rửa".to_string(),
                            message: "Đang chạy chu trình súc rửa bồn chứa.".to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "SensorCalibrating" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Hiệu Chuẩn Cảm Biến".to_string(),
                            message: "Hệ thống đang ở chế độ hiệu chuẩn đầu dò cảm biến."
                                .to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "DosingCycleComplete" => {
                        alert = Some(AlertMessage {
                            level: "success".to_string(),
                            title: "Hoàn Tất Chu Trình".to_string(),
                            message: "Chu trình châm phân & điều chỉnh pH đã hoàn thành."
                                .to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    s if s.starts_with("Warning:") => {
                        let reason = s.replace("Warning:", "");
                        alert = Some(AlertMessage {
                            level: "warning".to_string(), // Frontend sẽ hiển thị vàng/cam
                            title: "Cảnh Báo Hệ Thống".to_string(),
                            message: format!("Phát hiện cảnh báo: {}", reason),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    s if s.starts_with("LogInfo:") => {
                        let msg = s.replace("LogInfo:", "");
                        alert = Some(AlertMessage {
                            level: "info".to_string(), // Frontend hiển thị xanh dương
                            title: "Nhật Ký (Log)".to_string(),
                            message: msg,
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }

                    // --- CÁC TRẠNG THÁI CŨ GIỮ NGUYÊN ---
                    "EmergencyStop" => {
                        alert = Some(AlertMessage {
                            level: "critical".to_string(),
                            title: "Dừng Khẩn Cấp!".to_string(),
                            message: "Hệ thống đã bị ngắt khẩn cấp do vi phạm ngưỡng an toàn."
                                .to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    s if s.starts_with("SystemFault:") => {
                        let reason = s.replace("SystemFault:", "");
                        alert = Some(AlertMessage {
                            level: "critical".to_string(),
                            title: "Lỗi Hệ Thống!".to_string(),
                            message: format!(
                                "Phát hiện lỗi phần cứng: {}. Vui lòng kiểm tra!",
                                reason
                            ),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "WaterRefilling" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Cấp Nước".to_string(),
                            message: "Hệ thống đang tiến hành bơm cấp nước vào bồn.".to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "WaterDraining" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Xả Nước".to_string(),
                            message: "Hệ thống đang xả bớt nước trong bồn.".to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "DosingPumpA" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Châm Phân".to_string(),
                            message: "Đang tiến hành châm phân bón Dinh Dưỡng A.".to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "DosingPumpB" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Châm Phân".to_string(),
                            message: "Đang tiến hành châm phân bón Dinh Dưỡng B.".to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "DosingPH" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Điều Chỉnh pH".to_string(),
                            message: "Đang tiến hành bơm dung dịch điều chỉnh pH.".to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    "ActiveMixing" => {
                        alert = Some(AlertMessage {
                            level: "info".to_string(),
                            title: "Sục Trộn Dinh Dưỡng".to_string(),
                            message: "Đang trộn đều dung dịch trong bồn (Jet Mixing).".to_string(),
                            device_id: device_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
                    }
                    // Các trạng thái quá độ/chuyển tiếp ngắn, chỉ in log Backend để tránh spam UI Frontend
                    "StartingOsakaPump" => {
                        debug!("Bắt đầu khởi động bơm trung tâm (Osaka).");
                    }
                    "WaitingBetweenDose" => {
                        debug!("Đang chờ hòa tan giữa 2 lần châm A và B.");
                    }
                    "Stabilizing" => {
                        debug!("Đang chờ cảm biến đọc số liệu ổn định.");
                    }
                    "Monitoring" => {
                        debug!("Hệ thống đang ở trạng thái giám sát (Monitoring).");
                    }
                    _ => {
                        debug!("Trạng thái FSM khác: {}", state);
                    }
                }

                // 🟢 5. Xử lý phân luồng thông báo (UI vs Mobile Push)
                if let Some(alert_msg) = alert {
                    if alert_msg.level == "critical" || alert_msg.level == "warning" {
                        info!("🚨 KÍCH HOẠT BÁO ĐỘNG: {}", alert_msg.title);
                    } else {
                        info!("ℹ️ THAY ĐỔI TRẠNG THÁI: {}", alert_msg.title);
                    }

                    // Luôn gửi lên WebSocket (Cho React Web hiển thị Toast/Timeline)
                    let _ = app_state.alert_sender.send(alert_msg.clone());

                    // CHỈ gửi Push Notification tới Mobile nếu là lỗi (Tránh spam thông báo điện thoại)
                    if alert_msg.level == "critical" || alert_msg.level == "warning" {
                        let tokens = app_state.fcm_tokens.lock().unwrap().clone();
                        if tokens.is_empty() {
                            warn!(
                                "⚠️ Không có FCM Token nào trong RAM! Không thể gửi Push Notification tới điện thoại."
                            );
                        } else {
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
            } else {
                error!("❌ [MQTT-FSM] JSON hợp lệ nhưng bị thiếu trường 'current_state'!");
            }
        }
        Err(e) => {
            error!("❌ [MQTT-FSM] Cấu trúc JSON bị sai định dạng: {:?}", e);
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

    let season_id_str =
        match crate::db::postgres::get_active_crop_season(&app_state.pg_pool, &device_id).await {
            Ok(season) => season.unwrap().id.to_string(),
            Err(_) => "".to_string(), // Nếu không có mùa vụ nào đang chạy, để trống
        };

    let blockchain_payload = json!({
        "device_id": device_id,
        "season_id": season_id_str,
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

            // 🟢 BƯỚC 3: LƯU VÀO DATABASE ĐỂ TRANG "NIÊM PHONG" CÓ THỂ ĐỌC ĐƯỢC
            let action_str = format!(
                "Châm phân tự động: A({:.1}ml), B({:.1}ml)",
                report.pump_a_ml, report.pump_b_ml
            );

            if let Err(db_err) = crate::db::postgres::insert_blockchain_tx(
                &app_state.pg_pool,
                &device_id,
                &season_id_str,
                &action_str,
                &tx_id,
            )
            .await
            {
                error!("❌ Lỗi lưu TxID vào SQLite: {:?}", db_err);
            }

            // Gửi Alert lên Frontend (Đã có sẵn)
            let alert = AlertMessage {
                level: "success".to_string(),
                title: "Ghi Blockchain Thành Công".to_string(),
                message: format!(
                    "Đã bơm: Phân A: {:.1}ml | Phân B: {:.1}ml | pH Up: {:.1}ml | pH Down: {:.1}ml\nTxID Solana: {}",
                    report.pump_a_ml, report.pump_b_ml, report.ph_up_ml, report.ph_down_ml, tx_id
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
