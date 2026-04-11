use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

// --- CÁC STRUCT DATABASE (GIỮ NGUYÊN) ---
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
    pub is_enabled: i64,
    pub pump_a_capacity_ml_per_sec: f64,
    pub pump_b_capacity_ml_per_sec: f64,
    pub delay_between_a_and_b_sec: i64,
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

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PumpCalibration {
    pub id: String,
    pub device_id: String,
    pub pump_type: String,
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

    pub dosing_pwm_percent: i64,
    pub osaka_mixing_pwm_percent: i64,
    pub osaka_misting_pwm_percent: i64,
}

impl Default for DosingCalibration {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            tank_volume_l: 100.0,
            ec_gain_per_ml: 0.01,
            ph_shift_up_per_ml: 0.01,
            ph_shift_down_per_ml: 0.01,
            active_mixing_sec: 30,
            sensor_stabilize_sec: 10,
            ec_step_ratio: 0.1,
            ph_step_ratio: 0.1,
            dosing_pump_capacity_ml_per_sec: 1.0,
            soft_start_duration: 5,
            last_calibrated: String::new(),
            scheduled_mixing_interval_sec: 600,
            scheduled_mixing_duration_sec: 60,

            dosing_pwm_percent: 50,
            osaka_mixing_pwm_percent: 60,
            osaka_misting_pwm_percent: 100,
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
            max_ec_limit: 3.0,
            min_ec_limit: 0.5,
            min_ph_limit: 5.5,
            max_ph_limit: 6.5,
            max_ec_delta: 0.5,
            max_ph_delta: 0.5,
            max_dose_per_cycle: 50.0,
            cooldown_sec: 60,
            max_dose_per_hour: 200.0,
            water_level_critical_min: 5.0,
            max_refill_cycles_per_hour: 10,
            max_drain_cycles_per_hour: 10,
            max_refill_duration_sec: 120,
            max_drain_duration_sec: 120,
            min_temp_limit: 15.0,
            max_temp_limit: 35.0,
            emergency_shutdown: 0,
            ec_ack_threshold: 0.2,
            ph_ack_threshold: 0.2,
            water_ack_threshold: 1.0,
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
            water_level_min: 10.0,
            water_level_target: 20.0,
            water_level_max: 30.0,
            water_level_drain: 5.0,
            circulation_mode: "auto".to_string(),
            circulation_on_sec: 60,
            circulation_off_sec: 300,
            water_level_tolerance: 1.0,
            auto_refill_enabled: 1,
            auto_drain_overflow: 1,
            auto_dilute_enabled: 0,
            dilute_drain_amount_cm: 2.0,
            scheduled_water_change_enabled: 0,
            water_change_interval_sec: 86400,
            scheduled_drain_amount_cm: 5.0,
            misting_on_duration_ms: 5000,
            misting_off_duration_ms: 10000,
            last_updated: String::new(),
        }
    }
}

// =========================================================================
// 🟢 PAYLOAD HỢP NHẤT DÀNH RIÊNG ĐỂ BẮN MQTT XUỐNG ESP32
// =========================================================================
#[derive(Debug, Serialize)]
pub struct MqttConfigPayload {
    pub device_id: String,
    pub control_mode: String,
    pub is_enabled: bool,

    // Ngưỡng mục tiêu
    pub ec_target: f32,
    pub ec_tolerance: f32,
    pub ph_target: f32,
    pub ph_tolerance: f32,

    // Nước
    pub water_level_min: f32,
    pub water_level_target: f32,
    pub water_level_max: f32,
    pub water_level_tolerance: f32,
    pub auto_refill_enabled: bool,
    pub auto_drain_overflow: bool,
    pub auto_dilute_enabled: bool,
    pub dilute_drain_amount_cm: f32,
    pub scheduled_water_change_enabled: bool,
    pub water_change_interval_sec: u64,
    pub scheduled_drain_amount_cm: f32,
    pub misting_on_duration_ms: u64,
    pub misting_off_duration_ms: u64,

    // An toàn
    pub emergency_shutdown: bool,
    pub max_ec_limit: f32,
    pub min_ec_limit: f32,
    pub min_ph_limit: f32,
    pub max_ph_limit: f32,
    pub max_ec_delta: f32,
    pub max_ph_delta: f32,
    pub max_dose_per_cycle: f32,
    pub water_level_critical_min: f32,
    pub max_refill_duration_sec: u64,
    pub max_drain_duration_sec: u64,
    pub ec_ack_threshold: f32,
    pub ph_ack_threshold: f32,
    pub water_ack_threshold: f32,

    // Châm phân
    pub ec_gain_per_ml: f32,
    pub ph_shift_up_per_ml: f32,
    pub ph_shift_down_per_ml: f32,
    pub active_mixing_sec: u64,
    pub sensor_stabilize_sec: u64,
    pub ec_step_ratio: f32,
    pub ph_step_ratio: f32,
    pub dosing_pump_capacity_ml_per_sec: f32,
    pub soft_start_duration: u64,
    pub scheduled_mixing_interval_sec: u64,
    pub scheduled_mixing_duration_sec: u64,

    // Cảm biến
    pub ph_v7: f32,
    pub ph_v4: f32,
    pub ec_factor: f32,
    pub ec_offset: f32,
    pub temp_offset: f32,
    pub temp_compensation_beta: f32,
    pub sampling_interval: u64,
    pub publish_interval: u64,
    pub moving_average_window: u32,

    // Cờ Hardware (Map từ is_xxx_enabled của SensorCalibration)
    pub enable_ec_sensor: bool,
    pub enable_ph_sensor: bool,
    pub enable_water_level_sensor: bool,
    pub enable_temp_sensor: bool,

    pub dosing_pwm_percent: u32,
    pub osaka_mixing_pwm_percent: u32,
    pub osaka_misting_pwm_percent: u32,
}

impl MqttConfigPayload {
    /// Hàm gom 5 bảng DB thành 1 cục JSON phẳng duy nhất,
    /// ép kiểu i64 -> bool chuẩn xác cho ESP32 đọc.
    pub fn from_db_rows(
        dev: &DeviceConfig,
        water: &WaterConfig,
        safe: &SafetyConfig,
        dose: &DosingCalibration,
        sens: &SensorCalibration,
    ) -> Self {
        Self {
            device_id: dev.device_id.clone(),
            control_mode: dev.control_mode.clone(),
            is_enabled: dev.is_enabled != 0, // i64 -> bool

            ec_target: dev.ec_target as f32,
            ec_tolerance: dev.ec_tolerance as f32,
            ph_target: dev.ph_target as f32,
            ph_tolerance: dev.ph_tolerance as f32,

            water_level_min: water.water_level_min as f32,
            water_level_target: water.water_level_target as f32,
            water_level_max: water.water_level_max as f32,
            water_level_tolerance: water.water_level_tolerance as f32,
            auto_refill_enabled: water.auto_refill_enabled != 0,
            auto_drain_overflow: water.auto_drain_overflow != 0,
            auto_dilute_enabled: water.auto_dilute_enabled != 0,
            dilute_drain_amount_cm: water.dilute_drain_amount_cm as f32,
            scheduled_water_change_enabled: water.scheduled_water_change_enabled != 0,
            water_change_interval_sec: water.water_change_interval_sec as u64,
            scheduled_drain_amount_cm: water.scheduled_drain_amount_cm as f32,
            misting_on_duration_ms: water.misting_on_duration_ms as u64,
            misting_off_duration_ms: water.misting_off_duration_ms as u64,

            emergency_shutdown: safe.emergency_shutdown != 0,
            max_ec_limit: safe.max_ec_limit as f32,
            min_ec_limit: safe.min_ec_limit as f32,
            min_ph_limit: safe.min_ph_limit as f32,
            max_ph_limit: safe.max_ph_limit as f32,
            max_ec_delta: safe.max_ec_delta as f32,
            max_ph_delta: safe.max_ph_delta as f32,
            max_dose_per_cycle: safe.max_dose_per_cycle as f32,
            water_level_critical_min: safe.water_level_critical_min as f32,
            max_refill_duration_sec: safe.max_refill_duration_sec as u64,
            max_drain_duration_sec: safe.max_drain_duration_sec as u64,
            ec_ack_threshold: safe.ec_ack_threshold as f32,
            ph_ack_threshold: safe.ph_ack_threshold as f32,
            water_ack_threshold: safe.water_ack_threshold as f32,

            ec_gain_per_ml: dose.ec_gain_per_ml as f32,
            ph_shift_up_per_ml: dose.ph_shift_up_per_ml as f32,
            ph_shift_down_per_ml: dose.ph_shift_down_per_ml as f32,
            active_mixing_sec: dose.active_mixing_sec as u64,
            sensor_stabilize_sec: dose.sensor_stabilize_sec as u64,
            ec_step_ratio: dose.ec_step_ratio as f32,
            ph_step_ratio: dose.ph_step_ratio as f32,
            dosing_pump_capacity_ml_per_sec: dose.dosing_pump_capacity_ml_per_sec as f32,
            soft_start_duration: dose.soft_start_duration as u64,
            scheduled_mixing_interval_sec: dose.scheduled_mixing_interval_sec as u64,
            scheduled_mixing_duration_sec: dose.scheduled_mixing_duration_sec as u64,

            ph_v7: sens.ph_v7 as f32,
            ph_v4: sens.ph_v4 as f32,
            ec_factor: sens.ec_factor as f32,
            ec_offset: sens.ec_offset as f32,
            temp_offset: sens.temp_offset as f32,
            temp_compensation_beta: sens.temp_compensation_beta as f32,
            sampling_interval: sens.sampling_interval as u64,
            publish_interval: sens.publish_interval as u64,
            moving_average_window: sens.moving_average_window as u32,

            enable_ec_sensor: sens.is_ec_enabled != 0,
            enable_ph_sensor: sens.is_ph_enabled != 0,
            enable_water_level_sensor: sens.is_water_level_enabled != 0,
            enable_temp_sensor: sens.is_temp_enabled != 0,

            dosing_pwm_percent: dose.dosing_pwm_percent as u32,
            osaka_mixing_pwm_percent: dose.osaka_mixing_pwm_percent as u32,
            osaka_misting_pwm_percent: dose.osaka_misting_pwm_percent as u32,
        }
    }
}
