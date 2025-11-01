use serde::{Deserialize, Serialize};

/// DSP configuration settings for a profile
///
/// Stores audio processing parameters like sample rate, buffer size,
/// headroom control, dithering, and resampling settings. Each profile can have its own DSP configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DspSettings {
    pub id: Option<i64>,
    pub profile_id: i64,
    pub sample_rate: u32,
    pub buffer_ms: u32,
    pub headroom_db: f32,
    pub auto_compensate: bool,
    pub clip_detection: bool,
    pub dither_enabled: bool,
    pub dither_mode: String, // DitherMode as string: "None", "Rectangular", "Triangular", "Gaussian"
    pub noise_shaping: String, // NoiseShaping as string: "None", "FirstOrder", "SecondOrder", "Gesemann"
    pub target_bits: u8,
    pub resample_enabled: bool,
    pub resample_quality: String, // ResamplerQuality as string: "Fast", "Balanced", "High", "Ultra"
    pub target_sample_rate: u32,
    // DSP Enhancers & Filters - Tone/Character (mutually exclusive)
    pub tube_warmth_enabled: bool,
    pub tape_saturation_enabled: bool,
    pub transformer_enabled: bool,
    pub exciter_enabled: bool,
    pub transient_enhancer_enabled: bool,
    // DSP Enhancers & Filters - Dynamic Processors (mutually exclusive)
    pub compressor_enabled: bool,
    pub limiter_enabled: bool,
    pub expander_enabled: bool,
    // DSP Enhancers & Filters - Spatial/Psychoacoustic (can be stacked)
    pub stereo_width_enabled: bool,
    pub crossfeed_enabled: bool,
    pub room_ambience_enabled: bool,
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
            dither_enabled: false,
            dither_mode: "Triangular".to_string(), // TPDF is industry standard
            noise_shaping: "None".to_string(),
            target_bits: 16,
            resample_enabled: false,
            resample_quality: "Balanced".to_string(), // Balanced is recommended
            target_sample_rate: 48000, // 48 kHz is studio standard
            // DSP Enhancers - all disabled by default
            tube_warmth_enabled: false,
            tape_saturation_enabled: false,
            transformer_enabled: false,
            exciter_enabled: false,
            transient_enhancer_enabled: false,
            compressor_enabled: false,
            limiter_enabled: false,
            expander_enabled: false,
            stereo_width_enabled: false,
            crossfeed_enabled: false,
            room_ambience_enabled: false,
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
            dither_enabled: false,
            dither_mode: "Triangular".to_string(),
            noise_shaping: "None".to_string(),
            target_bits: 16,
            resample_enabled: false,
            resample_quality: "Balanced".to_string(),
            target_sample_rate: 48000,
            // DSP Enhancers - all disabled by default
            tube_warmth_enabled: false,
            tape_saturation_enabled: false,
            transformer_enabled: false,
            exciter_enabled: false,
            transient_enhancer_enabled: false,
            compressor_enabled: false,
            limiter_enabled: false,
            expander_enabled: false,
            stereo_width_enabled: false,
            crossfeed_enabled: false,
            room_ambience_enabled: false,
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
        assert!(!settings.dither_enabled);
        assert_eq!(settings.dither_mode, "Triangular");
        assert_eq!(settings.noise_shaping, "None");
        assert_eq!(settings.target_bits, 16);
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
