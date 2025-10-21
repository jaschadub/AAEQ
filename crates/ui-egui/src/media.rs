//! Media session integration
//!
//! This module provides a bridge between the cross-platform media-session crate
//! and AAEQ's TrackMeta format.

use aaeq_core::TrackMeta;
use aaeq_media_session::{MediaSession, create_media_session};
use anyhow::Result;
use std::sync::OnceLock;

/// Global media session instance
static MEDIA_SESSION: OnceLock<Box<dyn MediaSession>> = OnceLock::new();

/// Get the global media session instance
fn get_media_session() -> &'static Box<dyn MediaSession> {
    MEDIA_SESSION.get_or_init(|| create_media_session())
}

/// Get currently playing track metadata from the system's media player
///
/// This function queries the platform-specific media session API:
/// - Linux: MPRIS via D-Bus
/// - Windows: System Media Transport Controls (SMTC)
/// - macOS: AppleScript (Music.app and Spotify)
pub fn get_now_playing() -> Result<TrackMeta> {
    let session = get_media_session();

    match session.get_current_track()? {
        Some(metadata) => {
            // Convert MediaMetadata to TrackMeta
            let genre = metadata.genre.unwrap_or_else(|| "Unknown".to_string());

            Ok(TrackMeta {
                artist: metadata.artist,
                title: metadata.title,
                album: metadata.album,
                genre: genre.clone(),
                device_genre: genre, // Same for media session (no device override)
                album_art_url: metadata.album_art_url,
            })
        }
        None => {
            anyhow::bail!("No track currently playing")
        }
    }
}

/// Check if any media player is currently playing
pub fn is_playing() -> bool {
    let session = get_media_session();
    session.is_playing()
}

/// Get a list of active media players
pub fn list_active_players() -> Vec<String> {
    let session = get_media_session();
    session.list_active_players()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_now_playing() {
        match get_now_playing() {
            Ok(track) => {
                println!("Currently playing:");
                println!("  Artist: {}", track.artist);
                println!("  Title: {}", track.title);
                println!("  Album: {}", track.album);
                println!("  Genre: {}", track.genre);
            }
            Err(e) => {
                println!("No track playing (OK): {}", e);
            }
        }
    }

    #[test]
    fn test_list_active_players() {
        let players = list_active_players();
        println!("Active players: {:?}", players);
    }
}
