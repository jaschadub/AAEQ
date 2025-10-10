use aaeq_core::{DeviceController, EqPreset, TrackMeta};
use crate::models::*;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

/// WiiM device controller using LinkPlay HTTP API
///
/// Based on "HTTP API for WiiM Mini" documentation
/// API format: https://{host}/httpapi.asp?command={command}
pub struct WiimController {
    label: String,
    host: String,
    client: Client,
}

impl WiimController {
    /// Create a new WiiM controller
    pub fn new(label: impl Into<String>, host: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .danger_accept_invalid_certs(true)  // WiiM may use self-signed cert
            .build()
            .unwrap();

        Self {
            label: label.into(),
            host: host.into(),
            client,
        }
    }

    /// Execute a WiiM HTTP API command
    ///
    /// Format: https://{host}/httpapi.asp?command={command}
    async fn execute_command(&self, command: &str) -> Result<String> {
        // Try HTTPS first, fall back to HTTP if needed
        let urls = [
            format!("https://{}/httpapi.asp?command={}", self.host, command),
            format!("http://{}/httpapi.asp?command={}", self.host, command),
        ];

        let mut last_error = None;

        for url in &urls {
            tracing::debug!("WiiM API call: {}", url);

            match self.client.get(url).send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        tracing::warn!("WiiM API returned status: {}", response.status());
                        continue;
                    }

                    let text = response.text().await?;
                    tracing::debug!("WiiM API response: {}", text);

                    return Ok(text);
                }
                Err(e) => {
                    tracing::debug!("Failed to connect to {}: {}", url, e);
                    last_error = Some(e);
                }
            }
        }

        Err(anyhow!(
            "Failed to connect to WiiM device at {}: {}",
            self.host,
            last_error.unwrap()
        ))
    }

    /// Get additional metadata that might not be in getPlayerStatus
    /// This is a helper to extract metadata from various sources
    async fn get_metadata_supplemental(&self) -> Result<(String, String, String)> {
        // Try to get more detailed info from other endpoints if needed
        // For now, return empty strings - metadata should come from getPlayerStatus
        Ok((String::new(), String::new(), String::new()))
    }
}

#[async_trait]
impl DeviceController for WiimController {
    fn id(&self) -> &str {
        &self.label
    }

    /// Get current playing track metadata
    ///
    /// Command: getPlayerStatus
    /// Response: JSON with status, title, artist, album, etc.
    async fn get_now_playing(&self) -> Result<TrackMeta> {
        let response = self.execute_command("getPlayerStatus").await?;

        let status: PlayerStatus = serde_json::from_str(&response)
            .context("Failed to parse getPlayerStatus response")?;

        // Note: WiiM getPlayerStatus may not always include metadata fields
        // like title, artist, album, genre. These might be empty strings.
        // The metadata availability depends on the playback source (mode).

        let mut meta = TrackMeta {
            artist: status.artist.clone(),
            title: status.title.clone(),
            album: status.album.clone(),
            genre: String::new(),  // WiiM API doesn't provide genre directly
        };

        // If metadata is missing, try to extract from vendor field or use placeholder
        if meta.title.is_empty() && meta.artist.is_empty() {
            // Check if we're playing something
            if status.status == "play" || status.status == "pause" {
                // Use mode as fallback indication
                meta.title = format!("Track {} of {}", status.plicurr, status.plicount);
                meta.artist = format!("Mode {}", status.mode);
            } else {
                meta.title = "No track".to_string();
                meta.artist = "Not playing".to_string();
            }
        }

        Ok(meta)
    }

    /// List all available EQ preset names on the device
    ///
    /// Command: EQGetList
    /// Response: JSON array of preset names
    async fn list_presets(&self) -> Result<Vec<String>> {
        let response = self.execute_command("EQGetList").await?;

        // Response is a JSON array of preset names
        let presets: Vec<String> = serde_json::from_str(&response)
            .context("Failed to parse EQGetList response")?;

        tracing::info!("Found {} EQ presets on device", presets.len());
        Ok(presets)
    }

    /// Apply a preset by name
    ///
    /// Command: EQLoad:{preset_name}
    /// Response: {"status":"OK"} or {"status":"Failed"}
    async fn apply_preset(&self, preset_name: &str) -> Result<()> {
        let command = format!("EQLoad:{}", preset_name);
        let response = self.execute_command(&command).await?;

        // Parse the status response
        let status: StatusResponse = serde_json::from_str(&response)
            .context("Failed to parse EQLoad response")?;

        if status.status != "OK" {
            return Err(anyhow!("Failed to load EQ preset '{}': status={}", preset_name, status.status));
        }

        tracing::info!("Applied preset '{}' to device '{}'", preset_name, self.label);
        Ok(())
    }

    /// Get the current EQ settings (if supported)
    ///
    /// Note: WiiM API provides EQGetStat to check if EQ is on/off,
    /// but doesn't provide a way to read custom band values.
    async fn get_current_eq(&self) -> Result<Option<EqPreset>> {
        // Check if EQ is enabled
        let response = self.execute_command("EQGetStat").await?;

        let stat: EqStatResponse = serde_json::from_str(&response)
            .context("Failed to parse EQGetStat response")?;

        if stat.eq_stat == "Off" {
            return Ok(None);
        }

        // WiiM API doesn't provide a way to read the actual band values
        // We only know which preset is active from getPlayerStatus
        // Return None since we can't get the actual band configuration
        Ok(None)
    }

    /// Set custom EQ bands
    ///
    /// Note: The WiiM API documentation doesn't specify a command for
    /// setting custom EQ band values directly. The only EQ-related commands are:
    /// - EQOn/EQOff: Turn EQ on/off
    /// - EQGetList: Get available presets
    /// - EQLoad:{name}: Load a preset
    ///
    /// Custom EQ setting may require undocumented commands or may not be supported
    /// via the HTTP API at all.
    async fn set_custom_eq(&self, preset: &EqPreset) -> Result<()> {
        // This functionality may not be supported by the WiiM HTTP API
        // The API only allows loading predefined presets, not setting custom band values

        tracing::warn!(
            "Custom EQ setting is not documented in WiiM API. \
             Only preset loading is supported."
        );

        Err(anyhow!(
            "Setting custom EQ bands is not supported by WiiM HTTP API. \
             Use EQLoad with a preset name instead."
        ))
    }

    /// Check if device is reachable
    async fn is_online(&self) -> bool {
        match self.execute_command("getPlayerStatus").await {
            Ok(_) => {
                tracing::debug!("Device '{}' at {} is online", self.label, self.host);
                true
            }
            Err(e) => {
                tracing::debug!("Device '{}' at {} is offline: {}", self.label, self.host, e);
                false
            }
        }
    }
}

/// Helper functions for WiiM-specific operations
impl WiimController {
    /// Get detailed device information
    ///
    /// Command: getStatusEx
    /// Response: JSON with device info (name, firmware, uuid, etc.)
    pub async fn get_device_info(&self) -> Result<StatusEx> {
        let response = self.execute_command("getStatusEx").await?;
        let info: StatusEx = serde_json::from_str(&response)
            .context("Failed to parse getStatusEx response")?;
        Ok(info)
    }

    /// Turn EQ on
    ///
    /// Command: EQOn
    /// Response: {"status":"OK"} or {"status":"Failed"}
    pub async fn eq_on(&self) -> Result<()> {
        let response = self.execute_command("EQOn").await?;
        let status: StatusResponse = serde_json::from_str(&response)?;

        if status.status != "OK" {
            return Err(anyhow!("Failed to turn EQ on"));
        }

        Ok(())
    }

    /// Turn EQ off
    ///
    /// Command: EQOff
    /// Response: {"status":"OK"} or {"status":"Failed"}
    pub async fn eq_off(&self) -> Result<()> {
        let response = self.execute_command("EQOff").await?;
        let status: StatusResponse = serde_json::from_str(&response)?;

        if status.status != "OK" {
            return Err(anyhow!("Failed to turn EQ off"));
        }

        Ok(())
    }

    /// Check if EQ is enabled
    ///
    /// Command: EQGetStat
    /// Response: {"EQStat":"On"} or {"EQStat":"Off"}
    pub async fn is_eq_enabled(&self) -> Result<bool> {
        let response = self.execute_command("EQGetStat").await?;
        let stat: EqStatResponse = serde_json::from_str(&response)?;
        Ok(stat.eq_stat == "On")
    }

    /// Set volume (0-100)
    ///
    /// Command: setPlayerCmd:vol:{value}
    pub async fn set_volume(&self, volume: u8) -> Result<()> {
        let volume = volume.min(100);
        let command = format!("setPlayerCmd:vol:{}", volume);
        self.execute_command(&command).await?;
        Ok(())
    }

    /// Mute/unmute
    ///
    /// Command: setPlayerCmd:mute:{n}
    /// n=1 for mute, n=0 for unmute
    pub async fn set_mute(&self, muted: bool) -> Result<()> {
        let n = if muted { 1 } else { 0 };
        let command = format!("setPlayerCmd:mute:{}", n);
        self.execute_command(&command).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_player_status() {
        let json = r#"{
            "type":"0",
            "ch":"2",
            "mode":"10",
            "loop":"4",
            "eq":"0",
            "status":"play",
            "curpos":"184919",
            "offset_pts":"184919",
            "totlen":"0",
            "vol":"39",
            "mute":"0",
            "title":"Time",
            "artist":"Pink Floyd",
            "album":"The Dark Side of the Moon"
        }"#;

        let status: PlayerStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status, "play");
        assert_eq!(status.title, "Time");
        assert_eq!(status.artist, "Pink Floyd");
        assert_eq!(status.album, "The Dark Side of the Moon");
    }

    #[test]
    fn test_parse_eq_list() {
        let json = r#"["Flat", "Acoustic", "Bass Booster", "Rock"]"#;
        let presets: Vec<String> = serde_json::from_str(json).unwrap();
        assert_eq!(presets.len(), 4);
        assert_eq!(presets[0], "Flat");
    }

    #[test]
    fn test_parse_status_response() {
        let json = r#"{"status":"OK"}"#;
        let response: StatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, "OK");
    }

    #[test]
    fn test_parse_eq_stat() {
        let json = r#"{"EQStat":"On"}"#;
        let stat: EqStatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(stat.eq_stat, "On");
    }
}
