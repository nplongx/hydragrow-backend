use actix_web::{HttpResponse, Responder, web};
use rumqttc::QoS;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use crate::AppState;
use crate::db::influx::get_latest_sensor_data;
use crate::models::sensor::{ValveCommandReq, ValveState};
use crate::services::valve_guard::validate_valve_action;

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
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();
    let req_data = req.into_inner();

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

/// POST /api/devices/{device_id}/control/valve
#[instrument(skip(app_state, req))]
pub async fn control_valve(
    path: web::Path<String>,
    req: web::Json<ValveCommandReq>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();
    let req_data = req.into_inner();

    // 1. Lấy trạng thái cảm biến/bơm/van MỚI NHẤT từ InfluxDB
    let latest_data = match get_latest_sensor_data(
        &app_state.influx_client,
        &app_state.influx_bucket,
        &device_id,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => {
            error!(
                "Không thể lấy trạng thái thiết bị để check an toàn van: {:?}",
                e
            );
            return HttpResponse::InternalServerError().json(json!({
                "error": "Safety Check Failed",
                "message": "Không thể xác định trạng thái hiện tại của hệ thống để đảm bảo an toàn."
            }));
        }
    };

    // 2. SAFETY RULE: Đưa vào valve_guard kiểm tra TRƯỚC KHI thực thi
    if let Err(safety_error) = validate_valve_action(&req_data, &latest_data.pump_status) {
        warn!("Valve Guard đã chặn một lệnh nguy hiểm: {}", safety_error);

        // Bắn cảnh báo vi phạm qua WebSocket cho UI hiển thị ngay lập tức
        let ws_msg = json!({
            "type": "valve_conflict",
            "device_id": device_id,
            "message": safety_error.to_string()
        });
        let _ = app_state.alert_sender.send(ws_msg.to_string());

        // Trả về HTTP 409 Conflict cho client
        return HttpResponse::Conflict().json(json!({
            "error": "Conflict",
            "message": safety_error.to_string()
        }));
    }

    // 3. Chuẩn bị payload và gửi đi nếu qua được vòng bảo vệ
    let state_str = if req_data.action == ValveState::Open {
        "open"
    } else {
        "closed"
    };
    let command = MqttCommandPayload {
        action: "valve".to_string(),
        target: Some(req_data.valve.clone()),
        state: Some(state_str.to_string()),
        duration_sec: None,
    };

    if let Err(e) = publish_command(&app_state, &device_id, &command).await {
        error!("Lỗi gửi lệnh van qua MQTT: {:?}", e);
        return HttpResponse::InternalServerError()
            .json(json!({"error": "Không thể gửi lệnh xuống thiết bị"}));
    }

    info!(
        "Đã gửi lệnh van {} {} trên thiết bị {}",
        req_data.valve, state_str, device_id
    );
    HttpResponse::Ok().json(json!({"status": "success", "message": "Valve command sent"}))
}

/// Helper function để đẩy message lên MQTT Broker
async fn publish_command(
    app_state: &Arc<AppState>,
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
        web::scope("/api/devices/{device_id}/control")
            .route("", web::post().to(control_pump))
            .route("/valve", web::post().to(control_valve)),
    );
}

