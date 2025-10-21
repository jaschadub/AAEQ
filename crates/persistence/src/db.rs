use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::migrate::MigrateDatabase;
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;

/// Initialize database connection and run migrations
pub async fn init_db(db_path: &Path) -> Result<SqlitePool> {
    let db_url = format!("sqlite://{}", db_path.display());

    // Create database file if it doesn't exist
    if !sqlx::Sqlite::database_exists(&db_url).await? {
        tracing::info!("Creating database at {}", db_path.display());
        sqlx::Sqlite::create_database(&db_url).await?;
    }

    let options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Run migrations manually (inline the SQL)
    run_migrations(&pool).await?;

    tracing::info!("Database initialized successfully");
    Ok(pool)
}

/// Run database migrations
async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    tracing::info!("Running database migrations");

    // Migration 001: Initial schema
    sqlx::query(r#"
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
    "#)
    .execute(pool)
    .await?;

    // Migration 002: Genre overrides
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS genre_override (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_key TEXT NOT NULL UNIQUE,
            genre TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_genre_override_track ON genre_override(track_key);
    "#)
    .execute(pool)
    .await?;

    // Migration 003: App settings
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            last_connected_host TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
    "#)
    .execute(pool)
    .await?;

    // Migration 004: Add last_input_device to app_settings
    // Check if column exists first (SQLite doesn't have IF NOT EXISTS for ALTER TABLE)
    let column_exists = sqlx::query(
        "SELECT COUNT(*) as count FROM pragma_table_info('app_settings') WHERE name = 'last_input_device'"
    )
    .fetch_one(pool)
    .await?
    .get::<i32, _>("count") > 0;

    if !column_exists {
        sqlx::query("ALTER TABLE app_settings ADD COLUMN last_input_device TEXT")
            .execute(pool)
            .await?;
        tracing::info!("Added last_input_device column to app_settings");
    }

    // Migration 005: Custom EQ presets
    sqlx::query(r#"
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
    "#)
    .execute(pool)
    .await?;

    // Migration 006: Add last_output_device to app_settings
    let output_column_exists = sqlx::query(
        "SELECT COUNT(*) as count FROM pragma_table_info('app_settings') WHERE name = 'last_output_device'"
    )
    .fetch_one(pool)
    .await?
    .get::<i32, _>("count") > 0;

    if !output_column_exists {
        sqlx::query("ALTER TABLE app_settings ADD COLUMN last_output_device TEXT")
            .execute(pool)
            .await?;
        tracing::info!("Added last_output_device column to app_settings");
    }

    // Migration 007: Profiles
    // Check if profile table exists
    let profile_table_exists = sqlx::query(
        "SELECT COUNT(*) as count FROM sqlite_master WHERE type='table' AND name='profile'"
    )
    .fetch_one(pool)
    .await?
    .get::<i32, _>("count") > 0;

    if !profile_table_exists {
        // Create profile table
        sqlx::query(r#"
            CREATE TABLE profile (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                is_builtin INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            INSERT INTO profile (name, is_builtin, created_at, updated_at)
            VALUES
                ('Default', 1, strftime('%s', 'now'), strftime('%s', 'now')),
                ('Headphones', 1, strftime('%s', 'now'), strftime('%s', 'now'));
        "#)
        .execute(pool)
        .await?;

        // Recreate mapping table with profile_id in a single transaction
        sqlx::query(r#"
            -- Create new mapping table with profile support
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

            -- Drop old table
            DROP TABLE mapping;

            -- Rename new table
            ALTER TABLE mapping_new RENAME TO mapping;
        "#)
        .execute(pool)
        .await?;

        // Recreate indexes
        sqlx::query(r#"
            CREATE INDEX IF NOT EXISTS idx_mapping_profile ON mapping(profile_id);
            CREATE INDEX IF NOT EXISTS idx_mapping_scope ON mapping(scope);
            CREATE INDEX IF NOT EXISTS idx_mapping_key ON mapping(key_normalized);
        "#)
        .execute(pool)
        .await?;

        // Recreate app_settings with active_profile_id in a single transaction
        sqlx::query(r#"
            -- Create new app_settings table with active_profile_id
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

            -- Drop old table
            DROP TABLE app_settings;

            -- Rename new table
            ALTER TABLE app_settings_new RENAME TO app_settings;
        "#)
        .execute(pool)
        .await?;

        tracing::info!("Added profile support with Default and Headphones profiles");
    }

    // Migration 008: Add theme to app_settings
    let theme_column_exists = sqlx::query(
        "SELECT COUNT(*) as count FROM pragma_table_info('app_settings') WHERE name = 'theme'"
    )
    .fetch_one(pool)
    .await?
    .get::<i32, _>("count") > 0;

    if !theme_column_exists {
        sqlx::query("ALTER TABLE app_settings ADD COLUMN theme TEXT DEFAULT 'dark'")
            .execute(pool)
            .await?;
        tracing::info!("Added theme column to app_settings");
    }

    // Migration 009: Add auto_reconnect to app_settings
    let auto_reconnect_column_exists = sqlx::query(
        "SELECT COUNT(*) as count FROM pragma_table_info('app_settings') WHERE name = 'auto_reconnect'"
    )
    .fetch_one(pool)
    .await?
    .get::<i32, _>("count") > 0;

    if !auto_reconnect_column_exists {
        sqlx::query("ALTER TABLE app_settings ADD COLUMN auto_reconnect INTEGER DEFAULT 1")
            .execute(pool)
            .await?;
        tracing::info!("Added auto_reconnect column to app_settings");

        // Update existing rows to have auto_reconnect = 1 (enabled by default)
        // The DEFAULT in ALTER TABLE only applies to new rows, not existing ones
        sqlx::query("UPDATE app_settings SET auto_reconnect = 1 WHERE auto_reconnect IS NULL")
            .execute(pool)
            .await?;
        tracing::info!("Set auto_reconnect = 1 for existing app_settings rows");
    }

    // Migration 010: DSP Profile Settings
    let dsp_settings_table_exists = sqlx::query(
        "SELECT COUNT(*) as count FROM sqlite_master WHERE type='table' AND name='dsp_profile_settings'"
    )
    .fetch_one(pool)
    .await?
    .get::<i32, _>("count") > 0;

    if !dsp_settings_table_exists {
        sqlx::query(r#"
            CREATE TABLE dsp_profile_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_id INTEGER NOT NULL,
                sample_rate INTEGER NOT NULL DEFAULT 48000,
                buffer_ms INTEGER NOT NULL DEFAULT 150,
                headroom_db REAL NOT NULL DEFAULT -3.0,
                auto_compensate INTEGER NOT NULL DEFAULT 0,
                clip_detection INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE,
                UNIQUE(profile_id)
            );

            CREATE INDEX idx_dsp_profile_settings_profile ON dsp_profile_settings(profile_id);
        "#)
        .execute(pool)
        .await?;

        // Insert default DSP settings for existing profiles
        sqlx::query(r#"
            INSERT OR IGNORE INTO dsp_profile_settings (
                profile_id, sample_rate, buffer_ms, headroom_db,
                auto_compensate, clip_detection, created_at, updated_at
            )
            SELECT id, 48000, 150, -3.0, 0, 1,
                   strftime('%s', 'now'), strftime('%s', 'now')
            FROM profile
        "#)
        .execute(pool)
        .await?;

        tracing::info!("Added DSP profile settings table with defaults for existing profiles");
    }

    tracing::info!("Database migrations completed");
    Ok(())
}
