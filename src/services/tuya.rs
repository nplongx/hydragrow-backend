use anyhow::{Result, anyhow};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{
    Client, Method,
    header::{HeaderMap, HeaderValue},
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use tracing::{error, info};

// Cấu hình URL mặc định (Tùy khu vực của bạn, ở VN thường dùng server China hoặc US)
// Đổi thành "https://openapi.tuyaus.com" nếu bạn chọn server Mỹ
const TUYA_BASE_URL: &str = "https://openapi.tuyacn.com";

#[derive(Deserialize, Debug)]
struct TuyaTokenResponse {
    success: bool,
    result: Option<TuyaTokenResult>,
    msg: Option<String>,
}

#[derive(Deserialize, Debug)]
struct TuyaTokenResult {
    access_token: String,
}

/// Lấy mã băm SHA256 của body
fn get_body_hash(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    hex::encode(hasher.finalize()).to_lowercase()
}

/// Thuật toán tạo Signature chuẩn của Tuya
fn generate_sign(
    client_id: &str,
    secret: &str,
    access_token: &str,
    timestamp: &str,
    method: &str,
    url: &str,
    body_str: &str,
) -> String {
    let body_hash = get_body_hash(body_str);
    // Format: HTTPMethod \n Content-SHA256 \n Headers \n URL
    let string_to_sign = format!("{}\n{}\n\n{}", method, body_hash, url);
    let str_for_mac = format!(
        "{}{}{}{}",
        client_id, access_token, timestamp, string_to_sign
    );

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC có thể nhận key mọi kích cỡ");
    mac.update(str_for_mac.as_bytes());
    hex::encode(mac.finalize().into_bytes()).to_uppercase()
}

/// Lấy Access Token từ Tuya Cloud
async fn get_tuya_token(client: &Client, client_id: &str, secret: &str) -> Result<String> {
    let timestamp = Utc::now().timestamp_millis().to_string();
    let url_path = "/v1.0/token?grant_type=1";

    // Khi lấy token, access_token = rỗng, body = rỗng
    let sign = generate_sign(client_id, secret, "", &timestamp, "GET", url_path, "");

    let mut headers = HeaderMap::new();
    headers.insert("client_id", HeaderValue::from_str(client_id)?);
    headers.insert("sign", HeaderValue::from_str(&sign)?);
    headers.insert("sign_method", HeaderValue::from_static("HMAC-SHA256"));
    headers.insert("t", HeaderValue::from_str(&timestamp)?);

    let url = format!("{}{}", TUYA_BASE_URL, url_path);
    let resp: TuyaTokenResponse = client
        .get(&url)
        .headers(headers)
        .send()
        .await?
        .json()
        .await?;

    if resp.success {
        if let Some(res) = resp.result {
            return Ok(res.access_token);
        }
    }

    Err(anyhow!("Lỗi lấy Token Tuya: {:?}", resp.msg))
}

/// Hàm chính: Gửi lệnh Bật/Tắt ổ cắm
pub async fn send_tuya_command(turn_on: bool) -> Result<()> {
    // 1. Lấy thông tin từ biến môi trường (.env)
    let client_id = env::var("TUYA_CLIENT_ID").unwrap_or_default();
    let secret = env::var("TUYA_SECRET").unwrap_or_default();
    let device_id = env::var("TUYA_DEVICE_ID").unwrap_or_default();

    if client_id.is_empty() || secret.is_empty() || device_id.is_empty() {
        return Err(anyhow!(
            "Thiếu cấu hình TUYA_CLIENT_ID, TUYA_SECRET hoặc TUYA_DEVICE_ID trong .env"
        ));
    }

    let client = Client::new();

    // 2. Lấy Access Token
    let access_token = get_tuya_token(&client, &client_id, &secret).await?;

    // 3. Chuẩn bị payload lệnh
    let url_path = format!("/v1.0/iot-03/devices/{}/commands", device_id);
    let payload = json!({
        "commands": [
            {
                "code": "switch_1", // Mã code chuẩn của ổ cắm Tuya
                "value": turn_on
            }
        ]
    });
    let body_str = serde_json::to_string(&payload)?;

    // 4. Tạo chữ ký (Signature) cho request POST
    let timestamp = Utc::now().timestamp_millis().to_string();
    let sign = generate_sign(
        &client_id,
        &secret,
        &access_token,
        &timestamp,
        "POST",
        &url_path,
        &body_str,
    );

    // 5. Gắn Headers
    let mut headers = HeaderMap::new();
    headers.insert("client_id", HeaderValue::from_str(&client_id)?);
    headers.insert("access_token", HeaderValue::from_str(&access_token)?);
    headers.insert("sign", HeaderValue::from_str(&sign)?);
    headers.insert("sign_method", HeaderValue::from_static("HMAC-SHA256"));
    headers.insert("t", HeaderValue::from_str(&timestamp)?);
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    let url = format!("{}{}", TUYA_BASE_URL, url_path);

    // 6. Gửi lệnh
    let resp = client
        .post(&url)
        .headers(headers)
        .body(body_str)
        .send()
        .await?;
    let resp_text = resp.text().await?;

    // Kiểm tra xem Tuya phản hồi true hay false
    if resp_text.contains("\"success\":true") {
        Ok(())
    } else {
        error!("Tuya API trả về lỗi: {}", resp_text);
        Err(anyhow!("Gửi lệnh Tuya thất bại"))
    }
}
