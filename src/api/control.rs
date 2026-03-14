use actix_web::{HttpResponse, Responder, web};
use rumqttc::QoS;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use crate::AppState;
use crate::db::influx::get_latest_sensor_data;
use crate::models::sensor::{ValveCommandReq, ValveState};
use crate::services::tuya;
// use crate::services::valve_guard::validate_valve_action;

/// Request body cho API điều khiển bơm
#[derive(Debug, Deserialize)]
pub struct PumpControlReq {
    pub pump: String,   // "A", "B", "PH_UP", "PH_DOWN", "CIRCULATION", "WATER_PUMP"
    pub action: String, // "on" hoặc "off"
    pub duration_sec: Option<u64>,
}

/// Payload MQTT nội bộ để Serialize thành JSON gửi xuống ESP32
#[derive(Serialize)]
struct MqttCommandPayload {
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_sec: Option<u64>,
}

/// POST /api/devices/{device_id}/control
#[instrument(skip(app_state, req))]
pub async fn control_pump(
    path: web::Path<String>,
    req: web::Json<PumpControlReq>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let device_id = path.into_inner();
    let req_data = req.into_inner();

    if req_data.pump == "CIRCULATION" {
        let is_on = req_data.action == "on";

        match tuya::send_tuya_command(is_on).await {
            Ok(_) => {
                info!(
                    "Đã {} bơm tuần hoàn qua Tuya API",
                    if is_on { "BẬT" } else { "TẮT" }
                );
                return HttpResponse::Ok().json(json!({"status": "success"}));
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(json!({"error": e.to_string()}));
            }
        }
    }

    // 1. Chuẩn bị payload theo đúng format phần cứng yêu cầu
    let mqtt_action = if req_data.action == "on" {
        "pump_on"
    } else {
        "pump_off"
    };

    let command = MqttCommandPayload {
        action: mqtt_action.to_string(),
        target: Some(req_data.pump.clone()),
        state: None,
        duration_sec: req_data.duration_sec,
    };

    // 2. Publish MQTT
    if let Err(e) = publish_command(&app_state, &device_id, &command).await {
        error!("Lỗi gửi lệnh bơm qua MQTT: {:?}", e);
        return HttpResponse::InternalServerError()
            .json(json!({"error": "Không thể gửi lệnh xuống thiết bị"}));
    }

    info!(
        "Đã gửi lệnh {} cho bơm {} trên thiết bị {}",
        req_data.action, req_data.pump, device_id
    );
    HttpResponse::Ok().json(json!({"status": "success", "message": "Command sent"}))
}

/// Helper function để đẩy message lên MQTT Broker
async fn publish_command(
    app_state: &AppState,
    device_id: &str,
    payload: &MqttCommandPayload,
) -> anyhow::Result<()> {
    let topic = format!("hydro/{}/command", device_id);
    let payload_bytes = serde_json::to_vec(payload)?;

    // QoS 1 (AtLeastOnce) đảm bảo message không bị rớt dọc đường. Retain = false cho các lệnh chạy ngay.
    app_state
        .mqtt_client
        .publish(topic, QoS::AtLeastOnce, false, payload_bytes)
        .await?;

    Ok(())
}

// Khởi tạo router
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/devices/{device_id}/control").route("", web::post().to(control_pump)),
    );
}
