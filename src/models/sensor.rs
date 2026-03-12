use std::default;

use influxdb2::FromDataPoint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviceState {
    On,
    #[default]
    Off,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ValveState {
    Open,
    #[default]
    Closed,
}

/// Parse trực tiếp từ MQTT JSON payload của ESP32
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PumpStatus {
    #[serde(rename = "A")]
    pub a: DeviceState,
    #[serde(rename = "B")]
    pub b: DeviceState,
    #[serde(rename = "PH_UP")]
    pub ph_up: DeviceState,
    #[serde(rename = "PH_DOWN")]
    pub ph_down: DeviceState,
    #[serde(rename = "CIRCULATION")]
    pub circulation: DeviceState,
    #[serde(rename = "WATER_PUMP")]
    pub water_pump: DeviceState,
    #[serde(rename = "VAN_IN")]
    pub van_in: ValveState,
    #[serde(rename = "VAN_OUT")]
    pub van_out: ValveState,
}

/// Cấu trúc Sensor đẩy vào InfluxDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    pub device_id: String,
    pub ec_value: f64,
    pub ph_value: f64,
    pub temp_value: f64,
    pub water_level: f64,
    pub pump_status: PumpStatus,
    pub timestamp: String,
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
            timestamp: row.timestamp,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromDataPoint, Default)]
pub struct SensorDataRow {
    pub device_id: String,
    pub ec_value: f64,
    pub ph_value: f64,
    pub temp_value: f64,
    pub water_level: f64,
    pub pump_status: String,
    pub timestamp: String,
}

/// Payload request cho Manual Control Van (nhận từ API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValveCommandReq {
    pub valve: String,      // "VAN_IN" | "VAN_OUT"
    pub action: ValveState, // "open" | "closed"
}
