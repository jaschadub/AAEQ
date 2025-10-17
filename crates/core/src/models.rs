use serde::{Deserialize, Serialize};

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

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "song" => Some(Scope::Song),
            "album" => Some(Scope::Album),
            "genre" => Some(Scope::Genre),
            "default" => Some(Scope::Default),
            _ => None,
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
    pub created_at: i64,
    pub updated_at: i64,
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
