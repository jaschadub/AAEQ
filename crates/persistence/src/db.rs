use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::migrate::MigrateDatabase;
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

    // Create initial schema
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

    Ok(())
}
