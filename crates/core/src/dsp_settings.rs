use serde::{Deserialize, Serialize};

/// DSP configuration settings for a profile
///
/// Stores audio processing parameters like sample rate, buffer size,
/// and headroom control settings. Each profile can have its own DSP configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DspSettings {
    pub id: Option<i64>,
    pub profile_id: i64,
    pub sample_rate: u32,
    pub buffer_ms: u32,
    pub headroom_db: f32,
    pub auto_compensate: bool,
    pub clip_detection: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Default for DspSettings {
    fn default() -> Self {
        Self {
            id: None,
            profile_id: 1, // Default profile
            sample_rate: 48000,
            buffer_ms: 150,
            headroom_db: -3.0,
            auto_compensate: false,
            clip_detection: true,
            created_at: 0, // Will be set by persistence layer
            updated_at: 0, // Will be set by persistence layer
        }
    }
}

impl DspSettings {
    /// Create new DSP settings for a specific profile
    pub fn new_for_profile(profile_id: i64) -> Self {
        Self {
            profile_id,
            ..Default::default()
        }
    }

    /// Create DSP settings with custom values
    pub fn new(
        profile_id: i64,
        sample_rate: u32,
        buffer_ms: u32,
        headroom_db: f32,
    ) -> Self {
        Self {
            id: None,
            profile_id,
            sample_rate,
            buffer_ms,
            headroom_db,
            auto_compensate: false,
            clip_detection: true,
            created_at: 0,  // Will be set by persistence layer
            updated_at: 0,  // Will be set by persistence layer
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = DspSettings::default();
        assert_eq!(settings.profile_id, 1);
        assert_eq!(settings.sample_rate, 48000);
        assert_eq!(settings.buffer_ms, 150);
        assert_eq!(settings.headroom_db, -3.0);
        assert!(!settings.auto_compensate);
        assert!(settings.clip_detection);
    }

    #[test]
    fn test_new_for_profile() {
        let settings = DspSettings::new_for_profile(2);
        assert_eq!(settings.profile_id, 2);
        assert_eq!(settings.sample_rate, 48000); // Default values
    }

    #[test]
    fn test_custom_settings() {
        let settings = DspSettings::new(3, 96000, 200, -6.0);
        assert_eq!(settings.profile_id, 3);
        assert_eq!(settings.sample_rate, 96000);
        assert_eq!(settings.buffer_ms, 200);
        assert_eq!(settings.headroom_db, -6.0);
    }
}
