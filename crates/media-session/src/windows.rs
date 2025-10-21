//! Windows media session detection via System Media Transport Controls (SMTC)
//!
//! This module uses the Windows Runtime API to query currently playing media
//! from any application that implements SMTC (Spotify, iTunes, browsers, etc.)
//!
//! Requires Windows 10 version 1803 or later.

use crate::{MediaMetadata, MediaSession};
use anyhow::{Result, anyhow};
use tracing::{debug, warn};
use windows::{
    Foundation::IAsyncOperation,
    Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    },
};

pub struct SmtcSession {
    manager: Option<GlobalSystemMediaTransportControlsSessionManager>,
}

impl SmtcSession {
    pub fn new() -> Self {
        let manager = Self::init_manager();
        if manager.is_none() {
            warn!("Failed to initialize SMTC session manager");
        }
        Self { manager }
    }

    /// Initialize the SMTC session manager
    fn init_manager() -> Option<GlobalSystemMediaTransportControlsSessionManager> {
        match Self::request_manager_blocking() {
            Ok(mgr) => Some(mgr),
            Err(e) => {
                warn!("Failed to request SMTC manager: {}", e);
                None
            }
        }
    }

    /// Request the session manager (blocking call)
    fn request_manager_blocking() -> Result<GlobalSystemMediaTransportControlsSessionManager> {
        // Windows API is async, but we need to provide a sync interface
        // We'll use a simple blocking approach here
        let operation = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(|e| anyhow!("Failed to create RequestAsync operation: {}", e))?;

        // Block until the operation completes
        Self::block_on_async_operation(operation)
    }

    /// Block on an async Windows operation
    fn block_on_async_operation<T>(
        operation: IAsyncOperation<T>
    ) -> Result<T>
    where
        T: windows::core::RuntimeType + Clone,
    {
        use std::time::{Duration, Instant};

        let timeout = Duration::from_secs(2);
        let start = Instant::now();

        loop {
            match operation.Status() {
                Ok(status) => {
                    use windows::Foundation::AsyncStatus;
                    match status {
                        AsyncStatus::Completed => {
                            return operation.GetResults()
                                .map_err(|e| anyhow!("Failed to get operation results: {}", e));
                        }
                        AsyncStatus::Error => {
                            return Err(anyhow!("Async operation failed with error"));
                        }
                        AsyncStatus::Canceled => {
                            return Err(anyhow!("Async operation was canceled"));
                        }
                        AsyncStatus::Started => {
                            // Still running, continue waiting
                            if start.elapsed() > timeout {
                                return Err(anyhow!("Async operation timed out"));
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        _ => {
                            return Err(anyhow!("Unknown async status"));
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow!("Failed to get operation status: {}", e));
                }
            }
        }
    }

    /// Get the current active session
    fn get_current_session(&self) -> Result<Option<GlobalSystemMediaTransportControlsSession>> {
        let manager = self.manager.as_ref()
            .ok_or_else(|| anyhow!("SMTC manager not initialized"))?;

        match manager.GetCurrentSession() {
            Ok(session) => Ok(Some(session)),
            Err(e) => {
                debug!("No current session: {}", e);
                Ok(None)
            }
        }
    }

    /// Extract metadata from a session
    fn extract_metadata(&self, session: &GlobalSystemMediaTransportControlsSession) -> Result<MediaMetadata> {
        // Get media properties asynchronously
        let operation = session.TryGetMediaPropertiesAsync()
            .map_err(|e| anyhow!("Failed to get media properties: {}", e))?;

        let media_props = Self::block_on_async_operation(operation)?;

        // Extract metadata fields
        let title = media_props.Title()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        let artist = media_props.Artist()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        let album = media_props.AlbumTitle()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        // SMTC doesn't provide genre information
        let genre = None;

        // Thumbnail is available but would need additional processing
        // For now, we'll leave album_art_url as None
        let album_art_url = None;

        debug!("SMTC metadata: title={}, artist={}, album={}", title, artist, album);

        Ok(MediaMetadata {
            title,
            artist,
            album,
            genre,
            album_art_url,
        })
    }

    /// Get current session if available
    /// Note: Windows SMTC API doesn't provide a way to enumerate all sessions,
    /// so we can only check the currently active one
    fn get_current_session_option(&self) -> Option<GlobalSystemMediaTransportControlsSession> {
        self.get_current_session().ok().flatten()
    }

    /// Check if a session is currently playing
    fn is_session_playing(&self, session: &GlobalSystemMediaTransportControlsSession) -> bool {
        match session.GetPlaybackInfo() {
            Ok(info) => {
                match info.PlaybackStatus() {
                    Ok(status) => status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing,
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }
}

impl MediaSession for SmtcSession {
    fn get_current_track(&self) -> Result<Option<MediaMetadata>> {
        let session = match self.get_current_session()? {
            Some(s) => s,
            None => return Ok(None),
        };

        match self.extract_metadata(&session) {
            Ok(metadata) => Ok(Some(metadata)),
            Err(e) => {
                warn!("Failed to extract metadata: {}", e);
                Ok(None)
            }
        }
    }

    fn is_playing(&self) -> bool {
        if let Ok(Some(session)) = self.get_current_session() {
            return self.is_session_playing(&session);
        }
        false
    }

    fn list_active_players(&self) -> Vec<String> {
        // Windows SMTC API only provides access to the current active session
        if let Some(session) = self.get_current_session_option() {
            if let Ok(source) = session.SourceAppUserModelId() {
                return vec![source.to_string()];
            }
        }
        Vec::new()
    }
}
