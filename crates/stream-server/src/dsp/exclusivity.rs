/// DSP Enhancer mutual exclusivity rules engine
///
/// Manages which DSP effects can be enabled simultaneously and provides
/// friendly error messages when attempting to enable conflicting effects.
use aaeq_core::DspSettings;
use std::fmt;

/// Represents a DSP effect type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspEffect {
    // Tone/Character Enhancers (Group A - mutually exclusive)
    TubeWarmth,
    TapeSaturation,
    Transformer,
    Exciter,
    TransientEnhancer,
    // Dynamic Processors (Group B - mutually exclusive)
    Compressor,
    Limiter,
    Expander,
    // Spatial/Psychoacoustic (Group C - can stack)
    StereoWidth,
    Crossfeed,
    RoomAmbience,
}

impl DspEffect {
    /// Get human-readable display name
    pub fn display_name(&self) -> &'static str {
        match self {
            DspEffect::TubeWarmth => "Tube Warmth",
            DspEffect::TapeSaturation => "Tape Saturation",
            DspEffect::Transformer => "Transformer Color",
            DspEffect::Exciter => "Exciter",
            DspEffect::TransientEnhancer => "Transient Enhancer",
            DspEffect::Compressor => "Compressor",
            DspEffect::Limiter => "Limiter",
            DspEffect::Expander => "Expander/Noise Gate",
            DspEffect::StereoWidth => "Stereo Width",
            DspEffect::Crossfeed => "Crossfeed",
            DspEffect::RoomAmbience => "Room Ambience",
        }
    }

    /// Get the exclusivity group
    pub fn group(&self) -> ExclusivityGroup {
        match self {
            DspEffect::TubeWarmth
            | DspEffect::TapeSaturation
            | DspEffect::Transformer
            | DspEffect::Exciter
            | DspEffect::TransientEnhancer => ExclusivityGroup::ToneEnhancers,
            DspEffect::Compressor | DspEffect::Limiter | DspEffect::Expander => {
                ExclusivityGroup::DynamicProcessors
            }
            DspEffect::StereoWidth | DspEffect::Crossfeed | DspEffect::RoomAmbience => {
                ExclusivityGroup::SpatialEffects
            }
        }
    }

    /// Get all effects in the same exclusivity group
    pub fn conflicting_effects(&self) -> Vec<DspEffect> {
        let group = self.group();
        if group == ExclusivityGroup::SpatialEffects {
            // Spatial effects can stack, no conflicts
            return vec![];
        }

        match group {
            ExclusivityGroup::ToneEnhancers => {
                let mut effects = vec![
                    DspEffect::TubeWarmth,
                    DspEffect::TapeSaturation,
                    DspEffect::Transformer,
                    DspEffect::Exciter,
                    DspEffect::TransientEnhancer,
                ];
                effects.retain(|e| e != self);
                effects
            }
            ExclusivityGroup::DynamicProcessors => {
                let mut effects =
                    vec![DspEffect::Compressor, DspEffect::Limiter, DspEffect::Expander];
                effects.retain(|e| e != self);
                effects
            }
            ExclusivityGroup::SpatialEffects => vec![],
        }
    }
}

impl fmt::Display for DspEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Exclusivity groups for DSP effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusivityGroup {
    ToneEnhancers,      // Only one can be active
    DynamicProcessors,  // Only one can be active
    SpatialEffects,     // Can all be active simultaneously
}

impl ExclusivityGroup {
    pub fn display_name(&self) -> &'static str {
        match self {
            ExclusivityGroup::ToneEnhancers => "tone enhancer",
            ExclusivityGroup::DynamicProcessors => "dynamic processor",
            ExclusivityGroup::SpatialEffects => "spatial effect",
        }
    }
}

/// Represents a conflict when trying to enable a DSP effect
#[derive(Debug)]
pub struct ConflictError {
    pub effect: DspEffect,
    pub conflicting_with: Vec<DspEffect>,
}

impl ConflictError {
    /// Get a friendly error message for the user
    pub fn message(&self) -> String {
        let effect_name = self.effect.display_name();
        let group_name = self.effect.group().display_name();

        if self.conflicting_with.len() == 1 {
            format!(
                "Cannot enable {} - {} is already active. Only one {} can be enabled at a time.",
                effect_name,
                self.conflicting_with[0].display_name(),
                group_name
            )
        } else {
            let conflicts: Vec<&str> = self
                .conflicting_with
                .iter()
                .map(|e| e.display_name())
                .collect();
            let conflicts_str = conflicts.join(", ");
            format!(
                "Cannot enable {} - the following effects are already active: {}. Only one {} can be enabled at a time.",
                effect_name, conflicts_str, group_name
            )
        }
    }

    /// Get suggestion for resolution
    pub fn suggestion(&self) -> String {
        if self.conflicting_with.len() == 1 {
            format!(
                "Disable {} first, then enable {}.",
                self.conflicting_with[0].display_name(),
                self.effect.display_name()
            )
        } else {
            format!(
                "Disable the active {} first, then enable {}.",
                self.effect.group().display_name(),
                self.effect.display_name()
            )
        }
    }
}

impl fmt::Display for ConflictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.message(), self.suggestion())
    }
}

impl std::error::Error for ConflictError {}

/// Validate if a DSP effect can be enabled given the current settings
pub fn validate_toggle(
    effect: DspEffect,
    current_settings: &DspSettings,
) -> Result<(), ConflictError> {
    // If the effect group allows stacking, always allow
    if effect.group() == ExclusivityGroup::SpatialEffects {
        return Ok(());
    }

    // Check if any conflicting effects are currently enabled
    let mut conflicts = Vec::new();

    for conflicting_effect in effect.conflicting_effects() {
        if is_effect_enabled(conflicting_effect, current_settings) {
            conflicts.push(conflicting_effect);
        }
    }

    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(ConflictError {
            effect,
            conflicting_with: conflicts,
        })
    }
}

/// Check if a specific effect is currently enabled
pub fn is_effect_enabled(effect: DspEffect, settings: &DspSettings) -> bool {
    match effect {
        DspEffect::TubeWarmth => settings.tube_warmth_enabled,
        DspEffect::TapeSaturation => settings.tape_saturation_enabled,
        DspEffect::Transformer => settings.transformer_enabled,
        DspEffect::Exciter => settings.exciter_enabled,
        DspEffect::TransientEnhancer => settings.transient_enhancer_enabled,
        DspEffect::Compressor => settings.compressor_enabled,
        DspEffect::Limiter => settings.limiter_enabled,
        DspEffect::Expander => settings.expander_enabled,
        DspEffect::StereoWidth => settings.stereo_width_enabled,
        DspEffect::Crossfeed => settings.crossfeed_enabled,
        DspEffect::RoomAmbience => settings.room_ambience_enabled,
    }
}

/// Get all currently enabled effects
pub fn get_enabled_effects(settings: &DspSettings) -> Vec<DspEffect> {
    let all_effects = vec![
        DspEffect::TubeWarmth,
        DspEffect::TapeSaturation,
        DspEffect::Transformer,
        DspEffect::Exciter,
        DspEffect::TransientEnhancer,
        DspEffect::Compressor,
        DspEffect::Limiter,
        DspEffect::Expander,
        DspEffect::StereoWidth,
        DspEffect::Crossfeed,
        DspEffect::RoomAmbience,
    ];

    all_effects
        .into_iter()
        .filter(|effect| is_effect_enabled(*effect, settings))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_effects_no_conflicts() {
        let settings = DspSettings {
            stereo_width_enabled: true,
            crossfeed_enabled: true,
            ..Default::default()
        };

        // Should allow enabling room ambience when other spatial effects are on
        let result = validate_toggle(DspEffect::RoomAmbience, &settings);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tone_enhancers_mutually_exclusive() {
        let settings = DspSettings {
            tube_warmth_enabled: true,
            ..Default::default()
        };

        // Should not allow enabling tape saturation when tube warmth is on
        let result = validate_toggle(DspEffect::TapeSaturation, &settings);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.effect, DspEffect::TapeSaturation);
        assert_eq!(err.conflicting_with.len(), 1);
        assert_eq!(err.conflicting_with[0], DspEffect::TubeWarmth);
    }

    #[test]
    fn test_dynamic_processors_mutually_exclusive() {
        let settings = DspSettings {
            compressor_enabled: true,
            ..Default::default()
        };

        // Should not allow enabling limiter when compressor is on
        let result = validate_toggle(DspEffect::Limiter, &settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_conflicts() {
        let settings = DspSettings {
            tube_warmth_enabled: true,
            tape_saturation_enabled: true, // This violates rules but tests detection
            ..Default::default()
        };

        // Should detect multiple conflicts
        let result = validate_toggle(DspEffect::Transformer, &settings);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.conflicting_with.len(), 2);
    }

    #[test]
    fn test_error_message_formatting() {
        let settings = DspSettings {
            tube_warmth_enabled: true,
            ..Default::default()
        };

        let result = validate_toggle(DspEffect::TapeSaturation, &settings);
        let err = result.unwrap_err();
        let message = err.message();

        assert!(message.contains("Tape Saturation"));
        assert!(message.contains("Tube Warmth"));
        assert!(message.contains("tone enhancer"));
    }

    #[test]
    fn test_get_enabled_effects() {
        let settings = DspSettings {
            tube_warmth_enabled: true,
            stereo_width_enabled: true,
            ..Default::default()
        };

        let enabled = get_enabled_effects(&settings);
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&DspEffect::TubeWarmth));
        assert!(enabled.contains(&DspEffect::StereoWidth));
    }
}
