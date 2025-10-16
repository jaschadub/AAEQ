use aaeq_core::{Device, Mapping, Scope};
use anyhow::Result;
use sqlx::SqlitePool;
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
        let id = sqlx::query!(
            r#"
            INSERT INTO device (kind, label, host, discovered_at)
            VALUES (?, ?, ?, ?)
            "#,
            device.kind,
            device.label,
            device.host,
            device.discovered_at
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Option<Device>> {
        let device = sqlx::query_as!(
            Device,
            r#"
            SELECT id as "id: i64", kind, label, host, discovered_at
            FROM device
            WHERE id = ?
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(device)
    }

    pub async fn list_all(&self) -> Result<Vec<Device>> {
        let devices = sqlx::query_as!(
            Device,
            r#"
            SELECT id as "id: i64", kind, label, host, discovered_at
            FROM device
            ORDER BY discovered_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(devices)
    }

    pub async fn update_host(&self, id: i64, host: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE device
            SET host = ?
            WHERE id = ?
            "#,
            host,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM device WHERE id = ?
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn sync_presets(&self, device_id: i64, presets: &[String]) -> Result<()> {
        // Delete old presets
        sqlx::query!(
            r#"
            DELETE FROM device_preset WHERE device_id = ?
            "#,
            device_id
        )
        .execute(&self.pool)
        .await?;

        // Insert new presets
        for preset in presets {
            sqlx::query!(
                r#"
                INSERT INTO device_preset (device_id, name)
                VALUES (?, ?)
                "#,
                device_id,
                preset
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_presets(&self, device_id: i64) -> Result<Vec<String>> {
        let presets = sqlx::query_scalar!(
            r#"
            SELECT name
            FROM device_preset
            WHERE device_id = ?
            ORDER BY name
            "#,
            device_id
        )
        .fetch_all(&self.pool)
        .await?;

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

        let id = sqlx::query!(
            r#"
            INSERT INTO mapping (scope, key_normalized, preset_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
            scope_str,
            mapping.key_normalized,
            mapping.preset_name,
            now,
            now
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(id)
    }

    pub async fn upsert(&self, mapping: &Mapping) -> Result<i64> {
        let now = Utc::now().timestamp();
        let scope_str = mapping.scope.as_str();

        let id = sqlx::query!(
            r#"
            INSERT INTO mapping (scope, key_normalized, preset_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(scope, key_normalized)
            DO UPDATE SET preset_name = excluded.preset_name, updated_at = excluded.updated_at
            RETURNING id
            "#,
            scope_str,
            mapping.key_normalized,
            mapping.preset_name,
            now,
            now
        )
        .fetch_one(&self.pool)
        .await?
        .id;

        Ok(id)
    }

    pub async fn list_all(&self) -> Result<Vec<Mapping>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, scope, key_normalized, preset_name, created_at, updated_at
            FROM mapping
            ORDER BY scope, key_normalized
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mappings = rows
            .into_iter()
            .filter_map(|row| {
                let scope = Scope::from_str(&row.scope)?;
                Some(Mapping {
                    id: Some(row.id),
                    scope,
                    key_normalized: row.key_normalized,
                    preset_name: row.preset_name,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                })
            })
            .collect();

        Ok(mappings)
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM mapping WHERE id = ?
            "#,
            id
        )
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

        // SQLite doesn't support ON CONFLICT with no unique constraint, so we use REPLACE
        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO last_applied (id, device_id, last_track_key, last_preset, updated_at)
            VALUES (
                (SELECT id FROM last_applied WHERE device_id = ?),
                ?, ?, ?, ?
            )
            "#,
            device_id,
            device_id,
            track_key,
            preset,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, device_id: i64) -> Result<Option<(String, String)>> {
        let row = sqlx::query!(
            r#"
            SELECT last_track_key, last_preset
            FROM last_applied
            WHERE device_id = ?
            "#,
            device_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| {
            Some((r.last_track_key?, r.last_preset?))
        }))
    }
}

/// Repository for custom EQ preset operations
pub struct CustomEqPresetRepository {
    pool: SqlitePool,
}

impl CustomEqPresetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new custom EQ preset with bands
    pub async fn create(&self, preset: &aaeq_core::EqPreset) -> Result<i64> {
        let now = Utc::now().timestamp();

        let preset_id = sqlx::query!(
            r#"
            INSERT INTO custom_eq_preset (name, created_at, updated_at)
            VALUES (?, ?, ?)
            "#,
            preset.name,
            now,
            now
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        // Insert bands
        for band in &preset.bands {
            sqlx::query!(
                r#"
                INSERT INTO custom_eq_band (preset_id, frequency, gain)
                VALUES (?, ?, ?)
                "#,
                preset_id,
                band.frequency,
                band.gain
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(preset_id)
    }

    /// Update an existing custom EQ preset
    pub async fn update(&self, preset: &aaeq_core::EqPreset, preset_id: i64) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query!(
            r#"
            UPDATE custom_eq_preset
            SET name = ?, updated_at = ?
            WHERE id = ?
            "#,
            preset.name,
            now,
            preset_id
        )
        .execute(&self.pool)
        .await?;

        // Delete old bands
        sqlx::query!(
            r#"
            DELETE FROM custom_eq_band WHERE preset_id = ?
            "#,
            preset_id
        )
        .execute(&self.pool)
        .await?;

        // Insert new bands
        for band in &preset.bands {
            sqlx::query!(
                r#"
                INSERT INTO custom_eq_band (preset_id, frequency, gain)
                VALUES (?, ?, ?)
                "#,
                preset_id,
                band.frequency,
                band.gain
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get a custom EQ preset by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<aaeq_core::EqPreset>> {
        let preset_row = sqlx::query!(
            r#"
            SELECT id, name
            FROM custom_eq_preset
            WHERE name = ?
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(preset_row) = preset_row else {
            return Ok(None);
        };

        let bands = sqlx::query!(
            r#"
            SELECT frequency, gain
            FROM custom_eq_band
            WHERE preset_id = ?
            ORDER BY frequency
            "#,
            preset_row.id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| aaeq_core::EqBand {
            frequency: row.frequency as u32,
            gain: row.gain as f32,
        })
        .collect();

        Ok(Some(aaeq_core::EqPreset {
            name: preset_row.name,
            bands,
        }))
    }

    /// List all custom EQ preset names
    pub async fn list_names(&self) -> Result<Vec<String>> {
        let names = sqlx::query_scalar!(
            r#"
            SELECT name
            FROM custom_eq_preset
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(names)
    }

    /// Delete a custom EQ preset by name
    pub async fn delete(&self, name: &str) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM custom_eq_preset WHERE name = ?
            "#,
            name
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
