use actix_web::{App, HttpServer, web};
use anyhow::Context;
use dotenvy::dotenv;
use influxdb2::Client as InfluxClient; // 🟢 QUAY LẠI DÙNG INFLUXDB2
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use sqlx::sqlite::SqlitePoolOptions;
use std::{
    collections::HashMap,
    env, fs,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{
    RwLock,
    broadcast::{self, Receiver},
};
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

use crate::{
    models::{alert::AlertMessage, sensor::SensorData},
    services::{scheduler::start_tuya_scheduler, solana::SolanaTraceability},
};

pub mod api;
pub mod db;
pub mod models;
pub mod mqtt;
pub mod services;
//
// #[derive(Serialize)]
// pub struct AlertMessage {
//     pub alert_type: String,
//     pub device_id: String,
//     pub metric: String,
//     pub value: f64,
//     pub severity: String,
//     pub timestamp: String,
// }

pub struct AppState {
    pub sqlite_pool: sqlx::SqlitePool,
    pub influx_client: InfluxClient,
    pub influx_bucket: String,
    pub mqtt_client: AsyncClient,
    pub api_key: String,
    pub alert_sender: broadcast::Sender<AlertMessage>,
    pub device_states: std::sync::Arc<RwLock<HashMap<String, String>>>,
    pub solana_traceability: SolanaTraceability,
    pub sensor_sender: broadcast::Sender<SensorData>,

    pub fcm_tokens: Mutex<Vec<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Lỗi khởi tạo tracing");
    info!("Bắt đầu khởi động hệ thống IoT Hydroponics...");

    let database_url = env::var("DATABASE_URL").expect("Thiếu biến DATABASE_URL");
    let sqlite_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // 🟢 KHỞI TẠO INFLUXDB2 CHO CLOUD
    let influx_url = env::var("INFLUX_URL").expect("Thiếu biến INFLUX_URL");
    let influx_org = env::var("INFLUX_ORG").expect("Thiếu biến INFLUX_ORG");
    let influx_token = env::var("INFLUX_TOKEN").expect("Thiếu biến INFLUX_TOKEN");
    let influx_bucket = env::var("INFLUX_BUCKET").expect("Thiếu biến INFLUX_BUCKET");
    let influx_client = InfluxClient::new(influx_url, influx_org, influx_token);
    info!("Đã khởi tạo client InfluxDB Cloud (v2 API)");

    let mqtt_host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
    let mqtt_port: u16 = env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".to_string())
        .parse()?;
    let mqtt_client_id =
        env::var("MQTT_CLIENT_ID").unwrap_or_else(|_| "rust_backend_server".to_string());

    let mut mqttoptions = MqttOptions::new(mqtt_client_id, mqtt_host, mqtt_port);
    mqttoptions.set_keep_alive(Duration::from_secs(30));
    mqttoptions.set_clean_session(false);

    // ==========================================
    // 🟢 ĐÃ THÊM: XÁC THỰC USERNAME/PASSWORD CHO MQTT
    // ==========================================
    let mqtt_user = env::var("MQTT_USER").unwrap_or_default();
    let mqtt_pass = env::var("MQTT_PASSWORD").unwrap_or_default();
    if !mqtt_user.is_empty() && !mqtt_pass.is_empty() {
        mqttoptions.set_credentials(&mqtt_user, mqtt_pass);
        info!("Đã cấu hình xác thực MQTT với user: {}", mqtt_user);
    } else {
        info!("Cảnh báo: Không tìm thấy MQTT_USER/MQTT_PASSWORD trong .env, kết nối ẩn danh.");
    }
    // ==========================================

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    let wallet_data =
        fs::read_to_string("server_wallet.json").expect("Không tìm thấy server_wallet.json");
    let private_key: Vec<u8> = serde_json::from_str(&wallet_data).unwrap();
    let solana_service = SolanaTraceability::new("https://api.devnet.solana.com", &private_key);
    let (alert_sender, _) = broadcast::channel(100);

    let mut alert_rx_for_db: Receiver<AlertMessage> = alert_sender.subscribe();
    let db_pool_clone = sqlite_pool.clone(); // Giả sử db_pool là biến SqlitePool của bạn

    tokio::spawn(async move {
        while let Ok(alert) = alert_rx_for_db.recv().await {
            // Không lưu các event FSM_UPDATE (vì nó spam 100ms/lần làm phình to DB)
            // Chỉ lưu các cảnh báo (Error, Info, Warning, Dosing Report...)
            if alert.level != "FSM_UPDATE" {
                if let Err(e) = crate::db::sqlite::insert_system_event(&db_pool_clone, &alert).await
                {
                    tracing::error!("Lỗi ghi System Event vào DB: {:?}", e);
                }
            }
        }
    });

    // 🟢 THÊM: Khởi tạo kênh truyền dữ liệu Sensor
    let (sensor_sender, _) = broadcast::channel(100);

    let api_key = std::env::var("API_KEY").context("API_KEY must be set in .env")?;
    let device_states = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

    let app_state = web::Data::new(AppState {
        sqlite_pool,
        influx_client,
        influx_bucket,
        mqtt_client: mqtt_client.clone(),
        alert_sender,
        api_key,
        device_states,
        solana_traceability: solana_service,
        // 🟢 THÊM: Đưa kênh sensor vào AppState
        sensor_sender,
        fcm_tokens: Mutex::new(Vec::new()),
    });

    mqtt_client
        .subscribe("AGITECH/+/sensors", QoS::AtMostOnce)
        .await
        .expect("Lỗi sub sensors");
    mqtt_client
        .subscribe("AGITECH/+/status", QoS::AtLeastOnce)
        .await
        .expect("Lỗi sub status");
    mqtt_client
        .subscribe("AGITECH/+/fsm", QoS::AtLeastOnce)
        .await
        .expect("Lỗi sub fsm");
    mqtt_client
        .subscribe("AGITECH/+/dosing_report", QoS::AtLeastOnce)
        .await
        .expect("Lỗi sub dosing_report");

    let app_state_for_mqtt = app_state.clone();
    tokio::spawn(async move {
        info!("Bắt đầu vòng lặp sự kiện MQTT dưới nền...");
        loop {
            match eventloop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) => {
                    mqtt::handler::process_message(publish, app_state_for_mqtt.clone()).await;
                }
                Ok(_) => {}
                Err(e) => {
                    error!("Mất kết nối MQTT, thử lại sau 5 giây... Lỗi: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let server_port: u16 = env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()?;

    info!(
        "🚀 API Server đang khởi chạy tại http://{}:{}",
        server_host, server_port
    );

    HttpServer::new(move || {
        let auth_middleware = api::middleware::auth::ApiKeyAuth::new(app_state.api_key.clone());
        let rate_limit_middleware = api::middleware::rate_limit::RateLimiter::new(60, 60);

        App::new()
            .app_data(app_state.clone())
            .wrap(rate_limit_middleware)
            .wrap(auth_middleware)
            .service(
                web::scope("/api")
                    .configure(api::notification::init_routes)
                    .configure(api::solana::init_routes)
                    .service(
                        web::scope("/devices/{device_id}")
                            .configure(api::control::init_routes)
                            .configure(api::sensor::init_routes)
                            .configure(api::ws::init_routes)
                            .configure(api::config::init_routes)
                            .configure(api::crop_season::init_routes)
                            .configure(api::alert::init_routes),
                    ),
            )
    })
    .bind((server_host, server_port))?
    .run()
    .await?;

    Ok(())
}
