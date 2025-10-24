-- Migration 015: Managed Devices
-- Store manually-added and favorite devices per profile
-- Supports WiiM API, DLNA, AirPlay, Local DAC, and future ANP protocol

CREATE TABLE IF NOT EXISTS managed_devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER NOT NULL,
    name TEXT NOT NULL,                    -- User-friendly device name
    protocol TEXT NOT NULL,                -- 'WiimApi', 'LocalDac', 'Dlna', 'AirPlay', 'AnpNode'
    address TEXT NOT NULL,                 -- IP address, hostname, or UUID
    source TEXT NOT NULL DEFAULT 'Manual', -- 'Discovered', 'Manual', 'Database'
    favorite INTEGER NOT NULL DEFAULT 0,   -- Boolean: 0 = false, 1 = true
    last_seen INTEGER,                     -- Timestamp of last successful connection (optional)
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

-- Index for faster lookups by profile
CREATE INDEX IF NOT EXISTS idx_managed_devices_profile ON managed_devices(profile_id);

-- Index for faster lookups by protocol
CREATE INDEX IF NOT EXISTS idx_managed_devices_protocol ON managed_devices(protocol);

-- Unique constraint: one device per profile+protocol+address combination
CREATE UNIQUE INDEX IF NOT EXISTS idx_managed_devices_unique
ON managed_devices(profile_id, protocol, address);
