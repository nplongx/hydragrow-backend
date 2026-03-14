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
    // Nâng cấp kết nối HTTP lên WebSocket
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    // Subscribe vào kênh phát sóng cảnh báo từ AppState
    let mut alert_rx = app_state.alert_sender.subscribe();
    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    info!(
        "New WebSocket connection established from IP: {}",
        client_ip
    );

    // Sinh ra một Tokio Task chạy ngầm để quản lý vòng đời của kết nối WS này
    actix_web::rt::spawn(async move {
        loop {
            // tokio::select! cho phép lắng nghe đồng thời 2 luồng sự kiện:
            // 1. Cảnh báo từ backend (alert_rx)
            // 2. Message/Ping-Pong từ client (msg_stream)
            tokio::select! {
                // Nhận cảnh báo từ Service Layer (đã code ở alert.rs)
                alert_result = alert_rx.recv() => {
                    match alert_result {
                        Ok(alert_msg) => {
                            if let Ok(json_str) = serde_json::to_string(&alert_msg) {
                                // Push JSON xuống Tauri App
                                if session.text(json_str).await.is_err() {
                                    break; // Client đã ngắt kết nối
                                }
                            }
                        }
                        Err(RecvError::Lagged(_)) => {
                            warn!("WS Client {} is too slow, missed some alerts", client_ip);
                        }
                        Err(RecvError::Closed) => {
                            break; // Backend tắt channel
                        }
                    }
                }

                // Nhận tín hiệu từ Tauri Client (Mobile App)
                Some(Ok(msg)) = msg_stream.next() => {
                    match msg {
                        Message::Ping(bytes) => {
                            // Phản hồi Ping để giữ kết nối không bị timeout
                            if session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(reason) => {
                            let _ = session.close(reason).await;
                            break;
                        }
                        _ => {} // Bỏ qua Text/Binary message từ client vì WS này chỉ dùng để Push (1 chiều)
                    }
                }

                // Client ngắt kết nối đột ngột
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
