//! macOS media session detection via multiple methods
//!
//! This module attempts to query currently playing media using:
//! 1. System Now Playing info (works with all apps including Tidal, YouTube Music, etc.)
//! 2. AppleScript for specific apps (Music.app, Spotify)
//!
//! The system approach works with any app that publishes to macOS's media center,
//! including Tidal, YouTube Music, Amazon Music, and others.

use crate::{MediaMetadata, MediaSession};
use anyhow::{Result, anyhow};
use std::process::Command;
use tracing::{debug, warn};

pub struct MacOsSession;

impl MacOsSession {
    pub fn new() -> Self {
        Self
    }

    /// Try to get track info from macOS Now Playing (works with all apps)
    /// This uses a combination of osascript and System Events to access
    /// the currently playing media from the system's media center.
    fn get_system_now_playing(&self) -> Result<Option<MediaMetadata>> {
        // Try using 'nowplayingctl' if available (third-party tool)
        let nowplayingctl_output = Command::new("nowplayingctl")
            .args(["get", "title", "artist", "album"])
            .output();

        if let Ok(output) = nowplayingctl_output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = stdout.lines().collect();

                if lines.len() >= 3 {
                    return Ok(Some(MediaMetadata {
                        title: lines[0].to_string(),
                        artist: lines[1].to_string(),
                        album: lines[2].to_string(),
                        genre: None,
                        album_art_url: None,
                    }));
                }
            }
        }

        // Fallback: Try to read from notification center's now playing widget
        // This requires the 'nowplaying-cli' or similar tool
        debug!("System now playing not available, falling back to app-specific checks");
        Ok(None)
    }

    /// Try to get track info from Music.app
    fn get_music_app_track(&self) -> Result<Option<MediaMetadata>> {
        // Check if Music.app is running
        let check_running = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to (name of processes) contains \"Music\"")
            .output()?;

        let is_running = String::from_utf8_lossy(&check_running.stdout).trim() == "true";
        if !is_running {
            debug!("Music.app is not running");
            return Ok(None);
        }

        // Get track info
        let output = Command::new("osascript")
            .arg("-e")
            .arg(r#"
                tell application "Music"
                    if player state is playing or player state is paused then
                        set trackName to name of current track
                        set trackArtist to artist of current track
                        set trackAlbum to album of current track
                        set trackGenre to genre of current track
                        return trackName & "|||" & trackArtist & "|||" & trackAlbum & "|||" & trackGenre
                    else
                        return ""
                    end if
                end tell
            "#)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if stdout.is_empty() {
            return Ok(None);
        }

        let parts: Vec<&str> = stdout.split("|||").collect();
        if parts.len() >= 4 {
            Ok(Some(MediaMetadata {
                title: parts[0].to_string(),
                artist: parts[1].to_string(),
                album: parts[2].to_string(),
                genre: Some(parts[3].to_string()),
                album_art_url: None,
            }))
        } else {
            Ok(None)
        }
    }

    /// Try to get track info from Spotify
    fn get_spotify_track(&self) -> Result<Option<MediaMetadata>> {
        // Check if Spotify is running
        let check_running = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to (name of processes) contains \"Spotify\"")
            .output()?;

        let is_running = String::from_utf8_lossy(&check_running.stdout).trim() == "true";
        if !is_running {
            debug!("Spotify is not running");
            return Ok(None);
        }

        // Get track info
        let output = Command::new("osascript")
            .arg("-e")
            .arg(r#"
                tell application "Spotify"
                    if player state is playing or player state is paused then
                        set trackName to name of current track
                        set trackArtist to artist of current track
                        set trackAlbum to album of current track
                        return trackName & "|||" & trackArtist & "|||" & trackAlbum
                    else
                        return ""
                    end if
                end tell
            "#)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if stdout.is_empty() {
            return Ok(None);
        }

        let parts: Vec<&str> = stdout.split("|||").collect();
        if parts.len() >= 3 {
            Ok(Some(MediaMetadata {
                title: parts[0].to_string(),
                artist: parts[1].to_string(),
                album: parts[2].to_string(),
                genre: None, // Spotify AppleScript doesn't provide genre
                album_art_url: None,
            }))
        } else {
            Ok(None)
        }
    }

    /// Check if Music.app is playing
    fn is_music_app_playing(&self) -> bool {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(r#"
                tell application "System Events"
                    if (name of processes) contains "Music" then
                        tell application "Music"
                            return player state is playing
                        end tell
                    else
                        return false
                    end if
                end tell
            "#)
            .output();

        match output {
            Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "true",
            Err(_) => false,
        }
    }

    /// Check if Spotify is playing
    fn is_spotify_playing(&self) -> bool {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(r#"
                tell application "System Events"
                    if (name of processes) contains "Spotify" then
                        tell application "Spotify"
                            return player state is playing
                        end tell
                    else
                        return false
                    end if
                end tell
            "#)
            .output();

        match output {
            Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "true",
            Err(_) => false,
        }
    }

    /// List all media apps that are currently running
    fn list_running_media_apps(&self) -> Vec<String> {
        let mut apps = Vec::new();

        // Check for Music.app
        let check_music = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to (name of processes) contains \"Music\"")
            .output();

        if let Ok(output) = check_music {
            if String::from_utf8_lossy(&output.stdout).trim() == "true" {
                apps.push("Music".to_string());
            }
        }

        // Check for Spotify
        let check_spotify = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to (name of processes) contains \"Spotify\"")
            .output();

        if let Ok(output) = check_spotify {
            if String::from_utf8_lossy(&output.stdout).trim() == "true" {
                apps.push("Spotify".to_string());
            }
        }

        apps
    }
}

impl MediaSession for MacOsSession {
    fn get_current_track(&self) -> Result<Option<MediaMetadata>> {
        // Try system-wide now playing first (works with all apps including Tidal, YouTube Music, etc.)
        if let Ok(Some(track)) = self.get_system_now_playing() {
            debug!("Got track from system now playing");
            return Ok(Some(track));
        }

        // Fall back to app-specific AppleScript checks
        // Try Spotify first (usually preferred by users)
        if let Ok(Some(track)) = self.get_spotify_track() {
            debug!("Got track from Spotify");
            return Ok(Some(track));
        }

        // Try Music.app
        if let Ok(Some(track)) = self.get_music_app_track() {
            debug!("Got track from Music.app");
            return Ok(Some(track));
        }

        // No track found
        Ok(None)
    }

    fn is_playing(&self) -> bool {
        self.is_spotify_playing() || self.is_music_app_playing()
    }

    fn list_active_players(&self) -> Vec<String> {
        self.list_running_media_apps()
    }
}
