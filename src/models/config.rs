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

    // --- CÁC TRƯỜNG ĐƯỢC CẬP NHẬT THEO ESP32 ---
    pub active_mixing_sec: i64,
    pub sensor_stabilize_sec: i64,

    pub ec_step_ratio: f64,
    pub ph_step_ratio: f64,

    // --- CẬP NHẬT THEO ESP32 ---
    pub dosing_pump_capacity_ml_per_sec: f64,
    pub last_calibrated: String,
}

impl Default for DosingCalibration {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            tank_volume_l: 100.0,
            ec_gain_per_ml: 0.015,
            ph_shift_up_per_ml: 0.02,
            ph_shift_down_per_ml: 0.025,

            // Cập nhật giá trị mặc định giống ESP32
            active_mixing_sec: 5,
            sensor_stabilize_sec: 5,

            ec_step_ratio: 0.4,
            ph_step_ratio: 0.2,

            // Cập nhật giá trị mặc định giống ESP32
            dosing_pump_capacity_ml_per_sec: 0.5,
            last_calibrated: String::new(),
        }
    }
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
    pub last_updated: String,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            max_ec_limit: 3.5,
            min_ec_limit: 0.0,
            min_ph_limit: 4.0,
            max_ph_limit: 8.5,
            max_ec_delta: 1.0,
            max_ph_delta: 1.5,
            max_dose_per_cycle: 2.0,
            cooldown_sec: 300,
            max_dose_per_hour: 10.0,
            water_level_critical_min: 5.0,
            max_refill_cycles_per_hour: 3,
            max_drain_cycles_per_hour: 3,
            max_refill_duration_sec: 120,
            max_drain_duration_sec: 120,
            min_temp_limit: 10.0,
            max_temp_limit: 40.0,
            emergency_shutdown: 0,
            last_updated: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct WaterConfig {
    pub device_id: String,
    pub water_level_min: f64,
    pub water_level_target: f64,
    pub water_level_max: f64,
    pub water_level_drain: f64,
    pub circulation_mode: String,
    pub circulation_on_sec: i64,
    pub circulation_off_sec: i64,

    pub water_level_tolerance: f64,
    pub auto_refill_enabled: i64,
    pub auto_drain_overflow: i64,
    pub auto_dilute_enabled: i64,
    pub dilute_drain_amount_cm: f64,
    pub scheduled_water_change_enabled: i64,
    pub water_change_interval_sec: i64,
    pub scheduled_drain_amount_cm: f64,

    pub last_updated: String,
}

impl Default for WaterConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            water_level_min: 15.0,
            water_level_target: 20.0,
            water_level_max: 24.0,
            water_level_drain: 5.0,
            circulation_mode: "continuous".to_string(),
            circulation_on_sec: 3600,
            circulation_off_sec: 0,
            water_level_tolerance: 1.0,
            auto_refill_enabled: 1,
            auto_drain_overflow: 1,
            auto_dilute_enabled: 1,
            dilute_drain_amount_cm: 2.0,
            scheduled_water_change_enabled: 0,
            water_change_interval_sec: 259200,
            scheduled_drain_amount_cm: 5.0,
            last_updated: String::new(),
        }
    }
}

/// Đây là struct dùng để tạo JSON đẩy qua MQTT.
/// Các trường (fields) bắt buộc phải khớp 100% với struct DeviceConfig trên ESP32
#[derive(Debug, Serialize)]
pub struct Esp32AggregatedConfig {
    pub device_id: String,
    pub control_mode: String,
    pub is_enabled: bool,

    // --- 1. DEVICE CONFIG ---
    pub ec_target: f64,
    pub ec_tolerance: f64,
    pub ph_target: f64,
    pub ph_tolerance: f64,

    // --- 2. WATER CONFIG ---
    pub water_level_min: f64,
    pub water_level_target: f64,
    pub water_level_max: f64,
    pub water_level_tolerance: f64,
    pub auto_refill_enabled: bool,
    pub auto_drain_overflow: bool,

    pub auto_dilute_enabled: bool,
    pub dilute_drain_amount_cm: f64,

    pub scheduled_water_change_enabled: bool,
    pub water_change_interval_sec: i64,
    pub scheduled_drain_amount_cm: f64,

    // --- 3. SAFETY CONFIG ---
    pub emergency_shutdown: bool,
    pub max_ec_limit: f64,
    pub min_ph_limit: f64,
    pub max_ph_limit: f64,
    pub max_ec_delta: f64,
    pub max_ph_delta: f64,
    pub max_dose_per_cycle: f64,
    pub water_level_critical_min: f64,
    pub max_refill_duration_sec: i64,
    pub max_drain_duration_sec: i64,

    // --- 4. DOSING & PUMP ---
    pub ec_gain_per_ml: f64,
    pub ph_shift_up_per_ml: f64,
    pub ph_shift_down_per_ml: f64,

    // Đã thay thế và thêm mới để khớp ESP32
    pub active_mixing_sec: i64,
    pub sensor_stabilize_sec: i64,

    pub ec_step_ratio: f64,
    pub ph_step_ratio: f64,

    // Đã đổi tên để khớp ESP32
    pub dosing_pump_capacity_ml_per_sec: f64,
}

