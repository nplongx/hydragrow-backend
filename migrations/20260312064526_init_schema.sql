PRAGMA foreign_keys = ON;

CREATE TABLE device_config (
    device_id TEXT NOT NULL PRIMARY KEY,
    ec_target REAL NOT NULL,
    ec_tolerance REAL NOT NULL,
    ph_min REAL NOT NULL,
    ph_max REAL NOT NULL,
    ph_tolerance REAL NOT NULL,
    temp_min REAL NOT NULL,
    temp_max REAL NOT NULL,
    control_mode TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE sensor_calibration (
    device_id TEXT NOT NULL PRIMARY KEY,
    ph_v7 REAL NOT NULL,
    ph_v4 REAL NOT NULL,
    ec_factor REAL NOT NULL,
    temp_offset REAL NOT NULL,
    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

CREATE TABLE pump_calibration (
    id TEXT NOT NULL PRIMARY KEY,
    device_id TEXT NOT NULL,
    pump_type TEXT NOT NULL CHECK(pump_type IN ('A','B','PH_UP','PH_DOWN')),
    flow_rate_ml_per_sec REAL NOT NULL,
    min_activation_sec REAL NOT NULL,
    max_activation_sec REAL NOT NULL,
    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

CREATE TABLE dosing_calibration (
    device_id TEXT NOT NULL PRIMARY KEY,
    tank_volume_l REAL NOT NULL,
    ec_gain_per_ml REAL NOT NULL,
    ph_shift_up_per_ml REAL NOT NULL,
    ph_shift_down_per_ml REAL NOT NULL,
    mixing_delay_sec INTEGER NOT NULL,
    ec_step_ratio REAL NOT NULL,
    ph_step_ratio REAL NOT NULL,
    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

CREATE TABLE safety_config (
    device_id TEXT NOT NULL PRIMARY KEY,

    max_ec_limit REAL NOT NULL,
    min_ph_limit REAL NOT NULL,
    max_ph_limit REAL NOT NULL,
    max_ec_delta REAL NOT NULL,
    max_ph_delta REAL NOT NULL,

    max_dose_per_cycle REAL NOT NULL,
    max_dose_per_hour REAL NOT NULL,
    cooldown_sec INTEGER NOT NULL,

    water_level_critical_min REAL NOT NULL,
    water_level_target REAL NOT NULL,
    water_level_max REAL NOT NULL,

    max_refill_cycles_per_hour INTEGER NOT NULL,
    max_drain_cycles_per_hour INTEGER NOT NULL,

    max_refill_duration_sec INTEGER NOT NULL,
    max_drain_duration_sec INTEGER NOT NULL,

    emergency_shutdown INTEGER NOT NULL DEFAULT 0,

    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

-- Indexes for faster queries
CREATE INDEX idx_pump_device ON pump_calibration(device_id);
CREATE INDEX idx_sensor_calibration_device ON sensor_calibration(device_id);
