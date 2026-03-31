use actix_web::{HttpResponse, Responder, web};
use rumqttc::QoS;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info, instrument, warn};

use crate::AppState;
use crate::services::tuya;

#[derive(Debug, Deserialize)]
pub struct PumpControlReq {
    pub pump: String, // "A", "B", "PH_UP", "PH_DOWN", "CIRCULATION", "CHAMBER_PUMP", "WATER_PUMP"
    pub action: String, // "on" hoặc "off"
    pub duration_sec: Option<u64>,
}

#[derive(Debug, Serialize)]
struct MqttCommandPayload {
    pub action: String, // Firmware ESP32 sẽ nhận "pump_on" hoặc "pump_off"
    pub pump: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<u64>,
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
        "CIRCULATION",
        "CHAMBER_PUMP",
        "WATER_PUMP",
        "DRAIN_PUMP",
    ];

    if !valid_pumps.contains(&req_data.pump.as_str()) {
        warn!("Từ chối lệnh: Tên bơm không hợp lệ ({})", req_data.pump);
        return HttpResponse::BadRequest().json(json!({"error": "Invalid pump name"}));
    }

    if req_data.action != "on" && req_data.action != "off" {
        warn!("Từ chối lệnh: Hành động không hợp lệ ({})", req_data.action);
        return HttpResponse::BadRequest().json(json!({"error": "Action must be 'on' or 'off'"}));
    }

    if req_data.pump == "CIRCULATION" {
        let is_on = req_data.action == "on";
        // match tuya::send_tuya_command(is_on).await {
        //     Ok(_) => {
        //         info!(
        //             "Đã {} bơm tuần hoàn qua Tuya API cho thiết bị {}",
        //             if is_on { "BẬT" } else { "TẮT" },
        //             device_id
        //         );
        //         return HttpResponse::Ok().json(json!({"status": "success"}));
        //     }
        //     Err(e) => {
        //         error!("Lỗi Tuya API: {:?}", e);
        //         return HttpResponse::InternalServerError().json(json!({"error": e.to_string()}));
        //     }
        // }
    }

    let mqtt_action = if req_data.action == "on" {
        "pump_on"
    } else {
        "pump_off"
    };

    let command = MqttCommandPayload {
        action: mqtt_action.to_string(),
        pump: req_data.pump.clone(),
        duration_sec: req_data.duration_sec,
    };

    if let Err(e) = publish_command(&app_state, &device_id, &command).await {
        error!("Lỗi gửi lệnh bơm qua MQTT: {:?}", e);
        return HttpResponse::InternalServerError()
            .json(json!({"error": "Không thể gửi lệnh xuống thiết bị"}));
    }

    info!(
        "Đã gửi lệnh {} cho bơm {} trên thiết bị {} qua MQTT",
        req_data.action, req_data.pump, device_id
    );
    HttpResponse::Ok().json(json!({"status": "success", "message": "Command sent"}))
}

async fn publish_command(
    app_state: &AppState,
    device_id: &str,
    payload: &MqttCommandPayload,
) -> anyhow::Result<()> {
    let topic = format!("AGITECH/{}/command", device_id);
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
