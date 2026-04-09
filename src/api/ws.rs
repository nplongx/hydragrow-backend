use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_ws::Message;
use futures_util::StreamExt as _;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tracing::{info, warn};

use crate::AppState;

pub async fn ws_handler(
    req: HttpRequest,
    body: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    let mut alert_rx = app_state.alert_sender.subscribe();
    let mut sensor_rx = app_state.sensor_sender.subscribe();

    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    info!(
        "New WebSocket connection established from IP: {}",
        client_ip
    );

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                alert_result = alert_rx.recv() => {
                    match alert_result {
                        Ok(alert_msg) => {
                            // 🟢 Đóng gói JSON chuẩn cho Alert
                            let ws_msg = serde_json::json!({
                                "type": "alert",
                                "payload": alert_msg
                            });

                            if let Ok(json_str) = serde_json::to_string(&ws_msg) {
                                if session.text(json_str).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(RecvError::Lagged(_)) => {
                            warn!("WS Client {} is too slow, missed some alerts", client_ip);
                        }
                        Err(RecvError::Closed) => {
                            break;
                        }
                    }
                }
                // Luồng nghe Sensor Data
                sensor_result = sensor_rx.recv() => {
                    match sensor_result {
                        Ok(sensor_data) => {
                            // Gói đúng chuẩn cho Tauri
                            let ws_msg = serde_json::json!({
                                "type": "sensor_update",
                                "payload": sensor_data
                            });

                            // Gửi đi 1 lần duy nhất
                            if let Ok(json_str) = serde_json::to_string(&ws_msg) {
                                if session.text(json_str).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }

                Some(Ok(msg)) = msg_stream.next() => {
                    match msg {
                        Message::Ping(bytes) => {
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(reason) => {
                            let _ = session.close(reason).await;
                            break;
                        }
                        _ => {}
                    }
                }

                else => break,
            }
        }
        info!("WebSocket connection closed for IP: {}", client_ip);
    });

    Ok(response)
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/ws", web::get().to(ws_handler));
}
