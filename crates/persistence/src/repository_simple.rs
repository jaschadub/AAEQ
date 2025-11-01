use aaeq_core::{Device, DspSettings, DspSinkSettings, Mapping, Profile, Scope};
use anyhow::Result;
use sqlx::{Row, SqlitePool};
use chrono::Utc;
use std::str::FromStr;

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
            let scope = Scope::from_str(&scope_str).ok()?;

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
            let scope = Scope::from_str(&scope_str).ok()?;

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

    /// Update all mappings that reference a specific preset to use a new preset name
    /// Useful when a preset is deleted and all references should revert to "Flat"
    pub async fn update_preset_references(&self, old_preset: &str, new_preset: &str) -> Result<usize> {
        let now = Utc::now().timestamp();

        let result = sqlx::query(
            "UPDATE mapping SET preset_name = ?, updated_at = ? WHERE preset_name = ?"
        )
        .bind(new_preset)
        .bind(now)
        .bind(old_preset)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
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

    pub async fn get_enable_debug_logging(&self) -> Result<bool> {
        let row = sqlx::query(
            "SELECT enable_debug_logging FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| {
            let value: Option<i32> = r.get(0);
            value.map(|v| v != 0)
        }).unwrap_or(false))
    }

    pub async fn set_enable_debug_logging(&self, enabled: bool) -> Result<()> {
        let now = Utc::now().timestamp();
        let value = if enabled { 1 } else { 0 };

        // Try to update existing row first
        let result = sqlx::query(
            "UPDATE app_settings SET enable_debug_logging = ?, updated_at = ? WHERE id = 1"
        )
        .bind(value)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // If no row was updated, insert a new one
        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO app_settings (id, enable_debug_logging, created_at, updated_at)
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

    // Global hotkey settings
    pub async fn get_hotkey_enabled(&self) -> Result<bool> {
        let row = sqlx::query(
            "SELECT hotkey_enabled FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| {
            let value: Option<i32> = r.get(0);
            value.map(|v| v != 0)
        }).unwrap_or(true)) // Default to enabled
    }

    pub async fn set_hotkey_enabled(&self, enabled: bool) -> Result<()> {
        let now = Utc::now().timestamp();
        let value = if enabled { 1 } else { 0 };

        sqlx::query(
            "UPDATE app_settings SET hotkey_enabled = ?, updated_at = ? WHERE id = 1"
        )
        .bind(value)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_hotkey_modifiers(&self) -> Result<String> {
        let row = sqlx::query(
            "SELECT hotkey_modifiers FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get(0)).unwrap_or_else(|| "Ctrl+Shift".to_string()))
    }

    pub async fn get_hotkey_key(&self) -> Result<String> {
        let row = sqlx::query(
            "SELECT hotkey_key FROM app_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get(0)).unwrap_or_else(|| "A".to_string()))
    }

    pub async fn set_hotkey(&self, modifiers: &str, key: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "UPDATE app_settings SET hotkey_modifiers = ?, hotkey_key = ?, updated_at = ? WHERE id = 1"
        )
        .bind(modifiers)
        .bind(key)
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
            curve_data: None,
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
            "INSERT INTO profile (name, is_builtin, icon, color, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&profile.name)
        .bind(is_builtin)
        .bind(&profile.icon)
        .bind(&profile.color)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get a profile by ID
    pub async fn get_by_id(&self, id: i64) -> Result<Option<Profile>> {
        let row = sqlx::query(
            "SELECT id, name, is_builtin, icon, color, created_at, updated_at FROM profile WHERE id = ?"
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
            icon: r.get(3),
            color: r.get(4),
            created_at: r.get(5),
            updated_at: r.get(6),
        }))
    }

    /// Get a profile by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<Profile>> {
        let row = sqlx::query(
            "SELECT id, name, is_builtin, icon, color, created_at, updated_at FROM profile WHERE name = ?"
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
            icon: r.get(3),
            color: r.get(4),
            created_at: r.get(5),
            updated_at: r.get(6),
        }))
    }

    /// List all profiles
    pub async fn list_all(&self) -> Result<Vec<Profile>> {
        let rows = sqlx::query(
            "SELECT id, name, is_builtin, icon, color, created_at, updated_at FROM profile ORDER BY is_builtin DESC, name"
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
            icon: r.get(3),
            color: r.get(4),
            created_at: r.get(5),
            updated_at: r.get(6),
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

    /// Update a profile (name, icon, color) (only for user-created profiles)
    pub async fn update(&self, id: i64, name: &str, icon: &str, color: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "UPDATE profile SET name = ?, icon = ?, color = ?, updated_at = ? WHERE id = ? AND is_builtin = 0"
        )
        .bind(name)
        .bind(icon)
        .bind(color)
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

/// Repository for DSP settings operations
pub struct DspSettingsRepository {
    pool: SqlitePool,
}

impl DspSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get DSP settings for a specific profile
    pub async fn get_by_profile(&self, profile_id: i64) -> Result<Option<DspSettings>> {
        let row = sqlx::query(
            r#"SELECT id, profile_id, sample_rate, buffer_ms, headroom_db,
                      auto_compensate, clip_detection,
                      dither_enabled, dither_mode, noise_shaping, target_bits,
                      resample_enabled, resample_quality, target_sample_rate,
                      tube_warmth_enabled, tape_saturation_enabled, transformer_enabled,
                      exciter_enabled, transient_enhancer_enabled,
                      compressor_enabled, limiter_enabled, expander_enabled,
                      stereo_width_enabled, crossfeed_enabled, room_ambience_enabled,
                      created_at, updated_at
               FROM dsp_profile_settings
               WHERE profile_id = ?"#
        )
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| DspSettings {
            id: Some(r.get(0)),
            profile_id: r.get(1),
            sample_rate: r.get(2),
            buffer_ms: r.get(3),
            headroom_db: r.get(4),
            auto_compensate: r.get::<i32, _>(5) != 0, // Convert SQLite integer to bool
            clip_detection: r.get::<i32, _>(6) != 0,   // Convert SQLite integer to bool
            dither_enabled: r.get::<i32, _>(7) != 0,   // Convert SQLite integer to bool
            dither_mode: r.get(8),
            noise_shaping: r.get(9),
            target_bits: r.get::<i32, _>(10) as u8,    // Convert i32 to u8
            resample_enabled: r.get::<i32, _>(11) != 0, // Convert SQLite integer to bool
            resample_quality: r.get(12),
            target_sample_rate: r.get(13),
            // DSP Enhancers
            tube_warmth_enabled: r.get::<i32, _>(14) != 0,
            tape_saturation_enabled: r.get::<i32, _>(15) != 0,
            transformer_enabled: r.get::<i32, _>(16) != 0,
            exciter_enabled: r.get::<i32, _>(17) != 0,
            transient_enhancer_enabled: r.get::<i32, _>(18) != 0,
            compressor_enabled: r.get::<i32, _>(19) != 0,
            limiter_enabled: r.get::<i32, _>(20) != 0,
            expander_enabled: r.get::<i32, _>(21) != 0,
            stereo_width_enabled: r.get::<i32, _>(22) != 0,
            crossfeed_enabled: r.get::<i32, _>(23) != 0,
            room_ambience_enabled: r.get::<i32, _>(24) != 0,
            created_at: r.get(25),
            updated_at: r.get(26),
        }))
    }

    /// Create or update DSP settings for a profile
    pub async fn upsert(&self, settings: &DspSettings) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"INSERT INTO dsp_profile_settings
               (profile_id, sample_rate, buffer_ms, headroom_db,
                auto_compensate, clip_detection,
                dither_enabled, dither_mode, noise_shaping, target_bits,
                resample_enabled, resample_quality, target_sample_rate,
                tube_warmth_enabled, tape_saturation_enabled, transformer_enabled,
                exciter_enabled, transient_enhancer_enabled,
                compressor_enabled, limiter_enabled, expander_enabled,
                stereo_width_enabled, crossfeed_enabled, room_ambience_enabled,
                created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(profile_id) DO UPDATE SET
                   sample_rate = excluded.sample_rate,
                   buffer_ms = excluded.buffer_ms,
                   headroom_db = excluded.headroom_db,
                   auto_compensate = excluded.auto_compensate,
                   clip_detection = excluded.clip_detection,
                   dither_enabled = excluded.dither_enabled,
                   dither_mode = excluded.dither_mode,
                   noise_shaping = excluded.noise_shaping,
                   target_bits = excluded.target_bits,
                   resample_enabled = excluded.resample_enabled,
                   resample_quality = excluded.resample_quality,
                   target_sample_rate = excluded.target_sample_rate,
                   tube_warmth_enabled = excluded.tube_warmth_enabled,
                   tape_saturation_enabled = excluded.tape_saturation_enabled,
                   transformer_enabled = excluded.transformer_enabled,
                   exciter_enabled = excluded.exciter_enabled,
                   transient_enhancer_enabled = excluded.transient_enhancer_enabled,
                   compressor_enabled = excluded.compressor_enabled,
                   limiter_enabled = excluded.limiter_enabled,
                   expander_enabled = excluded.expander_enabled,
                   stereo_width_enabled = excluded.stereo_width_enabled,
                   crossfeed_enabled = excluded.crossfeed_enabled,
                   room_ambience_enabled = excluded.room_ambience_enabled,
                   updated_at = ?
            "#
        )
        .bind(settings.profile_id)
        .bind(settings.sample_rate)
        .bind(settings.buffer_ms)
        .bind(settings.headroom_db)
        .bind(if settings.auto_compensate { 1 } else { 0 }) // Convert bool to integer
        .bind(if settings.clip_detection { 1 } else { 0 })   // Convert bool to integer
        .bind(if settings.dither_enabled { 1 } else { 0 })   // Convert bool to integer
        .bind(&settings.dither_mode)
        .bind(&settings.noise_shaping)
        .bind(settings.target_bits as i32)                   // Convert u8 to i32 for SQLite
        .bind(if settings.resample_enabled { 1 } else { 0 }) // Convert bool to integer
        .bind(&settings.resample_quality)
        .bind(settings.target_sample_rate)
        // DSP Enhancers
        .bind(if settings.tube_warmth_enabled { 1 } else { 0 })
        .bind(if settings.tape_saturation_enabled { 1 } else { 0 })
        .bind(if settings.transformer_enabled { 1 } else { 0 })
        .bind(if settings.exciter_enabled { 1 } else { 0 })
        .bind(if settings.transient_enhancer_enabled { 1 } else { 0 })
        .bind(if settings.compressor_enabled { 1 } else { 0 })
        .bind(if settings.limiter_enabled { 1 } else { 0 })
        .bind(if settings.expander_enabled { 1 } else { 0 })
        .bind(if settings.stereo_width_enabled { 1 } else { 0 })
        .bind(if settings.crossfeed_enabled { 1 } else { 0 })
        .bind(if settings.room_ambience_enabled { 1 } else { 0 })
        .bind(now)
        .bind(now)
        .bind(now) // For the UPDATE SET updated_at
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete DSP settings for a profile
    pub async fn delete(&self, profile_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM dsp_profile_settings WHERE profile_id = ?")
            .bind(profile_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get all DSP settings (useful for debugging/admin)
    pub async fn list_all(&self) -> Result<Vec<DspSettings>> {
        let rows = sqlx::query(
            r#"SELECT id, profile_id, sample_rate, buffer_ms, headroom_db,
                      auto_compensate, clip_detection,
                      dither_enabled, dither_mode, noise_shaping, target_bits,
                      resample_enabled, resample_quality, target_sample_rate,
                      tube_warmth_enabled, tape_saturation_enabled, transformer_enabled,
                      exciter_enabled, transient_enhancer_enabled,
                      compressor_enabled, limiter_enabled, expander_enabled,
                      stereo_width_enabled, crossfeed_enabled, room_ambience_enabled,
                      created_at, updated_at
               FROM dsp_profile_settings
               ORDER BY profile_id"#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| DspSettings {
            id: Some(r.get(0)),
            profile_id: r.get(1),
            sample_rate: r.get(2),
            buffer_ms: r.get(3),
            headroom_db: r.get(4),
            auto_compensate: r.get::<i32, _>(5) != 0,
            clip_detection: r.get::<i32, _>(6) != 0,
            dither_enabled: r.get::<i32, _>(7) != 0,
            dither_mode: r.get(8),
            noise_shaping: r.get(9),
            target_bits: r.get::<i32, _>(10) as u8,
            resample_enabled: r.get::<i32, _>(11) != 0,
            resample_quality: r.get(12),
            target_sample_rate: r.get(13),
            // DSP Enhancers
            tube_warmth_enabled: r.get::<i32, _>(14) != 0,
            tape_saturation_enabled: r.get::<i32, _>(15) != 0,
            transformer_enabled: r.get::<i32, _>(16) != 0,
            exciter_enabled: r.get::<i32, _>(17) != 0,
            transient_enhancer_enabled: r.get::<i32, _>(18) != 0,
            compressor_enabled: r.get::<i32, _>(19) != 0,
            limiter_enabled: r.get::<i32, _>(20) != 0,
            expander_enabled: r.get::<i32, _>(21) != 0,
            stereo_width_enabled: r.get::<i32, _>(22) != 0,
            crossfeed_enabled: r.get::<i32, _>(23) != 0,
            room_ambience_enabled: r.get::<i32, _>(24) != 0,
            created_at: r.get(25),
            updated_at: r.get(26),
        }).collect())
    }
}

/// Repository for DSP sink settings operations (per-sink-type configuration)
pub struct DspSinkSettingsRepository {
    pool: SqlitePool,
}

impl DspSinkSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get DSP settings for a specific sink type
    pub async fn get_by_sink_type(&self, sink_type: &str) -> Result<Option<DspSinkSettings>> {
        let row = sqlx::query(
            r#"SELECT id, sink_type, sample_rate, format, buffer_ms, headroom_db,
                      created_at, updated_at
               FROM dsp_sink_settings
               WHERE sink_type = ?"#
        )
        .bind(sink_type)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| DspSinkSettings {
            id: Some(r.get(0)),
            sink_type: r.get(1),
            sample_rate: r.get(2),
            format: r.get(3),
            buffer_ms: r.get(4),
            headroom_db: r.get(5),
            created_at: r.get(6),
            updated_at: r.get(7),
        }))
    }

    /// Save or update DSP settings for a specific sink type
    pub async fn upsert(&self, settings: &DspSinkSettings) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"INSERT INTO dsp_sink_settings
               (sink_type, sample_rate, format, buffer_ms, headroom_db, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(sink_type) DO UPDATE SET
                   sample_rate = excluded.sample_rate,
                   format = excluded.format,
                   buffer_ms = excluded.buffer_ms,
                   headroom_db = excluded.headroom_db,
                   updated_at = ?
            "#
        )
        .bind(&settings.sink_type)
        .bind(settings.sample_rate)
        .bind(&settings.format)
        .bind(settings.buffer_ms)
        .bind(settings.headroom_db)
        .bind(now)
        .bind(now)
        .bind(now) // For the UPDATE SET updated_at
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List all sink settings
    pub async fn list_all(&self) -> Result<Vec<DspSinkSettings>> {
        let rows = sqlx::query(
            r#"SELECT id, sink_type, sample_rate, format, buffer_ms, headroom_db,
                      created_at, updated_at
               FROM dsp_sink_settings
               ORDER BY sink_type"#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| DspSinkSettings {
            id: Some(r.get(0)),
            sink_type: r.get(1),
            sample_rate: r.get(2),
            format: r.get(3),
            buffer_ms: r.get(4),
            headroom_db: r.get(5),
            created_at: r.get(6),
            updated_at: r.get(7),
        }).collect())
    }
}

/// Repository for managed device operations (per-profile device management)
pub struct ManagedDeviceRepository {
    pool: SqlitePool,
}

impl ManagedDeviceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new managed device
    pub async fn create(&self, device: &aaeq_core::ManagedDevice) -> Result<i64> {
        let now = Utc::now().timestamp();
        let favorite = if device.favorite { 1 } else { 0 };

        let result = sqlx::query(
            r#"INSERT INTO managed_devices
               (profile_id, name, protocol, address, source, favorite, last_seen, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(device.profile_id)
        .bind(&device.name)
        .bind(&device.protocol)
        .bind(&device.address)
        .bind(&device.source)
        .bind(favorite)
        .bind(device.last_seen)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get a managed device by ID
    pub async fn get_by_id(&self, id: i64) -> Result<Option<aaeq_core::ManagedDevice>> {
        let row = sqlx::query(
            r#"SELECT id, profile_id, name, protocol, address, source, favorite, last_seen, created_at, updated_at
               FROM managed_devices WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| aaeq_core::ManagedDevice {
            id: Some(r.get(0)),
            profile_id: r.get(1),
            name: r.get(2),
            protocol: r.get(3),
            address: r.get(4),
            source: r.get(5),
            favorite: r.get::<i32, _>(6) != 0,
            last_seen: r.get(7),
            created_at: r.get(8),
            updated_at: r.get(9),
        }))
    }

    /// List all managed devices for a specific profile
    pub async fn list_by_profile(&self, profile_id: i64) -> Result<Vec<aaeq_core::ManagedDevice>> {
        let rows = sqlx::query(
            r#"SELECT id, profile_id, name, protocol, address, source, favorite, last_seen, created_at, updated_at
               FROM managed_devices WHERE profile_id = ?
               ORDER BY favorite DESC, name"#
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| aaeq_core::ManagedDevice {
            id: Some(r.get(0)),
            profile_id: r.get(1),
            name: r.get(2),
            protocol: r.get(3),
            address: r.get(4),
            source: r.get(5),
            favorite: r.get::<i32, _>(6) != 0,
            last_seen: r.get(7),
            created_at: r.get(8),
            updated_at: r.get(9),
        }).collect())
    }

    /// List all managed devices for a specific profile and protocol
    pub async fn list_by_profile_and_protocol(&self, profile_id: i64, protocol: &str) -> Result<Vec<aaeq_core::ManagedDevice>> {
        let rows = sqlx::query(
            r#"SELECT id, profile_id, name, protocol, address, source, favorite, last_seen, created_at, updated_at
               FROM managed_devices WHERE profile_id = ? AND protocol = ?
               ORDER BY favorite DESC, name"#
        )
        .bind(profile_id)
        .bind(protocol)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| aaeq_core::ManagedDevice {
            id: Some(r.get(0)),
            profile_id: r.get(1),
            name: r.get(2),
            protocol: r.get(3),
            address: r.get(4),
            source: r.get(5),
            favorite: r.get::<i32, _>(6) != 0,
            last_seen: r.get(7),
            created_at: r.get(8),
            updated_at: r.get(9),
        }).collect())
    }

    /// Update managed device (upsert by profile_id + protocol + address)
    pub async fn upsert(&self, device: &aaeq_core::ManagedDevice) -> Result<i64> {
        let now = Utc::now().timestamp();
        let favorite = if device.favorite { 1 } else { 0 };

        let result = sqlx::query(
            r#"INSERT INTO managed_devices
               (profile_id, name, protocol, address, source, favorite, last_seen, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(profile_id, protocol, address) DO UPDATE SET
                   name = excluded.name,
                   source = excluded.source,
                   favorite = excluded.favorite,
                   last_seen = excluded.last_seen,
                   updated_at = ?
               RETURNING id"#
        )
        .bind(device.profile_id)
        .bind(&device.name)
        .bind(&device.protocol)
        .bind(&device.address)
        .bind(&device.source)
        .bind(favorite)
        .bind(device.last_seen)
        .bind(now)
        .bind(now)
        .bind(now) // For UPDATE SET updated_at
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get(0))
    }

    /// Update last_seen timestamp for a device
    pub async fn update_last_seen(&self, id: i64, last_seen: i64) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "UPDATE managed_devices SET last_seen = ?, updated_at = ? WHERE id = ?"
        )
        .bind(last_seen)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Toggle favorite status for a device
    pub async fn toggle_favorite(&self, id: i64) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "UPDATE managed_devices SET favorite = NOT favorite, updated_at = ? WHERE id = ?"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a managed device
    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM managed_devices WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Delete all devices for a specific profile (called on profile deletion via CASCADE)
    pub async fn delete_by_profile(&self, profile_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM managed_devices WHERE profile_id = ?")
            .bind(profile_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
