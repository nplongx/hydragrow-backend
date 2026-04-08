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
    pub is_enabled: i64, // SQLite INTEGER (0, 1)
    pub last_updated: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct SensorCalibration {
    pub device_id: String,
    pub ph_v7: f64,
    pub ph_v4: f64,
    pub ec_factor: f64,
    pub ec_offset: f64,
    pub temp_offset: f64,
    pub temp_compensation_beta: f64,
    pub sampling_interval: i64,
    pub publish_interval: i64,
    pub moving_average_window: i64,
    pub is_ph_enabled: i64,
    pub is_ec_enabled: i64,
    pub is_temp_enabled: i64,
    pub is_water_level_enabled: i64,
    pub last_calibrated: String,
}

impl From<SensorCalibration> for SensorNodeConfig {
    fn from(db_row: SensorCalibration) -> Self {
        Self {
            device_id: db_row.device_id,
            ph_v7: db_row.ph_v7 as f32,
            ph_v4: db_row.ph_v4 as f32,
            ec_factor: db_row.ec_factor as f32,
            ec_offset: db_row.ec_offset as f32,
            temp_offset: db_row.temp_offset as f32,
            temp_compensation_beta: db_row.temp_compensation_beta as f32,
            sampling_interval: db_row.sampling_interval as u32,
            publish_interval: db_row.publish_interval as u32,
            moving_average_window: db_row.moving_average_window as u8,
            is_ph_enabled: db_row.is_ph_enabled != 0,
            is_ec_enabled: db_row.is_ec_enabled != 0,
            is_temp_enabled: db_row.is_temp_enabled != 0,
            is_water_level_enabled: db_row.is_water_level_enabled != 0,
        }
    }
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
    pub active_mixing_sec: i64,
    pub sensor_stabilize_sec: i64,
    pub ec_step_ratio: f64,
    pub ph_step_ratio: f64,
    pub dosing_pump_capacity_ml_per_sec: f64,
    pub soft_start_duration: i64,
    pub last_calibrated: String,
    pub scheduled_mixing_interval_sec: i64,
    pub scheduled_mixing_duration_sec: i64,
}

impl Default for DosingCalibration {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            tank_volume_l: 100.0,
            ec_gain_per_ml: 0.015,
            ph_shift_up_per_ml: 0.02,
            ph_shift_down_per_ml: 0.025,
            active_mixing_sec: 5,
            sensor_stabilize_sec: 5,
            ec_step_ratio: 0.4,
            ph_step_ratio: 0.2,
            dosing_pump_capacity_ml_per_sec: 0.5,
            soft_start_duration: 3000,
            last_calibrated: String::new(),
            scheduled_mixing_interval_sec: 3600,
            scheduled_mixing_duration_sec: 300,
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
    pub water_level_critical_min: f64,
    pub max_refill_cycles_per_hour: i64,
    pub max_drain_cycles_per_hour: i64,
    pub max_refill_duration_sec: i64,
    pub max_drain_duration_sec: i64,
    pub min_temp_limit: f64,
    pub max_temp_limit: f64,
    pub emergency_shutdown: i64,
    pub ec_ack_threshold: f64,
    pub ph_ack_threshold: f64,
    pub water_ack_threshold: f64,
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
            ec_ack_threshold: 0.05,
            ph_ack_threshold: 0.1,
            water_ack_threshold: 0.5,
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
    pub misting_on_duration_ms: i64,
    pub misting_off_duration_ms: i64,
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
            misting_on_duration_ms: 10000,
            misting_off_duration_ms: 180000,
            last_updated: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ControllerNodeConfig {
    pub device_id: String,
    pub control_mode: String,
    pub is_enabled: bool,
    pub ec_target: f64,
    pub ec_tolerance: f64,
    pub ph_target: f64,
    pub ph_tolerance: f64,
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
    pub misting_on_duration_ms: i64,
    pub misting_off_duration_ms: i64,
    pub emergency_shutdown: bool,
    pub max_ec_limit: f64,
    pub min_ec_limit: f64,
    pub min_ph_limit: f64,
    pub max_ph_limit: f64,
    pub max_ec_delta: f64,
    pub max_ph_delta: f64,
    pub max_dose_per_cycle: f64,
    pub water_level_critical_min: f64,
    pub max_refill_duration_sec: i64,
    pub max_drain_duration_sec: i64,
    pub ec_ack_threshold: f64,
    pub ph_ack_threshold: f64,
    pub water_ack_threshold: f64,
    pub ec_gain_per_ml: f64,
    pub ph_shift_up_per_ml: f64,
    pub ph_shift_down_per_ml: f64,
    pub active_mixing_sec: i64,
    pub sensor_stabilize_sec: i64,
    pub ec_step_ratio: f64,
    pub ph_step_ratio: f64,
    pub dosing_pump_capacity_ml_per_sec: f64,
    pub soft_start_duration: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorNodeConfig {
    pub device_id: String,
    pub ph_v7: f32,
    pub ph_v4: f32,
    pub ec_factor: f32,
    pub ec_offset: f32,
    pub temp_offset: f32,
    pub temp_compensation_beta: f32,
    pub sampling_interval: u32,
    pub publish_interval: u32,
    pub moving_average_window: u8,
    pub is_ph_enabled: bool,
    pub is_ec_enabled: bool,
    pub is_temp_enabled: bool,
    pub is_water_level_enabled: bool,
}

impl Default for SensorNodeConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            ph_v7: 2.5,
            ph_v4: 3.0,
            ec_factor: 880.0,
            ec_offset: 0.0,
            temp_offset: 0.0,
            temp_compensation_beta: 0.02,
            sampling_interval: 1000,
            publish_interval: 5000,
            moving_average_window: 10,
            is_ph_enabled: true,
            is_ec_enabled: true,
            is_temp_enabled: true,
            is_water_level_enabled: true,
        }
    }
}
