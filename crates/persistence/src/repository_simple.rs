use aaeq_core::{Device, Mapping, Profile, Scope};
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
            "INSERT INTO mapping (scope, key_normalized, preset_name, profile_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(scope_str)
        .bind(&mapping.key_normalized)
        .bind(&mapping.preset_name)
        .bind(mapping.profile_id)
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
            "INSERT INTO mapping (scope, key_normalized, preset_name, profile_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(profile_id, scope, key_normalized)
             DO UPDATE SET preset_name = excluded.preset_name, updated_at = excluded.updated_at
             RETURNING id"
        )
        .bind(scope_str)
        .bind(&mapping.key_normalized)
        .bind(&mapping.preset_name)
        .bind(mapping.profile_id)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get(0))
    }

    pub async fn list_all(&self) -> Result<Vec<Mapping>> {
        let rows = sqlx::query(
            "SELECT id, scope, key_normalized, preset_name, profile_id, created_at, updated_at FROM mapping ORDER BY profile_id, scope, key_normalized"
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
                profile_id: row.get(4),
                created_at: row.get(5),
                updated_at: row.get(6),
            })
        }).collect();

        Ok(mappings)
    }

    pub async fn list_by_profile(&self, profile_id: i64) -> Result<Vec<Mapping>> {
        let rows = sqlx::query(
            "SELECT id, scope, key_normalized, preset_name, profile_id, created_at, updated_at FROM mapping WHERE profile_id = ? ORDER BY scope, key_normalized"
        )
        .bind(profile_id)
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
                profile_id: row.get(4),
                created_at: row.get(5),
                updated_at: row.get(6),
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

        // Try to update existing row first
        let result = sqlx::query(
            "UPDATE app_settings SET last_connected_host = ?, updated_at = ? WHERE id = 1"
        )
        .bind(host)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // If no row was updated, insert a new one
        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO app_settings (id, last_connected_host, created_at, updated_at)
                 VALUES (1, ?, ?, ?)"
            )
            .bind(host)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_last_input_device(&self) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT last_input_device FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get(0)))
    }

    pub async fn set_last_input_device(&self, device: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        // Try to update existing row first
        let result = sqlx::query(
            "UPDATE app_settings SET last_input_device = ?, updated_at = ? WHERE id = 1"
        )
        .bind(device)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // If no row was updated, insert a new one
        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO app_settings (id, last_input_device, created_at, updated_at)
                 VALUES (1, ?, ?, ?)"
            )
            .bind(device)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_last_output_device(&self) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT last_output_device FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get(0)))
    }

    pub async fn set_last_output_device(&self, device: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        // Try to update existing row first
        let result = sqlx::query(
            "UPDATE app_settings SET last_output_device = ?, updated_at = ? WHERE id = 1"
        )
        .bind(device)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // If no row was updated, insert a new one
        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO app_settings (id, last_output_device, created_at, updated_at)
                 VALUES (1, ?, ?, ?)"
            )
            .bind(device)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_active_profile_id(&self) -> Result<Option<i64>> {
        let row = sqlx::query(
            "SELECT active_profile_id FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get(0)))
    }

    pub async fn set_active_profile_id(&self, profile_id: i64) -> Result<()> {
        let now = Utc::now().timestamp();

        // Try to update existing row first
        let result = sqlx::query(
            "UPDATE app_settings SET active_profile_id = ?, updated_at = ? WHERE id = 1"
        )
        .bind(profile_id)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // If no row was updated, insert a new one
        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO app_settings (id, active_profile_id, created_at, updated_at)
                 VALUES (1, ?, ?, ?)"
            )
            .bind(profile_id)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_theme(&self) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT theme FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get(0)))
    }

    pub async fn set_theme(&self, theme: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        // Try to update existing row first
        let result = sqlx::query(
            "UPDATE app_settings SET theme = ?, updated_at = ? WHERE id = 1"
        )
        .bind(theme)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // If no row was updated, insert a new one
        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO app_settings (id, theme, created_at, updated_at)
                 VALUES (1, ?, ?, ?)"
            )
            .bind(theme)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_auto_reconnect(&self) -> Result<Option<bool>> {
        let row = sqlx::query(
            "SELECT auto_reconnect FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| {
            let value: Option<i32> = r.get(0);
            value.map(|v| v != 0)
        }))
    }

    pub async fn set_auto_reconnect(&self, auto_reconnect: bool) -> Result<()> {
        let now = Utc::now().timestamp();
        let value = if auto_reconnect { 1 } else { 0 };

        // Try to update existing row first
        let result = sqlx::query(
            "UPDATE app_settings SET auto_reconnect = ?, updated_at = ? WHERE id = 1"
        )
        .bind(value)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // If no row was updated, insert a new one
        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO app_settings (id, auto_reconnect, created_at, updated_at)
                 VALUES (1, ?, ?, ?)"
            )
            .bind(value)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

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

        let result = sqlx::query(
            "INSERT INTO custom_eq_preset (name, created_at, updated_at) VALUES (?, ?, ?)"
        )
        .bind(&preset.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        let preset_id = result.last_insert_rowid();

        // Insert bands
        for band in &preset.bands {
            sqlx::query(
                "INSERT INTO custom_eq_band (preset_id, frequency, gain) VALUES (?, ?, ?)"
            )
            .bind(preset_id)
            .bind(band.frequency)
            .bind(band.gain)
            .execute(&self.pool)
            .await?;
        }

        Ok(preset_id)
    }

    /// Update an existing custom EQ preset (upsert by name)
    pub async fn upsert(&self, preset: &aaeq_core::EqPreset) -> Result<i64> {
        let now = Utc::now().timestamp();

        // Try to get existing preset ID
        let existing = sqlx::query("SELECT id FROM custom_eq_preset WHERE name = ?")
            .bind(&preset.name)
            .fetch_optional(&self.pool)
            .await?;

        let preset_id = if let Some(row) = existing {
            let id: i64 = row.get(0);

            // Update timestamp
            sqlx::query("UPDATE custom_eq_preset SET updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;

            // Delete old bands
            sqlx::query("DELETE FROM custom_eq_band WHERE preset_id = ?")
                .bind(id)
                .execute(&self.pool)
                .await?;

            id
        } else {
            // Insert new preset
            let result = sqlx::query(
                "INSERT INTO custom_eq_preset (name, created_at, updated_at) VALUES (?, ?, ?)"
            )
            .bind(&preset.name)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;

            result.last_insert_rowid()
        };

        // Insert bands
        for band in &preset.bands {
            sqlx::query(
                "INSERT INTO custom_eq_band (preset_id, frequency, gain) VALUES (?, ?, ?)"
            )
            .bind(preset_id)
            .bind(band.frequency)
            .bind(band.gain)
            .execute(&self.pool)
            .await?;
        }

        Ok(preset_id)
    }

    /// Get a custom EQ preset by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<aaeq_core::EqPreset>> {
        let preset_row = sqlx::query(
            "SELECT id, name FROM custom_eq_preset WHERE name = ?"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        let Some(preset_row) = preset_row else {
            return Ok(None);
        };

        let preset_id: i64 = preset_row.get(0);
        let preset_name: String = preset_row.get(1);

        let band_rows = sqlx::query(
            "SELECT frequency, gain FROM custom_eq_band WHERE preset_id = ? ORDER BY frequency"
        )
        .bind(preset_id)
        .fetch_all(&self.pool)
        .await?;

        let bands = band_rows.iter().map(|row| {
            let frequency: i64 = row.get(0);
            let gain: f64 = row.get(1);
            aaeq_core::EqBand {
                frequency: frequency as u32,
                gain: gain as f32,
            }
        }).collect();

        Ok(Some(aaeq_core::EqPreset {
            name: preset_name,
            bands,
        }))
    }

    /// List all custom EQ preset names
    pub async fn list_names(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT name FROM custom_eq_preset ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await?;

        let names = rows.iter().map(|r| r.get(0)).collect();
        Ok(names)
    }

    /// Delete a custom EQ preset by name
    pub async fn delete(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM custom_eq_preset WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Repository for profile operations
pub struct ProfileRepository {
    pool: SqlitePool,
}

impl ProfileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new profile
    pub async fn create(&self, profile: &Profile) -> Result<i64> {
        let now = Utc::now().timestamp();
        let is_builtin = if profile.is_builtin { 1 } else { 0 };

        let result = sqlx::query(
            "INSERT INTO profile (name, is_builtin, created_at, updated_at) VALUES (?, ?, ?, ?)"
        )
        .bind(&profile.name)
        .bind(is_builtin)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get a profile by ID
    pub async fn get_by_id(&self, id: i64) -> Result<Option<Profile>> {
        let row = sqlx::query(
            "SELECT id, name, is_builtin, created_at, updated_at FROM profile WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Profile {
            id: Some(r.get(0)),
            name: r.get(1),
            is_builtin: {
                let val: i64 = r.get(2);
                val != 0
            },
            created_at: r.get(3),
            updated_at: r.get(4),
        }))
    }

    /// Get a profile by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<Profile>> {
        let row = sqlx::query(
            "SELECT id, name, is_builtin, created_at, updated_at FROM profile WHERE name = ?"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Profile {
            id: Some(r.get(0)),
            name: r.get(1),
            is_builtin: {
                let val: i64 = r.get(2);
                val != 0
            },
            created_at: r.get(3),
            updated_at: r.get(4),
        }))
    }

    /// List all profiles
    pub async fn list_all(&self) -> Result<Vec<Profile>> {
        let rows = sqlx::query(
            "SELECT id, name, is_builtin, created_at, updated_at FROM profile ORDER BY is_builtin DESC, name"
        )
        .fetch_all(&self.pool)
        .await?;

        let profiles = rows.iter().map(|r| Profile {
            id: Some(r.get(0)),
            name: r.get(1),
            is_builtin: {
                let val: i64 = r.get(2);
                val != 0
            },
            created_at: r.get(3),
            updated_at: r.get(4),
        }).collect();

        Ok(profiles)
    }

    /// Update a profile name (only for user-created profiles)
    pub async fn update_name(&self, id: i64, name: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "UPDATE profile SET name = ?, updated_at = ? WHERE id = ? AND is_builtin = 0"
        )
        .bind(name)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a profile (only for user-created profiles)
    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM profile WHERE id = ? AND is_builtin = 0")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
