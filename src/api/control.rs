use actix_web::{HttpResponse, Responder, web};
use rumqttc::QoS;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info, instrument, warn};

use crate::AppState;
use crate::services::tuya; // Giả sử file tuya nằm trong folder services

#[derive(Debug, Deserialize)]
pub struct PumpControlReq {
    pub pump: String, // "A", "B", "PH_UP", "PH_DOWN", "OSAKA_PUMP", "MIST_VALVE", "WATER_PUMP", "DRAIN_PUMP", "CIRCULATION_PUMP", "ALL"
    pub action: String, // "on", "off", "reset_fault", "set_pwm"
    pub duration_sec: Option<u64>,
    pub pwm: Option<u32>,
}

#[derive(Debug, Serialize)]
struct MqttCommandPayload {
    pub action: String,
    pub pump: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pwm: Option<u32>,
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

    let valid_pumps = [
        "A",
        "B",
        "PH_UP",
        "PH_DOWN",
        "OSAKA_PUMP",
        "MIST_VALVE",
        "WATER_PUMP",
        "DRAIN_PUMP",
        "CIRCULATION_PUMP", // 🟢 Bơm tuần hoàn điều khiển qua Tuya
        "ALL",
    ];

    if !valid_pumps.contains(&req_data.pump.as_str()) {
        warn!("Từ chối lệnh: Tên bơm/van không hợp lệ ({})", req_data.pump);
        return HttpResponse::BadRequest().json(json!({"error": "Invalid pump name"}));
    }

    let valid_actions = ["on", "off", "reset_fault", "set_pwm"];
    if !valid_actions.contains(&req_data.action.as_str()) {
        warn!("Từ chối lệnh: Hành động không hợp lệ ({})", req_data.action);
        return HttpResponse::BadRequest()
            .json(json!({"error": "Action must be 'on', 'off', 'reset_fault', or 'set_pwm'"}));
    }

    // 🟢 Xử lý riêng cho CIRCULATION_PUMP qua Tuya Cloud
    if req_data.pump == "CIRCULATION_PUMP" {
        let turn_on = match req_data.action.as_str() {
            "on" => true,
            "off" => false,
            _ => {
                return HttpResponse::BadRequest()
                    .json(json!({"error": "Circulation pump only supports on/off"}));
            }
        };

        if let Err(e) = tuya::send_tuya_command(turn_on).await {
            error!("Lỗi gửi lệnh Tuya: {:?}", e);
            return HttpResponse::InternalServerError()
                .json(json!({"error": format!("Tuya Error: {}", e)}));
        }

        info!("Đã gửi lệnh Tuya {} cho CIRCULATION_PUMP", req_data.action);
        return HttpResponse::Ok()
            .json(json!({"status": "success", "message": "Tuya command sent"}));
    }

    // Xử lý các bơm khác qua MQTT
    let mqtt_action = match req_data.action.as_str() {
        "on" => "pump_on",
        "off" => "pump_off",
        "reset_fault" => "reset_fault",
        "set_pwm" => "set_pwm",
        _ => "pump_off",
    };

    let command = MqttCommandPayload {
        action: mqtt_action.to_string(),
        pump: req_data.pump.clone(),
        duration_sec: req_data.duration_sec,
        pwm: req_data.pwm,
    };

    if let Err(e) = publish_command(&app_state, &device_id, &command).await {
        error!("Lỗi gửi lệnh qua MQTT: {:?}", e);
        return HttpResponse::InternalServerError()
            .json(json!({"error": "Không thể gửi lệnh xuống thiết bị"}));
    }

    info!(
        "Đã gửi lệnh {} (pwm: {:?}) cho {} trên thiết bị {} qua MQTT",
        req_data.action, req_data.pwm, req_data.pump, device_id
    );

    HttpResponse::Ok().json(json!({"status": "success", "message": "Command sent"}))
}

async fn publish_command(
    app_state: &AppState,
    device_id: &str,
    payload: &MqttCommandPayload,
) -> anyhow::Result<()> {
    let topic = format!("AGITECH/{}/controller/command", device_id);
    let payload_bytes = serde_json::to_vec(payload)?;

    app_state
        .mqtt_client
        .publish(topic, QoS::AtLeastOnce, false, payload_bytes)
        .await?;

    Ok(())
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/devices/{device_id}/control").route("", web::post().to(control_pump)),
    );
}

