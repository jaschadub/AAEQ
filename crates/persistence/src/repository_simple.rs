use aaeq_core::{Device, Mapping, Scope};
use anyhow::Result;
use sqlx::{Row, SqlitePool};
use chrono::Utc;

/// Repository for device operations
pub struct DeviceRepository {
    pool: SqlitePool,
}

impl DeviceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, device: &Device) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO device (kind, label, host, discovered_at) VALUES (?, ?, ?, ?)"
        )
        .bind(&device.kind)
        .bind(&device.label)
        .bind(&device.host)
        .bind(device.discovered_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<Device>> {
        let row = sqlx::query(
            "SELECT id, kind, label, host, discovered_at FROM device WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Device {
            id: Some(r.get(0)),
            kind: r.get(1),
            label: r.get(2),
            host: r.get(3),
            discovered_at: r.get(4),
        }))
    }

    pub async fn list_all(&self) -> Result<Vec<Device>> {
        let rows = sqlx::query(
            "SELECT id, kind, label, host, discovered_at FROM device ORDER BY discovered_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let devices = rows.iter().map(|r| Device {
            id: Some(r.get(0)),
            kind: r.get(1),
            label: r.get(2),
            host: r.get(3),
            discovered_at: r.get(4),
        }).collect();

        Ok(devices)
    }

    pub async fn update_host(&self, id: i64, host: &str) -> Result<()> {
        sqlx::query("UPDATE device SET host = ? WHERE id = ?")
            .bind(host)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM device WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn sync_presets(&self, device_id: i64, presets: &[String]) -> Result<()> {
        // Delete old presets
        sqlx::query("DELETE FROM device_preset WHERE device_id = ?")
            .bind(device_id)
            .execute(&self.pool)
            .await?;

        // Insert new presets
        for preset in presets {
            sqlx::query("INSERT INTO device_preset (device_id, name) VALUES (?, ?)")
                .bind(device_id)
                .bind(preset)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn get_presets(&self, device_id: i64) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT name FROM device_preset WHERE device_id = ? ORDER BY name"
        )
        .bind(device_id)
        .fetch_all(&self.pool)
        .await?;

        let presets = rows.iter().map(|r| r.get(0)).collect();
        Ok(presets)
    }
}

/// Repository for mapping operations
pub struct MappingRepository {
    pool: SqlitePool,
}

impl MappingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, mapping: &Mapping) -> Result<i64> {
        let now = Utc::now().timestamp();
        let scope_str = mapping.scope.as_str();

        let result = sqlx::query(
            "INSERT INTO mapping (scope, key_normalized, preset_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(scope_str)
        .bind(&mapping.key_normalized)
        .bind(&mapping.preset_name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn upsert(&self, mapping: &Mapping) -> Result<i64> {
        let now = Utc::now().timestamp();
        let scope_str = mapping.scope.as_str();

        let result = sqlx::query(
            "INSERT INTO mapping (scope, key_normalized, preset_name, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(scope, key_normalized)
             DO UPDATE SET preset_name = excluded.preset_name, updated_at = excluded.updated_at
             RETURNING id"
        )
        .bind(scope_str)
        .bind(&mapping.key_normalized)
        .bind(&mapping.preset_name)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get(0))
    }

    pub async fn list_all(&self) -> Result<Vec<Mapping>> {
        let rows = sqlx::query(
            "SELECT id, scope, key_normalized, preset_name, created_at, updated_at FROM mapping ORDER BY scope, key_normalized"
        )
        .fetch_all(&self.pool)
        .await?;

        let mappings = rows.iter().filter_map(|row| {
            let scope_str: String = row.get(1);
            let scope = Scope::from_str(&scope_str)?;

            Some(Mapping {
                id: Some(row.get(0)),
                scope,
                key_normalized: row.get(2),
                preset_name: row.get(3),
                created_at: row.get(4),
                updated_at: row.get(5),
            })
        }).collect();

        Ok(mappings)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM mapping WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Repository for genre overrides
pub struct GenreOverrideRepository {
    pool: SqlitePool,
}

impl GenreOverrideRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, track_key: &str, genre: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO genre_override (track_key, genre, created_at, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(track_key)
             DO UPDATE SET genre = excluded.genre, updated_at = excluded.updated_at"
        )
        .bind(track_key)
        .bind(genre)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, track_key: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT genre FROM genre_override WHERE track_key = ?"
        )
        .bind(track_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get(0)))
    }

    pub async fn delete(&self, track_key: &str) -> Result<()> {
        sqlx::query("DELETE FROM genre_override WHERE track_key = ?")
            .bind(track_key)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Repository for app-wide settings
pub struct AppSettingsRepository {
    pool: SqlitePool,
}

impl AppSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_last_connected_host(&self) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT last_connected_host FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get(0)))
    }

    pub async fn set_last_connected_host(&self, host: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "UPDATE app_settings SET last_connected_host = ?, updated_at = ? WHERE id = 1"
        )
        .bind(host)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Repository for tracking last applied state
pub struct LastAppliedRepository {
    pool: SqlitePool,
}

impl LastAppliedRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn update(&self, device_id: i64, track_key: &str, preset: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "INSERT OR REPLACE INTO last_applied (id, device_id, last_track_key, last_preset, updated_at)
             VALUES ((SELECT id FROM last_applied WHERE device_id = ?), ?, ?, ?, ?)"
        )
        .bind(device_id)
        .bind(device_id)
        .bind(track_key)
        .bind(preset)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, device_id: i64) -> Result<Option<(String, String)>> {
        let row = sqlx::query(
            "SELECT last_track_key, last_preset FROM last_applied WHERE device_id = ?"
        )
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| {
            let track_key: Option<String> = r.get(0);
            let preset: Option<String> = r.get(1);
            match (track_key, preset) {
                (Some(t), Some(p)) => Some((t, p)),
                _ => None,
            }
        }))
    }
}
