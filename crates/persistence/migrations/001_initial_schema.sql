-- Device table
CREATE TABLE IF NOT EXISTS device (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    host TEXT NOT NULL,
    discovered_at INTEGER NOT NULL
);

-- Device presets (cached from device)
CREATE TABLE IF NOT EXISTS device_preset (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    UNIQUE(device_id, name),
    FOREIGN KEY (device_id) REFERENCES device(id) ON DELETE CASCADE
);

-- Mapping rules
CREATE TABLE IF NOT EXISTS mapping (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL CHECK(scope IN ('song', 'album', 'genre', 'default')),
    key_normalized TEXT,
    preset_name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(scope, key_normalized)
);

-- Last applied state per device
CREATE TABLE IF NOT EXISTS last_applied (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    last_track_key TEXT,
    last_preset TEXT,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (device_id) REFERENCES device(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_mapping_scope ON mapping(scope);
CREATE INDEX IF NOT EXISTS idx_mapping_key ON mapping(key_normalized);
CREATE INDEX IF NOT EXISTS idx_device_preset_device ON device_preset(device_id);
