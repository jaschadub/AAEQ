use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Track metadata extracted from the device
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackMeta {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub genre: String,
    /// The original genre from the device (before any override)
    #[serde(default)]
    pub device_genre: String,
    /// URL to album artwork (optional)
    #[serde(default)]
    pub album_art_url: Option<String>,
}

impl TrackMeta {
    /// Create a normalized key for song matching (artist - title)
    pub fn song_key(&self) -> String {
        normalize_key(&format!("{} - {}", self.artist, self.title))
    }

    /// Create a normalized key for album matching (artist - album)
    pub fn album_key(&self) -> String {
        normalize_key(&format!("{} - {}", self.artist, self.album))
    }

    /// Create a normalized key for genre matching
    pub fn genre_key(&self) -> String {
        normalize_key(&self.genre)
    }

    /// Create a composite key for debounce tracking
    pub fn track_key(&self) -> String {
        format!("{}|{}|{}|{}", self.artist, self.title, self.album, self.genre)
    }
}

/// Normalization for mapping keys (lowercase, trim whitespace)
pub fn normalize_key(input: &str) -> String {
    input.trim().to_lowercase()
}

/// Scope of a mapping rule (precedence: Song > Album > Genre > Default)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Song,
    Album,
    Genre,
    Default,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Song => "song",
            Scope::Album => "album",
            Scope::Genre => "genre",
            Scope::Default => "default",
        }
    }
}

/// Error type for invalid scope strings
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseScopeError;

impl std::fmt::Display for ParseScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid scope value")
    }
}

impl std::error::Error for ParseScopeError {}

impl FromStr for Scope {
    type Err = ParseScopeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "song" => Ok(Scope::Song),
            "album" => Ok(Scope::Album),
            "genre" => Ok(Scope::Genre),
            "default" => Ok(Scope::Default),
            _ => Err(ParseScopeError),
        }
    }
}

/// A mapping rule that associates a key with a preset (scoped by profile)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mapping {
    pub id: Option<i64>,
    pub scope: Scope,
    pub key_normalized: Option<String>, // None for Default scope
    pub preset_name: String,
    pub profile_id: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A listening profile (e.g., "Default", "Headphones", "Car")
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: Option<i64>,
    pub name: String,
    pub is_builtin: bool,
    /// Icon emoji for visual identification (e.g., "🎧", "🚗", "🏠")
    #[serde(default = "default_profile_icon")]
    pub icon: String,
    /// Color hex code for visual identification (e.g., "#4A90E2")
    #[serde(default = "default_profile_color")]
    pub color: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn default_profile_icon() -> String {
    "📁".to_string()
}

fn default_profile_color() -> String {
    "#808080".to_string() // Gray
}

/// EQ band configuration for creating/editing presets
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EqBand {
    pub frequency: u32,  // Hz
    pub gain: f32,       // dB, typically -12.0 to +12.0
}

/// A complete EQ preset with all bands
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EqPreset {
    pub name: String,
    pub bands: Vec<EqBand>,
}

impl Default for EqPreset {
    fn default() -> Self {
        // Standard 10-band EQ frequencies
        Self {
            name: "Flat".to_string(),
            bands: vec![
                EqBand { frequency: 31, gain: 0.0 },
                EqBand { frequency: 62, gain: 0.0 },
                EqBand { frequency: 125, gain: 0.0 },
                EqBand { frequency: 250, gain: 0.0 },
                EqBand { frequency: 500, gain: 0.0 },
                EqBand { frequency: 1000, gain: 0.0 },
                EqBand { frequency: 2000, gain: 0.0 },
                EqBand { frequency: 4000, gain: 0.0 },
                EqBand { frequency: 8000, gain: 0.0 },
                EqBand { frequency: 16000, gain: 0.0 },
            ],
        }
    }
}

/// Device information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: Option<i64>,
    pub kind: String,    // "wiim"
    pub label: String,   // User-friendly name
    pub host: String,    // IP or hostname
    pub discovered_at: i64,
}

/// DSP settings specific to each sink type (output device type)
/// This allows different audio configurations for different output types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DspSinkSettings {
    pub id: Option<i64>,
    pub sink_type: String,     // "LocalDac", "Dlna", "AirPlay"
    pub sample_rate: u32,      // Hz (44100, 48000, 96000, 192000)
    pub format: String,        // "S16LE", "S24LE", "F32"
    pub buffer_ms: u32,        // Buffer size in milliseconds
    pub headroom_db: f32,      // Pre-EQ gain reduction (typically -3.0 to -6.0)
    pub created_at: i64,
    pub updated_at: i64,
}

impl DspSinkSettings {
    /// Create default settings for LocalDac
    pub fn default_local_dac() -> Self {
        Self {
            id: None,
            sink_type: "LocalDac".to_string(),
            sample_rate: 48000,
            format: "F32".to_string(),
            buffer_ms: 150,
            headroom_db: -3.0,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// Create default settings for DLNA
    pub fn default_dlna() -> Self {
        Self {
            id: None,
            sink_type: "Dlna".to_string(),
            sample_rate: 48000,
            format: "S16LE".to_string(),
            buffer_ms: 200,
            headroom_db: -3.0,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// Create default settings for AirPlay
    pub fn default_airplay() -> Self {
        Self {
            id: None,
            sink_type: "AirPlay".to_string(),
            sample_rate: 44100,
            format: "S16LE".to_string(),
            buffer_ms: 300,
            headroom_db: -3.0,
            created_at: 0,
            updated_at: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_key() {
        assert_eq!(normalize_key("  Pink Floyd  "), "pink floyd");
        assert_eq!(normalize_key("The Beatles"), "the beatles");
    }

    #[test]
    fn test_track_keys() {
        let track = TrackMeta {
            artist: "Pink Floyd".to_string(),
            title: "Time".to_string(),
            album: "The Dark Side of the Moon".to_string(),
            genre: "Progressive Rock".to_string(),
            device_genre: "Progressive Rock".to_string(),
            album_art_url: None,
        };

        assert_eq!(track.song_key(), "pink floyd - time");
        assert_eq!(track.album_key(), "pink floyd - the dark side of the moon");
        assert_eq!(track.genre_key(), "progressive rock");
    }
}
