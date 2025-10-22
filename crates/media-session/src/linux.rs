//! Linux media session detection via MPRIS (Media Player Remote Interfacing Specification)
//!
//! This module queries D-Bus for MPRIS-compatible media players and extracts
//! currently playing media information.

use crate::{MediaMetadata, MediaSession};
use anyhow::{Result, anyhow};
use std::process::Command;
use tracing::{debug, warn};

pub struct MprisSession;

impl MprisSession {
    pub fn new() -> Self {
        Self
    }

    /// Get the first available MPRIS media player
    fn get_active_player(&self) -> Result<String> {
        let output = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus.ListNames",
            ])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Find MPRIS players (prefer non-browser players first)
        let mut browser_players = Vec::new();
        let mut other_players = Vec::new();

        for line in stdout.lines() {
            if line.contains("org.mpris.MediaPlayer2.") {
                let player = line
                    .trim()
                    .trim_start_matches("string \"")
                    .trim_end_matches('"')
                    .to_string();

                // Deprioritize browser players as they might be playing videos
                if player.contains("firefox") || player.contains("chrome") || player.contains("chromium") {
                    browser_players.push(player);
                } else {
                    other_players.push(player);
                }
            }
        }

        // Prefer dedicated music players over browsers
        other_players.extend(browser_players);

        // Try to find a player that's currently playing, otherwise return first available
        for player in &other_players {
            if let Ok(status) = self.get_mpris_property(player, "PlaybackStatus") {
                if status == "Playing" {
                    debug!("Found playing player: {}", player);
                    return Ok(player.clone());
                }
            }
        }

        // No player is currently playing, return the first available (might be paused)
        other_players.into_iter().next()
            .ok_or_else(|| anyhow!("No MPRIS media players found"))
    }

    /// Get property value from MPRIS player
    fn get_mpris_property(&self, player: &str, property: &str) -> Result<String> {
        let output = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                &format!("--dest={}", player),
                "/org/mpris/MediaPlayer2",
                "org.freedesktop.DBus.Properties.Get",
                "string:org.mpris.MediaPlayer2.Player",
                &format!("string:{}", property),
            ])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse the dbus-send output
        // Format is usually "variant <type> <value>"
        let lines: Vec<&str> = stdout.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let line = line.trim();
            if line.starts_with("variant") {
                // Look for string value on same or next line
                if let Some(value) = line.split('"').nth(1) {
                    return Ok(value.to_string());
                }
                // Check next line
                if i + 1 < lines.len() {
                    let next_line = lines[i + 1].trim();
                    if let Some(value) = next_line.split('"').nth(1) {
                        return Ok(value.to_string());
                    }
                }
            }
        }

        Err(anyhow!("Failed to parse property: {}", property))
    }

    /// Get metadata dictionary from MPRIS player
    fn get_mpris_metadata(&self, player: &str) -> Result<std::collections::HashMap<String, String>> {
        let output = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                &format!("--dest={}", player),
                "/org/mpris/MediaPlayer2",
                "org.freedesktop.DBus.Properties.Get",
                "string:org.mpris.MediaPlayer2.Player",
                "string:Metadata",
            ])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut metadata = std::collections::HashMap::new();
        let lines: Vec<&str> = stdout.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();

            // Look for dict entries like: dict entry(
            if line.starts_with("dict entry(") {
                // Next line should be the key
                if i + 1 < lines.len() {
                    let key_line = lines[i + 1].trim();
                    if let Some(key) = key_line.strip_prefix("string \"").and_then(|s| s.strip_suffix('"')) {
                        debug!("Parsing metadata key: {}", key);

                        // Look for the value in subsequent lines
                        let mut found_value = false;
                        for j in i + 2..std::cmp::min(i + 15, lines.len()) {
                            let val_line = lines[j].trim();

                            // Check if this is an array variant
                            if val_line.starts_with("variant") && val_line.contains("array") {
                                debug!("Found array variant for key: {}", key);
                                // Look for the first string value inside the array
                                for array_line in lines.iter().skip(j + 1).take(9) {
                                    let array_line = array_line.trim();

                                    // Stop at closing bracket or next dict entry
                                    if array_line.starts_with("]") || array_line.starts_with("dict entry(") {
                                        break;
                                    }

                                    // Extract string value from array element
                                    if array_line.contains("string") && array_line.contains('"') {
                                        if let Some(value) = array_line.split('"').nth(1) {
                                            debug!("Extracted array value for {}: {}", key, value);
                                            metadata.insert(key.to_string(), value.to_string());
                                            found_value = true;
                                            break;
                                        }
                                    }
                                }
                                if found_value {
                                    break;
                                }
                            }
                            // Check for simple variant string (non-array)
                            else if val_line.starts_with("variant") {
                                debug!("Found simple variant for key: {}", key);
                                // Look for string on same or next line
                                for string_line in lines.iter().skip(j).take(3) {
                                    let string_line = string_line.trim();
                                    if string_line.contains("string") && string_line.contains('"') {
                                        if let Some(value) = string_line.split('"').nth(1) {
                                            debug!("Extracted simple value for {}: {}", key, value);
                                            metadata.insert(key.to_string(), value.to_string());
                                            found_value = true;
                                            break;
                                        }
                                    }
                                }
                                if found_value {
                                    break;
                                }
                            }

                            // Stop at next dict entry
                            if val_line.starts_with("dict entry(") {
                                break;
                            }
                        }
                    }
                }
            }

            i += 1;
        }

        debug!("Parsed metadata map: {:?}", metadata);
        Ok(metadata)
    }

    /// Get all available MPRIS players
    fn get_all_players(&self) -> Vec<String> {
        let output = match Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus.ListNames",
            ])
            .output() {
                Ok(output) => output,
                Err(_) => return Vec::new(),
            };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut players = Vec::new();

        for line in stdout.lines() {
            if line.contains("org.mpris.MediaPlayer2.") {
                let player = line
                    .trim()
                    .trim_start_matches("string \"")
                    .trim_end_matches('"')
                    .to_string();
                players.push(player);
            }
        }

        players
    }
}

impl MediaSession for MprisSession {
    fn get_current_track(&self) -> Result<Option<MediaMetadata>> {
        // Find an active player
        let player = match self.get_active_player() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        debug!("Found MPRIS player: {}", player);

        // Get playback status first
        match self.get_mpris_property(&player, "PlaybackStatus") {
            Ok(status) if status == "Playing" => {
                debug!("Player is playing");
            }
            Ok(status) => {
                debug!("Player status: {}", status);
                // Continue anyway - user might have paused
            }
            Err(e) => {
                warn!("Failed to get playback status: {}", e);
            }
        }

        // Get metadata
        let metadata = self.get_mpris_metadata(&player)?;

        // Extract standard MPRIS fields
        let artist = metadata.get("xesam:artist")
            .or_else(|| metadata.get("xesam:albumArtist"))
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        let title = metadata.get("xesam:title")
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        let album = metadata.get("xesam:album")
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        let genre = metadata.get("xesam:genre")
            .cloned();

        let album_art_url = metadata.get("mpris:artUrl").cloned();

        debug!("MPRIS metadata: artist={}, title={}, album={}, genre={:?}, art_url={:?}",
               artist, title, album, genre, album_art_url);

        Ok(Some(MediaMetadata {
            artist,
            title,
            album,
            genre,
            album_art_url,
        }))
    }

    fn is_playing(&self) -> bool {
        if let Ok(player) = self.get_active_player() {
            if let Ok(status) = self.get_mpris_property(&player, "PlaybackStatus") {
                return status == "Playing";
            }
        }
        false
    }

    fn list_active_players(&self) -> Vec<String> {
        self.get_all_players()
    }
}
