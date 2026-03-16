use influxdb2::FromDataPoint;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviceState {
    On,
    #[default]
    Off,
}

/// Trạng thái hoạt động của các máy bơm (Pump)
/// Parse trực tiếp từ MQTT JSON payload của ESP32
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PumpStatus {
    #[serde(rename = "A")]
    pub pump_a: DeviceState,

    #[serde(rename = "B")]
    pub pump_b: DeviceState,

    #[serde(rename = "PH_UP")]
    pub ph_up: DeviceState,

    #[serde(rename = "PH_DOWN")]
    pub ph_down: DeviceState,

    #[serde(rename = "CHAMBER_PUMP")]
    pub chamber_pump: DeviceState,

    #[serde(rename = "WATER_PUMP")]
    pub water_pump: DeviceState,
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

    pub pump_status: PumpStatus,
    pub timestamp: String,
}

/// Cấu trúc trung gian để lưu/đọc từ InfluxDB (Vì InfluxDB không lưu trực tiếp struct lồng nhau)
#[derive(Debug, Clone, Serialize, Deserialize, FromDataPoint, Default)]
pub struct SensorDataRow {
    pub device_id: String,
    pub ec_value: f64,
    pub ph_value: f64,
    pub temp_value: f64,
    pub water_level: f64,
    pub pump_status: String,
    pub _time: String,
}

impl From<SensorDataRow> for SensorData {
    fn from(row: SensorDataRow) -> Self {
        let pump_status = serde_json::from_str(&row.pump_status).unwrap_or_default();
        Self {
            device_id: row.device_id,
            ec_value: row.ec_value,
            ph_value: row.ph_value,
            temp_value: row.temp_value,
            water_level: row.water_level,
            pump_status,
            timestamp: row._time,
        }
    }
}
/// Request điều khiển Bơm từ Frontend
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PumpCommandReq {
    #[validate(length(min = 1))]
    pub pump_id: String, // "A", "B", "PH_UP", "WATER_PUMP", v.v.
    pub action: DeviceState, // "on" | "off"
}
