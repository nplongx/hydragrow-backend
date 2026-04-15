PRAGMA foreign_keys = ON;

-- 1. DEVICE CONFIG
CREATE TABLE device_config (
    device_id TEXT NOT NULL PRIMARY KEY,
    ec_target REAL NOT NULL,
    ec_tolerance REAL NOT NULL,
    ph_target REAL NOT NULL,
    ph_tolerance REAL NOT NULL,
    temp_target REAL NOT NULL,
    temp_tolerance REAL NOT NULL,
    control_mode TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1, -- 0: false, 1: true
    
    -- [CỘT MỚI BỔ SUNG]
    pump_a_capacity_ml_per_sec REAL NOT NULL DEFAULT 1.2,
    pump_b_capacity_ml_per_sec REAL NOT NULL DEFAULT 1.2,
    delay_between_a_and_b_sec INTEGER NOT NULL DEFAULT 10,

    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 2. SENSOR CALIBRATION
CREATE TABLE sensor_calibration (
    device_id TEXT NOT NULL PRIMARY KEY,
    ph_v7 REAL NOT NULL,
    ph_v4 REAL NOT NULL,
    ec_factor REAL NOT NULL,
    ec_offset REAL NOT NULL DEFAULT 0.0,
    temp_offset REAL NOT NULL,
    temp_compensation_beta REAL NOT NULL DEFAULT 0.02,
    sampling_interval INTEGER NOT NULL DEFAULT 1000,
    publish_interval INTEGER NOT NULL DEFAULT 5000,
    moving_average_window INTEGER NOT NULL DEFAULT 10,
    is_ph_enabled INTEGER NOT NULL DEFAULT 1,
    is_ec_enabled INTEGER NOT NULL DEFAULT 1,
    is_temp_enabled INTEGER NOT NULL DEFAULT 1,
    is_water_level_enabled INTEGER NOT NULL DEFAULT 1,
    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id) ON DELETE CASCADE
);

-- 3. PUMP CALIBRATION
CREATE TABLE pump_calibration (
    id TEXT NOT NULL PRIMARY KEY,
    device_id TEXT NOT NULL,
    pump_type TEXT NOT NULL CHECK(pump_type IN ('A','B','PH_UP','PH_DOWN')),
    flow_rate_ml_per_sec REAL NOT NULL,
    min_activation_sec REAL NOT NULL,
    max_activation_sec REAL NOT NULL,
    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id) ON DELETE CASCADE
);

-- 4. DOSING CALIBRATION
CREATE TABLE dosing_calibration (
    device_id TEXT NOT NULL PRIMARY KEY,
    tank_volume_l REAL NOT NULL,
    ec_gain_per_ml REAL NOT NULL,
    ph_shift_up_per_ml REAL NOT NULL,
    ph_shift_down_per_ml REAL NOT NULL,
    active_mixing_sec INTEGER NOT NULL,
    sensor_stabilize_sec INTEGER NOT NULL,
    ec_step_ratio REAL NOT NULL,
    ph_step_ratio REAL NOT NULL,
    dosing_pump_capacity_ml_per_sec REAL NOT NULL DEFAULT 0.5,
    soft_start_duration INTEGER NOT NULL DEFAULT 3000,
    scheduled_mixing_interval_sec INTEGER NOT NULL DEFAULT 3600,
    scheduled_mixing_duration_sec INTEGER NOT NULL DEFAULT 300,

    -- [CỘT MỚI BỔ SUNG]
    dosing_pwm_percent INTEGER NOT NULL DEFAULT 50,
    osaka_mixing_pwm_percent INTEGER NOT NULL DEFAULT 60,
    osaka_misting_pwm_percent INTEGER NOT NULL DEFAULT 100,

    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id) ON DELETE CASCADE
);

-- 5. WATER CONFIG
CREATE TABLE water_config (
    device_id TEXT NOT NULL PRIMARY KEY,
    water_level_min REAL NOT NULL,
    water_level_target REAL NOT NULL,
    water_level_max REAL NOT NULL,
    water_level_drain REAL NOT NULL,

    circulation_mode TEXT NOT NULL,
    circulation_on_sec INTEGER NOT NULL,
    circulation_off_sec INTEGER NOT NULL,

    water_level_tolerance REAL NOT NULL DEFAULT 1.0,
    auto_refill_enabled INTEGER NOT NULL DEFAULT 1,
    auto_drain_overflow INTEGER NOT NULL DEFAULT 1,
    auto_dilute_enabled INTEGER NOT NULL DEFAULT 1,
    dilute_drain_amount_cm REAL NOT NULL DEFAULT 2.0,
    scheduled_water_change_enabled INTEGER NOT NULL DEFAULT 0,
    water_change_interval_sec INTEGER NOT NULL DEFAULT 259200,
    scheduled_drain_amount_cm REAL NOT NULL DEFAULT 5.0,
    misting_on_duration_ms INTEGER NOT NULL DEFAULT 10000,
    misting_off_duration_ms INTEGER NOT NULL DEFAULT 180000,
    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id) ON DELETE CASCADE
);

-- 6. SAFETY CONFIG
CREATE TABLE safety_config (
    device_id TEXT NOT NULL PRIMARY KEY,
    min_ec_limit REAL NOT NULL,
    max_ec_limit REAL NOT NULL,
    min_ph_limit REAL NOT NULL,
    max_ph_limit REAL NOT NULL,
    max_ec_delta REAL NOT NULL,
    max_ph_delta REAL NOT NULL,
    max_dose_per_cycle REAL NOT NULL,
    max_dose_per_hour REAL NOT NULL,
    cooldown_sec INTEGER NOT NULL,
    min_temp_limit REAL NOT NULL,
    max_temp_limit REAL NOT NULL,
    water_level_critical_min REAL NOT NULL,
    max_refill_cycles_per_hour INTEGER NOT NULL,
    max_drain_cycles_per_hour INTEGER NOT NULL,
    max_refill_duration_sec INTEGER NOT NULL,
    max_drain_duration_sec INTEGER NOT NULL,
    emergency_shutdown INTEGER NOT NULL DEFAULT 0,
    ec_ack_threshold REAL NOT NULL DEFAULT 0.05,
    ph_ack_threshold REAL NOT NULL DEFAULT 0.1,
    water_ack_threshold REAL NOT NULL DEFAULT 0.5,
    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id) ON DELETE CASCADE
);

-- 7. BLOCKCHAIN HISTORY
CREATE TABLE blockchain_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_id TEXT NOT NULL,
    action TEXT NOT NULL,
    tx_id TEXT NOT NULL UNIQUE,
    explorer_url TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id) ON DELETE CASCADE
);

-- Create crop_seasons table
CREATE TABLE crop_seasons (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL,
    name TEXT NOT NULL, -- Ví dụ: "Dưa lưới giống Nhật - Vụ Xuân 2026"
    plant_type TEXT, -- Loại cây
    start_time DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    end_time DATETIME, -- Null nếu vụ đang chạy
    status TEXT NOT NULL DEFAULT 'active', -- 'active' hoặc 'completed'
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

CREATE TABLE IF NOT EXISTS blockchain_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    season_id TEXT,
    action TEXT NOT NULL,
    tx_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS system_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    level TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_system_events_device_id ON system_events(device_id);
CREATE INDEX IF NOT EXISTS idx_system_events_timestamp ON system_events(timestamp DESC);

-- Thêm cột season_id vào bảng lưu lịch sử blockchain (nếu bạn có bảng này trong DB)
-- Hoặc chúng ta sẽ lọc (filter) dựa vào khoảng thời gian start_time -> end_time.

-- INDEXES
CREATE INDEX idx_blockchain_device ON blockchain_history(device_id);
CREATE INDEX idx_pump_device ON pump_calibration(device_id);
