-- Profile table for managing different listening contexts (headphones, speakers, etc.)
CREATE TABLE IF NOT EXISTS profile (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    is_builtin INTEGER NOT NULL DEFAULT 0,  -- 1 for built-in profiles, 0 for user-created
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Insert built-in profiles
INSERT INTO profile (name, is_builtin, created_at, updated_at)
VALUES
    ('Default', 1, strftime('%s', 'now'), strftime('%s', 'now')),
    ('Headphones', 1, strftime('%s', 'now'), strftime('%s', 'now'));

-- Recreate mapping table with profile_id
-- SQLite doesn't support ADD COLUMN with FOREIGN KEY, so we recreate
CREATE TABLE mapping_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL CHECK(scope IN ('song', 'album', 'genre', 'default')),
    key_normalized TEXT,
    preset_name TEXT NOT NULL,
    profile_id INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(profile_id, scope, key_normalized),
    FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
);

-- Copy existing mappings to new table (all with profile_id=1 for "Default")
INSERT INTO mapping_new (id, scope, key_normalized, preset_name, profile_id, created_at, updated_at)
SELECT id, scope, key_normalized, preset_name, 1, created_at, updated_at
FROM mapping;

-- Drop old table and rename new one
DROP TABLE mapping;
ALTER TABLE mapping_new RENAME TO mapping;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_mapping_profile ON mapping(profile_id);
CREATE INDEX IF NOT EXISTS idx_mapping_scope ON mapping(scope);
CREATE INDEX IF NOT EXISTS idx_mapping_key ON mapping(key_normalized);

-- Recreate app_settings with active_profile_id
-- Save existing data first
CREATE TABLE app_settings_new (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_connected_host TEXT,
    last_input_device TEXT,
    last_output_device TEXT,
    active_profile_id INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (active_profile_id) REFERENCES profile(id)
);

-- Copy existing settings
INSERT INTO app_settings_new (id, last_connected_host, last_input_device, last_output_device, active_profile_id, created_at, updated_at)
SELECT id, last_connected_host, last_input_device, last_output_device, 1, created_at, updated_at
FROM app_settings;

-- Drop old table and rename new one
DROP TABLE app_settings;
ALTER TABLE app_settings_new RENAME TO app_settings;
