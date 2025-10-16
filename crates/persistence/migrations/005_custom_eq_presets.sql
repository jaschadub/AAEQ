-- Custom EQ presets table
CREATE TABLE IF NOT EXISTS custom_eq_preset (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- EQ bands for custom presets
CREATE TABLE IF NOT EXISTS custom_eq_band (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    preset_id INTEGER NOT NULL,
    frequency INTEGER NOT NULL,
    gain REAL NOT NULL,
    FOREIGN KEY (preset_id) REFERENCES custom_eq_preset(id) ON DELETE CASCADE
);

-- Index for performance
CREATE INDEX IF NOT EXISTS idx_custom_eq_band_preset ON custom_eq_band(preset_id);
