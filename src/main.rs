use actix_web::{App, HttpResponse, HttpServer, web};
use anyhow::Context;
use dotenvy::dotenv;
use influxdb2::Client;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use sqlx::sqlite::SqlitePoolOptions;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

// Khai báo các module của dự án
pub mod api;
pub mod db;
pub mod models;
pub mod mqtt;
pub mod services;

/// Cấu trúc tin nhắn cảnh báo để gửi qua WebSocket
#[derive(Debug, Clone, serde::Serialize)]
pub struct AlertMessage {
    pub alert_type: String,
    pub device_id: String,
    pub metric: String,
    pub value: f64,
    pub severity: String,
}

/// Trạng thái ứng dụng dùng chung (Shared State)
pub struct AppState {
    pub sqlite_pool: sqlx::SqlitePool,
    pub influx_client: Client,
    pub influx_bucket: String,
    pub mqtt_client: AsyncClient,
    pub api_key: String,
    pub alert_sender: broadcast::Sender<AlertMessage>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // 1. Tải các biến môi trường từ file .env
    dotenv().ok();

    // 2. Khởi tạo hệ thống Logging (Tracing)
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Setting default subscriber failed");
    info!("Starting Hydroponics IoT Server...");

    // 3. Khởi tạo kết nối SQLite (Chứa cấu hình thiết bị)
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let sqlite_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create SQLite pool");
    info!("Connected to SQLite database.");

    // 4. Khởi tạo kết nối InfluxDB (Chứa dữ liệu cảm biến time-series)
    let influx_url = env::var("INFLUX_URL").expect("INFLUX_URL must be set");
    let influx_org = env::var("INFLUX_ORG").expect("INFLUX_ORG must be set");
    let influx_token = env::var("INFLUX_TOKEN").expect("INFLUX_TOKEN must be set");
    let influx_bucket = env::var("INFLUX_BUCKET").expect("INFLUX_BUCKET must be set");

    let influx_client = Client::new(influx_url, influx_org, influx_token);
    info!("Configured InfluxDB client.");

    // 5. Khởi tạo Broadcast Channel cho WebSocket Alerts
    // Sức chứa 100 tin nhắn (nếu client quá lag không nhận kịp, tin nhắn cũ sẽ bị drop)
    let (alert_sender, _alert_receiver) = broadcast::channel(100);

    // 6. Cấu hình và khởi tạo MQTT Client
    let mqtt_host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
    let mqtt_port = env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".to_string())
        .parse::<u16>()
        .expect("MQTT_PORT must be a number");
    let mqtt_client_id = env::var("MQTT_CLIENT_ID").unwrap_or_else(|_| "hydro_server".to_string());

    let mut mqttoptions = MqttOptions::new(mqtt_client_id, mqtt_host, mqtt_port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    // Khởi tạo AsyncClient của rumqttc
    let (mqtt_client, mut mqtt_eventloop) = AsyncClient::new(mqttoptions, 10);
    info!("MQTT Client initialized.");

    // 7. Tạo AppState (Đóng gói vào Arc để chia sẻ an toàn giữa các luồng)
    let api_key = std::env::var("API_KEY")
        .context("API_KEY must be set in .env")
        .unwrap();
    let app_state = Arc::new(AppState {
        sqlite_pool,
        influx_client,
        influx_bucket,
        mqtt_client: mqtt_client.clone(), // Clone client để gửi lệnh điều khiển
        api_key,
        alert_sender,
    });

    // 8. Chạy Background Task: Lắng nghe và xử lý sự kiện MQTT
    let app_state_for_mqtt = app_state.clone();

    // Subscribe vào các topic quan trọng ngay khi khởi động
    mqtt_client
        .subscribe("hydro/+/sensors", QoS::AtLeastOnce)
        .await
        .unwrap();
    mqtt_client
        .subscribe("hydro/+/status", QoS::AtLeastOnce)
        .await
        .unwrap();

    tokio::spawn(async move {
        info!("Started MQTT event loop task.");
        loop {
            match mqtt_eventloop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) => {
                    // Chuyển việc xử lý payload cho module handler
                    crate::mqtt::handler::process_message(publish, app_state_for_mqtt.clone())
                        .await;
                }
                Ok(_) => {
                    // Bỏ qua các sự kiện khác (Ping, Ack, v.v.)
                }
                Err(e) => {
                    error!("MQTT Connection Error: {:?}", e);
                    // Đợi 3 giây trước khi thử kết nối lại
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    });

    // 9. Khởi tạo và chạy Actix-web HTTP Server
    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let server_port = env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_address = format!("{}:{}", server_host, server_port);

    info!("Starting HTTP server at http://{}", bind_address);

    HttpServer::new(move || {
        App::new()
            // Truyền AppState vào Actix-web để các handler (endpoints) có thể gọi được
            .app_data(web::Data::new(app_state.clone()))
            // Gắn các routes (endpoints) từ module api
            .configure(api::sensor::init_routes)
            .configure(api::config::init_routes)
            .configure(api::control::init_routes)
            .configure(api::ws::init_routes)
            // Health check endpoint (dành cho Docker hoặc Load Balancer)
            .route(
                "/health",
                web::get().to(|| async { HttpResponse::Ok().body("OK") }),
            )
    })
    .bind(&bind_address)?
    .run()
    .await
}
