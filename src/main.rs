use actix_web::{App, HttpServer, web};
use anyhow::Context;
use dotenvy::dotenv;
use influxdb2::Client as InfluxClient;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use sqlx::sqlite::SqlitePoolOptions;
use std::{env, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

// Khai báo các module trong dự án (tương ứng với cấu trúc thư mục)
pub mod api;
pub mod db;
pub mod models;
pub mod mqtt;
pub mod services;

#[derive(Serialize)]
pub struct AlertMessage {
    pub alert_type: String,
    pub device_id: String,
    pub metric: String,
    pub value: f64,
    pub severity: String,
    pub timestamp: String,
}

pub struct AppState {
    pub sqlite_pool: sqlx::SqlitePool,
    pub influx_client: InfluxClient,
    pub influx_bucket: String,
    pub mqtt_client: AsyncClient,
    pub api_key: String,
    pub alert_sender: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Load biến môi trường từ file .env
    dotenv().ok();

    // 2. Khởi tạo hệ thống Log (Tracing) cực kỳ mạnh mẽ
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Lỗi khởi tạo tracing");
    info!("Bắt đầu khởi động hệ thống IoT Hydroponics...");

    // 3. Khởi tạo kết nối SQLite với Connection Pool
    let database_url = env::var("DATABASE_URL").expect("Thiếu biến DATABASE_URL trong .env");
    let sqlite_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    info!("Đã kết nối thành công tới SQLite");

    // chạy file .sql tạo bảng nếu DB chưa có.
    sqlx::migrate!().run(&sqlite_pool).await?;

    // 4. Khởi tạo kết nối InfluxDB
    let influx_url = env::var("INFLUX_URL").expect("Thiếu biến INFLUX_URL");
    let influx_org = env::var("INFLUX_ORG").expect("Thiếu biến INFLUX_ORG");
    let influx_token = env::var("INFLUX_TOKEN").expect("Thiếu biến INFLUX_TOKEN");
    let influx_bucket = env::var("INFLUX_BUCKET").expect("Thiếu biến INFLUX_BUCKET");
    let influx_client = InfluxClient::new(influx_url, influx_org, influx_token);
    info!("Đã khởi tạo client InfluxDB");

    // 5. Cấu hình và khởi tạo MQTT Client
    let mqtt_host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
    let mqtt_port: u16 = env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".to_string())
        .parse()?;

    let mut mqttoptions = MqttOptions::new("rust_backend_server", mqtt_host, mqtt_port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    info!("Đã khởi tạo MQTT Client");

    // 6. Khởi tạo Broadcast Channel cho WebSocket (Sức chứa 100 message)
    let (alert_sender, _) = broadcast::channel(100);
    let api_key = std::env::var("API_KEY").context("API_KEY must be set in .env")?;
    // 7. Đóng gói tất cả vào AppState (dùng Arc để chia sẻ an toàn giữa các thread)
    let app_state = Arc::new(AppState {
        sqlite_pool,
        influx_client,
        influx_bucket,
        mqtt_client: mqtt_client.clone(),
        alert_sender,
        api_key,
    });

    // 8. Đăng ký nhận (Subscribe) các topic từ MQTT Broker
    mqtt_client
        .subscribe("hydro/+/sensors", QoS::AtMostOnce)
        .await?;
    mqtt_client
        .subscribe("hydro/+/status", QoS::AtLeastOnce)
        .await?;

    // 9. Chạy vòng lặp lắng nghe MQTT trong một background task (Bắt buộc dùng tokio::spawn)
    let app_state_for_mqtt = app_state.clone();
    tokio::spawn(async move {
        info!("Bắt đầu vòng lặp sự kiện MQTT dưới nền...");
        loop {
            match eventloop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) => {
                    // Chuyển việc xử lý message cho hàm process_message đã viết ở Bước 7
                    mqtt::handler::process_message(publish, app_state_for_mqtt.clone()).await;
                }
                Ok(_) => {} // Bỏ qua các sự kiện nội bộ khác của MQTT (Ping, Ack...)
                Err(e) => {
                    error!(
                        "Mất kết nối MQTT, thử lại sau 5 giây... Chi tiết lỗi: {:?}",
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    let api_key = env::var("API_KEY").expect("Bạn phải cấu hình biến API_KEY để bảo mật API");

    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let server_port: u16 = env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()?;

    info!(
        "🚀 API Server đang khởi chạy tại http://{}:{}",
        server_host, server_port
    );

    HttpServer::new(move || {
        let auth_middleware = api::middleware::auth::ApiKeyAuth::new(api_key.clone());
        let rate_limit_middleware = api::middleware::rate_limit::RateLimiter::new(60, 60);

        App::new()
            .app_data(web::Data::from(app_state.clone()))
            .wrap(rate_limit_middleware)
            .wrap(auth_middleware)
            .configure(api::control::init_routes)
            .configure(api::sensor::init_routes)
            .configure(api::ws::init_routes)
            .configure(api::config::init_routes)
    })
    .bind((server_host, server_port))?
    .run()
    .await?;

    Ok(())
}
