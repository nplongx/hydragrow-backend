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

-- 2. CẤU HÌNH & HIỆU CHUẨN SENSOR NODE (Đã cập nhật)
CREATE TABLE sensor_calibration (
    device_id TEXT NOT NULL PRIMARY KEY,
    
    -- [MỚI] HIỆU CHUẨN CẢM BIẾN
    ph_v7 REAL NOT NULL,
    ph_v4 REAL NOT NULL,
    ec_factor REAL NOT NULL,
    ec_offset REAL NOT NULL DEFAULT 0.0,
    temp_offset REAL NOT NULL,
    temp_compensation_beta REAL NOT NULL DEFAULT 0.02,
    
    -- [MỚI] CẤU HÌNH ĐỌC & LỌC NHIỄU (SAMPLING & FILTERING)
    sampling_interval INTEGER NOT NULL DEFAULT 1000,
    publish_interval INTEGER NOT NULL DEFAULT 5000,
    moving_average_window INTEGER NOT NULL DEFAULT 10,
    
    -- [MỚI] TRẠNG THÁI CẢM BIẾN (HARDWARE FLAGS) - Dùng INTEGER (0, 1) thay cho boolean
    is_ph_enabled INTEGER NOT NULL DEFAULT 1,
    is_ec_enabled INTEGER NOT NULL DEFAULT 1,
    is_temp_enabled INTEGER NOT NULL DEFAULT 1,
    is_water_level_enabled INTEGER NOT NULL DEFAULT 1,
    
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

-- 4. CẤU HÌNH ĐỊNH LƯỢNG (Dosing) - ĐÃ CẬP NHẬT THEO ESP32
CREATE TABLE dosing_calibration (
    device_id TEXT NOT NULL PRIMARY KEY,
    tank_volume_l REAL NOT NULL,
    ec_gain_per_ml REAL NOT NULL,
    ph_shift_up_per_ml REAL NOT NULL,
    ph_shift_down_per_ml REAL NOT NULL,
    
    -- Thời gian khuấy và ổn định cảm biến
    active_mixing_sec INTEGER NOT NULL,
    sensor_stabilize_sec INTEGER NOT NULL,
    
    ec_step_ratio REAL NOT NULL,
    ph_step_ratio REAL NOT NULL,
    
    -- Công suất bơm dùng để tính toán thời gian bơm
    dosing_pump_capacity_ml_per_sec REAL NOT NULL DEFAULT 0.5,
    
    last_calibrated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

-- 5. CẤU HÌNH NƯỚC (Water Config)
CREATE TABLE water_config (
    device_id TEXT NOT NULL PRIMARY KEY,
    water_level_min REAL NOT NULL,
    water_level_target REAL NOT NULL,
    water_level_max REAL NOT NULL,
    water_level_drain REAL NOT NULL,
    circulation_mode TEXT NOT NULL,
    circulation_on_sec INTEGER NOT NULL,
    circulation_off_sec INTEGER NOT NULL,
    
    -- Các biến liên quan đến tự động điều tiết nước
    water_level_tolerance REAL NOT NULL DEFAULT 1.0,
    auto_refill_enabled INTEGER NOT NULL DEFAULT 1,
    auto_drain_overflow INTEGER NOT NULL DEFAULT 1,
    
    -- Tính năng pha loãng (Dilution) khi EC quá cao
    auto_dilute_enabled INTEGER NOT NULL DEFAULT 1,
    dilute_drain_amount_cm REAL NOT NULL DEFAULT 2.0,
    
    -- Tính năng lịch thay nước (Schedule)
    scheduled_water_change_enabled INTEGER NOT NULL DEFAULT 0,
    water_change_interval_sec INTEGER NOT NULL DEFAULT 259200,
    scheduled_drain_amount_cm REAL NOT NULL DEFAULT 5.0,

    last_updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

-- 6. CẤU HÌNH AN TOÀN (Safety Config - Chuyên lo việc ngắt khẩn cấp, ngăn rủi ro)
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

CREATE TABLE blockchain_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    device_id TEXT NOT NULL,
    action TEXT NOT NULL,        -- VD: 'dosing_report', 'manual_pump_override'
    tx_id TEXT NOT NULL UNIQUE,  -- Mã giao dịch trên Solana (Không được trùng)
    explorer_url TEXT NOT NULL,  -- Link xem trên Solscan
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device_config(device_id)
);

CREATE INDEX idx_blockchain_device ON blockchain_history(device_id);

-- Indexes for faster queries
CREATE INDEX idx_pump_device ON pump_calibration(device_id);
