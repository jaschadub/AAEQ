use crate::models::{EqPreset, TrackMeta};
use anyhow::{anyhow, Result};
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

    // Playback control methods (optional, with default implementations)

    /// Resume/start playback
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn play(&self) -> Result<()> {
        Err(anyhow!("Playback control not supported by this device"))
    }

    /// Pause playback
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn pause(&self) -> Result<()> {
        Err(anyhow!("Playback control not supported by this device"))
    }

    /// Stop playback
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn stop(&self) -> Result<()> {
        Err(anyhow!("Playback control not supported by this device"))
    }

    /// Next track
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn next_track(&self) -> Result<()> {
        Err(anyhow!("Playback control not supported by this device"))
    }

    /// Previous track
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn prev_track(&self) -> Result<()> {
        Err(anyhow!("Playback control not supported by this device"))
    }

    /// Check if this device supports playback controls
    /// Default is false. Override in device implementations that support playback.
    fn supports_playback_control(&self) -> bool {
        false
    }

    /// Switch playback source/input
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn switch_source(&self, _source: &str) -> Result<()> {
        Err(anyhow!("Source switching not supported by this device"))
    }

    /// Get player status (mode, playback state, volume, mute)
    /// Returns (mode, status, volume, muted) tuple where:
    /// - mode: source/input (e.g., "31" for Spotify, "41" for Bluetooth)
    /// - status: playback state ("play", "pause", "stop", "loading")
    /// - volume: volume level (0-100)
    /// - muted: mute state (true/false)
    /// Default implementation returns None. Override in device implementations that support this.
    async fn get_player_status(&self) -> Result<Option<(String, String, u8, bool)>> {
        Ok(None)
    }

    /// Set volume level
    /// Volume range is 0-100 (percentage)
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn set_volume(&self, _volume: u8) -> Result<()> {
        Err(anyhow!("Volume control not supported by this device"))
    }

    /// Set mute state
    /// Default implementation returns an error. Override in device implementations that support this.
    async fn set_mute(&self, _muted: bool) -> Result<()> {
        Err(anyhow!("Mute control not supported by this device"))
    }
}
