use chrono::{DateTime, FixedOffset, Utc};
use influxdb2::FromDataPoint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, FromDataPoint, Default)]
pub struct SensorData {
    pub device_id: String,
    pub ec_value: f64,
    pub ph_value: f64,
    pub temp_value: f64,
    pub water_level: f64,
    pub pump_status: String, // Có thể chứa JSON string như '{"A":"on", "B":"off"}'
    pub timestamp: DateTime<FixedOffset>,
}
