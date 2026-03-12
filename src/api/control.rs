use crate::{
    AppState,
    services::command::{CommandPayload, send_command},
};
use actix_web::{HttpResponse, Responder, web};
use serde_json::json;
use std::sync::Arc;

pub async fn manual_control(
    path: web::Path<String>,
    payload: web::Json<CommandPayload>,
    app_state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let device_id = path.into_inner();
    let command = payload.into_inner();

    match send_command(&app_state, &device_id, &command).await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": format!("Command '{}' published to device {}", command.action, device_id)
        })),
        Err(e) => {
            tracing::error!("Manual control failed: {:?}", e);
            HttpResponse::InternalServerError()
                .json(json!({"error": "Failed to send command to device"}))
        }
    }
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    // Gom nhóm route lại
    cfg.service(
        web::scope("/api/devices").route("/{device_id}/control", web::post().to(manual_control)),
    );
}

