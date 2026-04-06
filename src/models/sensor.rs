use std::str::FromStr;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use validator::Validate;

// Bạn có thể xóa DeviceState nếu không còn nơi nào trong Frontend/Backend dùng đến nó.
// Mình tạm giữ lại để tránh lỗi compile ở các file khác (nếu có).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviceState {
    On,
    #[default]
    Off,
}

/// Trạng thái hoạt động của các máy bơm (Pump)
/// ĐÃ SỬA: Chuyển sang dùng bool và tên biến chuẩn khớp 100% với ESP32
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PumpStatus {
    pub pump_a: bool,
    pub pump_b: bool,
    pub ph_up: bool,
    pub ph_down: bool,
    pub osaka_pump: bool,
    pub mist_valve: bool,
    pub water_pump_in: bool,
    pub water_pump_out: bool,
}

/// Cấu trúc Sensor đẩy vào InfluxDB và trả về Client
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SensorData {
    pub device_id: String,

    #[validate(range(min = 0.0, max = 20.0))]
    pub ec_value: f64,

    #[validate(range(min = 0.0, max = 14.0))]
    pub ph_value: f64,

    #[validate(range(min = -10.0, max = 100.0))]
    pub temp_value: f64,

    #[validate(range(min = 0.0))]
    pub water_level: f64,

    #[serde(default)]
    pub pump_status: PumpStatus,
    // #[serde(default)]
    // pub timestamp: String,
    pub time: String,
}

/// Cấu trúc trung gian để lưu/đọc từ InfluxDB (Vì InfluxDB không lưu trực tiếp struct lồng nhau)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SensorDataRow {
    pub device_id: String,
    pub ec_value: f64,
    pub ph_value: f64,
    pub temp_value: f64,
    pub water_level: f64,
    pub pump_status: String,
    pub time: String,
}

impl From<SensorDataRow> for SensorData {
    fn from(row: SensorDataRow) -> Self {
        let pump_status = serde_json::from_str(&row.pump_status).unwrap_or_default();

        let time = DateTime::from_str(&row.time)
            .unwrap_or_else(|_| {
                // fallback nếu parse lỗi
                DateTime::parse_from_rfc3339("1970-01-01T00:00:00+00:00").unwrap()
            })
            .to_string();

        Self {
            device_id: row.device_id,
            ec_value: row.ec_value,
            ph_value: row.ph_value,
            temp_value: row.temp_value,
            water_level: row.water_level,
            pump_status,
            time,
        }
    }
}

/// Request điều khiển Bơm từ Frontend
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PumpCommandReq {
    #[validate(length(min = 1))]
    pub pump_id: String, // "A", "B", "PH_UP", "WATER_PUMP", "ALL", v.v.

    // ĐÃ SỬA: Chuyển sang String để có thể truyền lệnh "reset_fault" bên cạnh "on" / "off"
    pub action: String,
}

