use crate::models::{EqPreset, TrackMeta};
use anyhow::Result;
use async_trait::async_trait;

/// Trait for device-specific controllers (WiiM, Sonos, etc.)
#[async_trait]
pub trait DeviceController: Send + Sync {
    /// Unique identifier for this device (label or serial)
    fn id(&self) -> &str;

    /// Get current playing track metadata
    async fn get_now_playing(&self) -> Result<TrackMeta>;

    /// List all available EQ preset names on the device
    async fn list_presets(&self) -> Result<Vec<String>>;

    /// Apply a preset by name
    async fn apply_preset(&self, preset_name: &str) -> Result<()>;

    /// Get the current EQ settings (if supported)
    async fn get_current_eq(&self) -> Result<Option<EqPreset>>;

    /// Set custom EQ bands (if supported)
    async fn set_custom_eq(&self, preset: &EqPreset) -> Result<()>;

    /// Check if device is reachable
    async fn is_online(&self) -> bool;
}
