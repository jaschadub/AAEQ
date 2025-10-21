-- Migration 010: DSP Profile Settings
-- Store DSP configuration (sample rate, buffer, headroom, etc.) per profile

CREATE TABLE IF NOT EXISTS dsp_profile_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER NOT NULL,
    sample_rate INTEGER NOT NULL DEFAULT 48000,
    buffer_ms INTEGER NOT NULL DEFAULT 150,
    headroom_db REAL NOT NULL DEFAULT -3.0,
    auto_compensate INTEGER NOT NULL DEFAULT 0,  -- Boolean: 0 = false, 1 = true
    clip_detection INTEGER NOT NULL DEFAULT 1,   -- Boolean: 0 = false, 1 = true
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE,
    UNIQUE(profile_id)  -- One DSP settings row per profile
);

-- Create index for faster lookups by profile_id
CREATE INDEX IF NOT EXISTS idx_dsp_profile_settings_profile ON dsp_profile_settings(profile_id);

-- Insert default DSP settings for existing profiles
-- Default profile (id=1): -3 dB headroom, standard settings
INSERT OR IGNORE INTO dsp_profile_settings (
    profile_id,
    sample_rate,
    buffer_ms,
    headroom_db,
    auto_compensate,
    clip_detection,
    created_at,
    updated_at
)
SELECT
    id,
    48000,
    150,
    -3.0,
    0,
    1,
    strftime('%s', 'now'),
    strftime('%s', 'now')
FROM profile;
