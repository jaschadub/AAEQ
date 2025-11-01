//! Feature negotiation and management for AANP protocol
//!
//! Implements the feature negotiation as specified in the AANP v0.4 specification.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Feature types supported by the AANP protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Feature {
    /// Micro-PLL clock synchronization
    MicroPll,
    /// CRC verification for bit-perfect delivery
    CrcVerify,
    /// Volume control
    VolumeControl,
    /// Gapless playback
    Gapless,
    /// Node capabilities
    Capabilities,
    /// Latency calibration
    LatencyCal,
    /// DSP transfer
    DspTransfer,
    /// Convolution
    Convolution,
    /// RTCP SR
    RtcpSr,
}

impl Feature {
    /// Get the feature flag name (as used in TXT records)
    pub fn flag_name(&self) -> &'static str {
        match self {
            Feature::MicroPll => "micro_pll",
            Feature::CrcVerify => "crc_verify",
            Feature::VolumeControl => "volume_control",
            Feature::Gapless => "gapless",
            Feature::Capabilities => "capabilities",
            Feature::LatencyCal => "latency_cal",
            Feature::DspTransfer => "dsp_transfer",
            Feature::Convolution => "convolution",
            Feature::RtcpSr => "rtcp_sr",
        }
    }

    /// Get the feature description
    pub fn description(&self) -> &'static str {
        match self {
            Feature::MicroPll => "Clock drift correction via resampling",
            Feature::CrcVerify => "Bit-perfect verification",
            Feature::VolumeControl => "Remote volume adjustment",
            Feature::Gapless => "Seamless track transitions",
            Feature::Capabilities => "Node hardware and software capabilities",
            Feature::LatencyCal => "Sample-accurate timing",
            Feature::DspTransfer => "Server pushes DSP state to Node",
            Feature::Convolution => "Room correction (IRs)",
            Feature::RtcpSr => "Sender reports for QoS",
        }
    }
}

/// Feature set for managing supported and active features
#[derive(Debug, Clone, Default)]
pub struct FeatureSet {
    /// Supported features
    pub supported: HashSet<Feature>,
    /// Active features (during session)
    pub active: HashSet<Feature>,
    /// Optional features (negotiated)
    pub optional: HashSet<Feature>,
}

impl FeatureSet {
    /// Create a new feature set
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a supported feature
    pub fn add_supported(&mut self, feature: Feature) {
        self.supported.insert(feature);
    }

    /// Add an active feature
    pub fn add_active(&mut self, feature: Feature) {
        self.active.insert(feature);
    }

    /// Add an optional feature
    pub fn add_optional(&mut self, feature: Feature) {
        self.optional.insert(feature);
    }

    /// Check if feature is supported
    pub fn is_supported(&self, feature: Feature) -> bool {
        self.supported.contains(&feature)
    }

    /// Check if feature is active
    pub fn is_active(&self, feature: Feature) -> bool {
        self.active.contains(&feature)
    }

    /// Check if feature is optional
    pub fn is_optional(&self, feature: Feature) -> bool {
        self.optional.contains(&feature)
    }

    /// Get all supported features as flag names
    pub fn supported_flags(&self) -> Vec<String> {
        self.supported
            .iter()
            .map(|f| f.flag_name().to_string())
            .collect()
    }

    /// Get all active features as flag names
    pub fn active_flags(&self) -> Vec<String> {
        self.active
            .iter()
            .map(|f| f.flag_name().to_string())
            .collect()
    }

    /// Get all optional features as flag names
    pub fn optional_flags(&self) -> Vec<String> {
        self.optional
            .iter()
            .map(|f| f.flag_name().to_string())
            .collect()
    }

    /// Clear all features
    pub fn clear(&mut self) {
        self.supported.clear();
        self.active.clear();
        self.optional.clear();
    }
}

/// Feature negotiation manager
pub struct FeatureNegotiator {
    /// Local supported features
    local_features: FeatureSet,
    /// Remote features (from session_init)
    remote_features: Option<FeatureSet>,
    /// Negotiated features
    negotiated_features: FeatureSet,
}

impl FeatureNegotiator {
    /// Create a new feature negotiator
    pub fn new() -> Self {
        Self {
            local_features: FeatureSet::new(),
            remote_features: None,
            negotiated_features: FeatureSet::new(),
        }
    }

    /// Initialize with local features
    pub fn initialize_local_features(&mut self, features: Vec<Feature>) {
        for feature in features {
            self.local_features.add_supported(feature);
        }
    }

    /// Process remote feature set from session_init
    pub fn process_remote_features(&mut self, remote_features: FeatureSet) -> FeatureSet {
        self.remote_features = Some(remote_features.clone());
        
        // Negotiate features - accept intersection of local and remote
        let mut negotiated = FeatureSet::new();
        
        // Core features (must be supported by both)
        for feature in &self.local_features.supported {
            if self.remote_features.as_ref().map_or(false, |rf| rf.supported.contains(feature)) {
                negotiated.add_active(*feature);
            }
        }
        
        // Optional features (accept if both support)
        for feature in &self.local_features.optional {
            if self.remote_features.as_ref().map_or(false, |rf| rf.optional.contains(feature)) {
                negotiated.add_active(*feature);
            }
        }
        
        self.negotiated_features = negotiated.clone();
        negotiated
    }

    /// Get negotiated features
    pub fn get_negotiated_features(&self) -> &FeatureSet {
        &self.negotiated_features
    }

    /// Check if feature negotiation was successful
    pub fn is_negotiation_successful(&self) -> bool {
        self.negotiated_features.active.is_empty() || 
        !self.negotiated_features.active.is_subset(&self.local_features.supported)
    }
}

/// Feature configuration for specific protocol aspects
#[derive(Debug, Clone, Default)]
pub struct FeatureConfiguration {
    /// Micro-PLL settings
    pub micro_pll: MicroPllSettings,
    /// CRC verification settings
    pub crc_verify: CrcSettings,
    /// Volume control settings
    pub volume_control: VolumeControlSettings,
    /// Gapless playback settings
    pub gapless: GaplessSettings,
    /// Buffer management settings
    pub buffer_management: BufferSettings,
}

/// Micro-PLL settings
#[derive(Debug, Clone, Default)]
pub struct MicroPllSettings {
    /// Enable Micro-PLL
    pub enabled: bool,
    /// Maximum drift in ppm
    pub ppm_limit: i32,
    /// Adjustment interval in milliseconds
    pub adjustment_interval_ms: u32,
    /// Slew rate in ppm per second
    pub slew_rate_ppm_per_sec: i32,
    /// EMA window size
    pub ema_window: u32,
}

/// CRC settings
#[derive(Debug, Clone, Default)]
pub struct CrcSettings {
    /// Enable CRC verification
    pub enabled: bool,
    /// Check window size
    pub check_window: u32,
}

/// Volume control settings
#[derive(Debug, Clone, Default)]
pub struct VolumeControlSettings {
    /// Enable volume control
    pub enabled: bool,
    /// Supported volume curves
    pub supported_curves: Vec<String>,
    /// Ramp shapes
    pub ramp_shapes: Vec<String>,
}

/// Gapless playback settings
#[derive(Debug, Clone, Default)]
pub struct GaplessSettings {
    /// Enable gapless playback
    pub enabled: bool,
    /// Extension ID
    pub extension_id: u8,
}

/// Buffer management settings
#[derive(Debug, Clone, Default)]
pub struct BufferSettings {
    /// Target buffer size in milliseconds
    pub target_ms: u32,
    /// Minimum buffer size in milliseconds
    pub min_ms: u32,
    /// Maximum buffer size in milliseconds
    pub max_ms: u32,
    /// Start threshold in milliseconds
    pub start_threshold_ms: u32,
}

/// Feature validation utilities
pub struct FeatureValidator;

impl FeatureValidator {
    /// Validate feature compatibility
    pub fn validate_compatibility(
        local_features: &FeatureSet,
        remote_features: &FeatureSet,
    ) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // Check version compatibility
        // (In a real implementation, this would check protocol versions)
        
        // Check feature availability
        for feature in &local_features.supported {
            if !remote_features.supported.contains(feature) {
                warnings.push(format!("Feature '{}' not supported by remote", feature.flag_name()));
            }
        }
        
        // Check required features
        if !local_features.supported.contains(&Feature::MicroPll) {
            warnings.push("Micro-PLL not supported - clock drift correction disabled".to_string());
        }
        
        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Validate feature configuration
    pub fn validate_configuration(config: &FeatureConfiguration) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // Validate Micro-PLL settings
        if config.micro_pll.ppm_limit < 0 {
            errors.push("Micro-PLL PPM limit must be positive".to_string());
        }
        
        if config.micro_pll.ppm_limit > 500 {
            warnings.push("Micro-PLL PPM limit is very high".to_string());
        }
        
        // Validate buffer settings
        if config.buffer_management.target_ms < config.buffer_management.min_ms {
            errors.push("Buffer target size must be >= minimum size".to_string());
        }
        
        if config.buffer_management.target_ms > config.buffer_management.max_ms {
            errors.push("Buffer target size must be <= maximum size".to_string());
        }
        
        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }
}

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether validation passed
    pub is_valid: bool,
    /// Error messages
    pub errors: Vec<String>,
    /// Warning messages
    pub warnings: Vec<String>,
}

/// Feature state machine
pub struct FeatureStateMachine {
    /// Current feature state
    pub current_state: FeatureState,
    /// Feature state history
    pub state_history: Vec<(FeatureState, u64)>,
}

/// Feature states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureState {
    /// Feature disabled
    Disabled,
    /// Feature enabled
    Enabled,
    /// Feature in transition
    Transitioning,
    /// Feature error
    Error,
}

impl FeatureStateMachine {
    /// Create a new feature state machine
    pub fn new() -> Self {
        Self {
            current_state: FeatureState::Disabled,
            state_history: Vec::new(),
        }
    }

    /// Transition to new state
    pub fn transition_to(&mut self, new_state: FeatureState) {
        let timestamp = Self::get_current_timestamp();
        self.state_history.push((self.current_state, timestamp));
        self.current_state = new_state;
    }

    /// Get current timestamp
    fn get_current_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Get current state
    pub fn get_state(&self) -> FeatureState {
        self.current_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_flags() {
        let feature = Feature::MicroPll;
        assert_eq!(feature.flag_name(), "micro_pll");
        assert_eq!(feature.description(), "Clock drift correction via resampling");
    }

    #[test]
    fn test_feature_set() {
        let mut features = FeatureSet::new();
        features.add_supported(Feature::MicroPll);
        features.add_supported(Feature::CrcVerify);
        features.add_optional(Feature::DspTransfer);
        
        assert!(features.is_supported(Feature::MicroPll));
        assert!(features.is_optional(Feature::DspTransfer));
        assert!(!features.is_supported(Feature::Gapless));
    }

    #[test]
    fn test_feature_validation() {
        let config = FeatureConfiguration::default();
        let result = FeatureValidator::validate_configuration(&config);
        
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }
}