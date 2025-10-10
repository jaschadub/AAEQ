use aaeq_core::{DeviceController, EqPreset, TrackMeta};
use crate::models::*;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

/// WiiM device controller using LinkPlay HTTP API
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
            .build()
            .unwrap();

        Self {
            label: label.into(),
            host: host.into(),
            client,
        }
    }

    /// Execute a WiiM HTTP API command
    async fn execute_command(&self, command: &str) -> Result<String> {
        let url = format!("http://{}/httpapi.asp?command={}", self.host, command);

        tracing::debug!("WiiM API call: {}", url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to WiiM device")?;

        if !response.status().is_success() {
            return Err(anyhow!("WiiM API returned status: {}", response.status()));
        }

        let text = response.text().await?;
        tracing::debug!("WiiM API response: {}", text);

        Ok(text)
    }

    /// Parse player status from JSON or text response
    fn parse_player_status(&self, response: &str) -> Result<PlayerStatus> {
        // Try JSON first
        if let Ok(status) = serde_json::from_str::<PlayerStatus>(response) {
            return Ok(status);
        }

        // Fallback: parse text format if needed
        // WiiM sometimes returns plain text like "artist:Pink Floyd\ntitle:Time\n..."
        let mut status = PlayerStatus {
            status: String::new(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
        };

        for line in response.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim();
                match key.trim().to_lowercase().as_str() {
                    "title" => status.title = value.to_string(),
                    "artist" => status.artist = value.to_string(),
                    "album" => status.album = value.to_string(),
                    "genre" => status.genre = value.to_string(),
                    _ => {}
                }
            }
        }

        Ok(status)
    }
}

#[async_trait]
impl DeviceController for WiimController {
    fn id(&self) -> &str {
        &self.label
    }

    async fn get_now_playing(&self) -> Result<TrackMeta> {
        let response = self.execute_command("getPlayerStatus").await?;
        let status = self.parse_player_status(&response)?;

        Ok(TrackMeta {
            artist: status.artist,
            title: status.title,
            album: status.album,
            genre: status.genre,
        })
    }

    async fn list_presets(&self) -> Result<Vec<String>> {
        let response = self.execute_command("EQGetList").await?;

        // Try parsing as JSON
        if let Ok(list_response) = serde_json::from_str::<EqListResponse>(&response) {
            return Ok(list_response.list);
        }

        // Fallback: parse as comma-separated or newline-separated list
        let presets = response
            .split(&[',', '\n'][..])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(presets)
    }

    async fn apply_preset(&self, preset_name: &str) -> Result<()> {
        let command = format!("EQLoad:{}", preset_name);
        self.execute_command(&command).await?;

        tracing::info!("Applied preset '{}' to device '{}'", preset_name, self.label);
        Ok(())
    }

    async fn get_current_eq(&self) -> Result<Option<EqPreset>> {
        // Try to get current EQ settings
        // Note: This might not be supported by all WiiM firmware versions
        let response = self.execute_command("EQGet").await;

        if response.is_err() {
            return Ok(None);
        }

        // Parse EQ settings if available
        // Format depends on WiiM API - will need actual API docs to implement properly
        Ok(None)
    }

    async fn set_custom_eq(&self, preset: &EqPreset) -> Result<()> {
        // Set custom EQ bands
        // WiiM API format: EQSet:band0:gain0:band1:gain1:...
        // Example: EQSet:31:2.5:62:1.0:125:0.0...

        let mut command = String::from("EQSet");
        for band in &preset.bands {
            command.push_str(&format!(":{}:{}", band.frequency, band.gain));
        }

        self.execute_command(&command).await?;
        tracing::info!("Set custom EQ on device '{}'", self.label);

        Ok(())
    }

    async fn is_online(&self) -> bool {
        self.execute_command("getPlayerStatus").await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_player_status_json() {
        let wiim = WiimController::new("Test", "192.168.1.100");

        let json = r#"{"title":"Time","artist":"Pink Floyd","album":"The Dark Side of the Moon","genre":"Rock"}"#;
        let status = wiim.parse_player_status(json).unwrap();

        assert_eq!(status.title, "Time");
        assert_eq!(status.artist, "Pink Floyd");
        assert_eq!(status.album, "The Dark Side of the Moon");
        assert_eq!(status.genre, "Rock");
    }

    #[test]
    fn test_parse_player_status_text() {
        let wiim = WiimController::new("Test", "192.168.1.100");

        let text = "title: Time\nartist: Pink Floyd\nalbum: The Dark Side of the Moon\ngenre: Rock\n";
        let status = wiim.parse_player_status(text).unwrap();

        assert_eq!(status.title, "Time");
        assert_eq!(status.artist, "Pink Floyd");
        assert_eq!(status.album, "The Dark Side of the Moon");
        assert_eq!(status.genre, "Rock");
    }
}
