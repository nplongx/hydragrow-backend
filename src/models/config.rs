use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DeviceConfig {
    pub device_id: String,
    pub ec_target: f64,
    pub ec_tolerance: f64,
    pub ph_target: f64,
    pub ph_tolerance: f64,
    pub temp_target: f64,
    pub temp_tolerance: f64,
    pub control_mode: String,
    pub is_enabled: i64, // SQLite không có boolean, dùng INTEGER (0, 1)
    pub last_updated: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct SensorCalibration {
    pub device_id: String,
    pub ph_v7: f64,
    pub ph_v4: f64,
    pub ec_factor: f64,
    pub temp_offset: f64,
    pub last_calibrated: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PumpCalibration {
    pub id: String,
    pub device_id: String,
    pub pump_type: String, // 'A', 'B', 'PH_UP', 'PH_DOWN'
    pub flow_rate_ml_per_sec: f64,
    pub min_activation_sec: f64,
    pub max_activation_sec: f64,
    pub last_calibrated: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DosingCalibration {
    pub device_id: String,
    pub tank_volume_l: f64,
    pub ec_gain_per_ml: f64,
    pub ph_shift_up_per_ml: f64,
    pub ph_shift_down_per_ml: f64,
    pub mixing_delay_sec: i64,
    pub ec_step_ratio: f64,
    pub ph_step_ratio: f64,
    pub last_calibrated: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct SafetyConfig {
    pub device_id: String,
    pub max_ec_limit: f64,
    pub min_ec_limit: f64,
    pub min_ph_limit: f64,
    pub max_ph_limit: f64,
    pub max_ec_delta: f64,
    pub max_ph_delta: f64,
    pub max_dose_per_cycle: f64,
    pub cooldown_sec: i64,
    pub max_dose_per_hour: f64,

    pub water_level_critical_min: f64, // Mức nước nguy hiểm (cạn quá mức -> cháy bơm)
    pub max_refill_cycles_per_hour: i64, // Chống kẹt phao (bơm liên tục)
    pub max_drain_cycles_per_hour: i64, // Chống rò rỉ (xả liên tục)
    pub max_refill_duration_sec: i64,  // Chống tràn (thời gian bơm max 1 lần)
    pub max_drain_duration_sec: i64,   // Chống kẹt van xả

    pub min_temp_limit: f64,
    pub max_temp_limit: f64,

    pub emergency_shutdown: i64,

    // XÓA: water_level_target và water_level_max (Vì đã sang WaterConfig)
    pub last_updated: String,
}

// Cập nhật lại WaterConfig cho khớp 100% với React State
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WaterConfig {
    pub device_id: String, // Thêm cho chuẩn form
    pub water_level_min: f64,
    pub water_level_target: f64,
    pub water_level_max: f64,
    pub water_level_drain: f64,
    pub circulation_mode: String,
    pub circulation_on_sec: i64,
    pub circulation_off_sec: i64,
    pub last_updated: String, // Thêm để tracking
}
