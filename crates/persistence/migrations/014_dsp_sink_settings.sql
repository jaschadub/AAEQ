-- Migration 014: DSP Sink Settings
-- Store DSP configuration per sink type (LocalDac, Dlna, AirPlay)
-- This allows different settings for different output types

CREATE TABLE IF NOT EXISTS dsp_sink_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sink_type TEXT NOT NULL UNIQUE,  -- 'LocalDac', 'Dlna', 'AirPlay'
    sample_rate INTEGER NOT NULL DEFAULT 48000,
    format TEXT NOT NULL DEFAULT 'F32',  -- 'S16LE', 'S24LE', 'F32'
    buffer_ms INTEGER NOT NULL DEFAULT 150,
    headroom_db REAL NOT NULL DEFAULT -3.0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Create index for faster lookups by sink_type
CREATE INDEX IF NOT EXISTS idx_dsp_sink_settings_type ON dsp_sink_settings(sink_type);

-- Insert default settings for each sink type
INSERT OR IGNORE INTO dsp_sink_settings (
    sink_type,
    sample_rate,
    format,
    buffer_ms,
    headroom_db,
    created_at,
    updated_at
) VALUES
    ('LocalDac', 48000, 'F32', 150, -3.0, strftime('%s', 'now'), strftime('%s', 'now')),
    ('Dlna', 48000, 'S16LE', 200, -3.0, strftime('%s', 'now'), strftime('%s', 'now')),
    ('AirPlay', 44100, 'S16LE', 300, -3.0, strftime('%s', 'now'), strftime('%s', 'now'));
