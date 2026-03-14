PRAGMA foreign_keys = ON;

-- 1. BẢNG CẤU HÌNH GỐC (Lõi thiết bị)
CREATE TABLE device_config (
    device_id TEXT NOT NULL PRIMARY KEY,
    ec_target REAL NOT NULL,
    ec_tolerance REAL NOT NULL,
    ph_target REAL NOT NULL,
    ph_tolerance REAL NOT NULL,
    temp_target REAL NOT NULL,
    temp_tolerance REAL NOT NULL,
    control_mode TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 2. HIỆU CHUẨN CẢM BIẾN
CREATE TABLE sensor_calibration (
    device_id TEXT NOT NULL PRIMARY KEY,
    ph_v7 REAL NOT NULL,
    ph_v4 REAL NOT NULL,
    ec_factor REAL NOT NULL,
    temp_offset REAL NOT NULL,
    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

-- 3. HIỆU CHUẨN BƠM
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

-- 4. CẤU HÌNH ĐỊNH LƯỢNG (Dosing)
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

-- 5. CẤU HÌNH NƯỚC (Water Config - BẢNG MỚI)
-- Chuyên lo việc vận hành, tuần hoàn và các mốc nước mục tiêu
CREATE TABLE water_config (
    device_id TEXT NOT NULL PRIMARY KEY,
    water_level_min REAL NOT NULL,
    water_level_target REAL NOT NULL,
    water_level_max REAL NOT NULL,
    water_level_drain REAL NOT NULL,
    circulation_mode TEXT NOT NULL,
    circulation_on_sec INTEGER NOT NULL,
    circulation_off_sec INTEGER NOT NULL,
    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

-- 6. CẤU HÌNH AN TOÀN (Safety Config - ĐÃ CẮT GỌT)
-- Chuyên lo việc ngắt khẩn cấp, giới hạn phần cứng, ngăn rủi ro
CREATE TABLE safety_config (
    device_id TEXT NOT NULL PRIMARY KEY,

    -- Giới hạn Dinh dưỡng & pH
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

    -- Cảnh báo Nước & Máy bơm
    water_level_critical_min REAL NOT NULL, -- Mức nước cạn nguy hiểm (cứu rủi ro cháy bơm)
    max_refill_cycles_per_hour INTEGER NOT NULL,
    max_drain_cycles_per_hour INTEGER NOT NULL,
    max_refill_duration_sec INTEGER NOT NULL,
    max_drain_duration_sec INTEGER NOT NULL,

    -- Dừng khẩn cấp toàn hệ thống
    emergency_shutdown INTEGER NOT NULL DEFAULT 0,

    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

-- Indexes for faster queries
CREATE INDEX idx_pump_device ON pump_calibration(device_id);
