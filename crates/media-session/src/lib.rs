//! Cross-platform media session detection
//!
//! This crate provides a unified interface for detecting currently playing media
//! across different platforms:
//! - Linux: MPRIS via D-Bus
//! - Windows: System Media Transport Controls (SMTC)
//! - macOS: AppleScript (with future MediaPlayer framework support)

use anyhow::Result;

/// Metadata for currently playing media
#[derive(Debug, Clone)]
pub struct MediaMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_art_url: Option<String>,
    pub genre: Option<String>,
}

/// Cross-platform trait for media session detection
pub trait MediaSession: Send + Sync {
    /// Get currently playing track metadata
    fn get_current_track(&self) -> Result<Option<MediaMetadata>>;

    /// Check if any media player is currently playing
    fn is_playing(&self) -> bool;

    /// Get a list of active media players (platform-specific)
    fn list_active_players(&self) -> Vec<String>;
}

// Platform-specific modules
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

/// Create a platform-specific media session
pub fn create_media_session() -> Box<dyn MediaSession> {
    #[cfg(target_os = "linux")]
    return Box::new(linux::MprisSession::new());

    #[cfg(target_os = "windows")]
    return Box::new(windows::SmtcSession::new());

    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOsSession::new());

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    compile_error!("Unsupported platform - media session detection requires Linux, Windows, or macOS");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_media_session() {
        let session = create_media_session();
        let players = session.list_active_players();
        println!("Active players: {:?}", players);
    }

    #[test]
    fn test_get_current_track() {
        let session = create_media_session();
        match session.get_current_track() {
            Ok(Some(track)) => {
                println!("Currently playing:");
                println!("  Title: {}", track.title);
                println!("  Artist: {}", track.artist);
                println!("  Album: {}", track.album);
                println!("  Genre: {:?}", track.genre);
            }
            Ok(None) => {
                println!("No track currently playing");
            }
            Err(e) => {
                println!("Error getting track: {}", e);
            }
        }
    }
}
