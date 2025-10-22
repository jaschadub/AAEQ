use crate::models::{Mapping, Scope, TrackMeta};
use std::collections::HashMap;

/// Index of mapping rules for fast lookup
#[derive(Clone, Debug, Default)]
pub struct RulesIndex {
    pub song_rules: HashMap<String, String>,
    pub album_rules: HashMap<String, String>,
    pub genre_rules: HashMap<String, String>,
    pub default_preset: Option<String>,
}

impl RulesIndex {
    /// Build an index from a list of mappings
    pub fn from_mappings(mappings: Vec<Mapping>) -> Self {
        let mut index = RulesIndex::default();

        for mapping in mappings {
            match mapping.scope {
                Scope::Song => {
                    if let Some(key) = mapping.key_normalized {
                        index.song_rules.insert(key, mapping.preset_name);
                    }
                }
                Scope::Album => {
                    if let Some(key) = mapping.key_normalized {
                        index.album_rules.insert(key, mapping.preset_name);
                    }
                }
                Scope::Genre => {
                    if let Some(key) = mapping.key_normalized {
                        index.genre_rules.insert(key, mapping.preset_name);
                    }
                }
                Scope::Default => {
                    index.default_preset = Some(mapping.preset_name);
                }
            }
        }

        index
    }
}

/// Resolve the appropriate preset for a given track using the hierarchy:
/// Song > Album > Genre > Default
pub fn resolve_preset(meta: &TrackMeta, rules: &RulesIndex, fallback: &str) -> String {
    // 1. Check song-specific rule
    let song_key = meta.song_key();
    if let Some(preset) = rules.song_rules.get(&song_key) {
        tracing::debug!("Matched song rule: {} -> {}", song_key, preset);
        return preset.clone();
    }

    // 2. Check album-specific rule
    let album_key = meta.album_key();
    if let Some(preset) = rules.album_rules.get(&album_key) {
        tracing::debug!("Matched album rule: {} -> {}", album_key, preset);
        return preset.clone();
    }

    // 3. Check genre-specific rule
    let genre_key = meta.genre_key();
    if let Some(preset) = rules.genre_rules.get(&genre_key) {
        tracing::debug!("Matched genre rule: {} -> {}", genre_key, preset);
        return preset.clone();
    }

    // 4. Use default from rules or fallback
    let default = rules.default_preset.as_deref().unwrap_or(fallback);
    tracing::debug!("Using default preset: {}", default);
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_preset_song_priority() {
        let track = TrackMeta {
            artist: "Pink Floyd".to_string(),
            title: "Time".to_string(),
            album: "The Dark Side of the Moon".to_string(),
            genre: "Rock".to_string(),
            device_genre: "Rock".to_string(),
            album_art_url: None,
        };

        let mut rules = RulesIndex::default();
        rules.song_rules.insert("pink floyd - time".to_string(), "Bass Boost".to_string());
        rules.album_rules.insert("pink floyd - the dark side of the moon".to_string(), "Neutral".to_string());
        rules.genre_rules.insert("rock".to_string(), "Rock".to_string());
        rules.default_preset = Some("Flat".to_string());

        let result = resolve_preset(&track, &rules, "Fallback");
        assert_eq!(result, "Bass Boost");
    }

    #[test]
    fn test_resolve_preset_album_priority() {
        let track = TrackMeta {
            artist: "Pink Floyd".to_string(),
            title: "Money".to_string(),
            album: "The Dark Side of the Moon".to_string(),
            genre: "Rock".to_string(),
            device_genre: "Rock".to_string(),
            album_art_url: None,
        };

        let mut rules = RulesIndex::default();
        rules.album_rules.insert("pink floyd - the dark side of the moon".to_string(), "Neutral".to_string());
        rules.genre_rules.insert("rock".to_string(), "Rock".to_string());

        let result = resolve_preset(&track, &rules, "Fallback");
        assert_eq!(result, "Neutral");
    }

    #[test]
    fn test_resolve_preset_genre_priority() {
        let track = TrackMeta {
            artist: "Led Zeppelin".to_string(),
            title: "Stairway to Heaven".to_string(),
            album: "Led Zeppelin IV".to_string(),
            genre: "Rock".to_string(),
            device_genre: "Rock".to_string(),
            album_art_url: None,
        };

        let mut rules = RulesIndex::default();
        rules.genre_rules.insert("rock".to_string(), "Rock".to_string());
        rules.default_preset = Some("Flat".to_string());

        let result = resolve_preset(&track, &rules, "Fallback");
        assert_eq!(result, "Rock");
    }

    #[test]
    fn test_resolve_preset_default() {
        let track = TrackMeta {
            artist: "Unknown Artist".to_string(),
            title: "Unknown Song".to_string(),
            album: "Unknown Album".to_string(),
            genre: "Unknown".to_string(),
            device_genre: "Unknown".to_string(),
            album_art_url: None,
        };

        let rules = RulesIndex {
            default_preset: Some("Flat".to_string()),
            ..Default::default()
        };

        let result = resolve_preset(&track, &rules, "Fallback");
        assert_eq!(result, "Flat");
    }

    #[test]
    fn test_resolve_preset_fallback() {
        let track = TrackMeta::default();
        let rules = RulesIndex::default();

        let result = resolve_preset(&track, &rules, "Fallback");
        assert_eq!(result, "Fallback");
    }
}
