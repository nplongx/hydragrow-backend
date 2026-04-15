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

