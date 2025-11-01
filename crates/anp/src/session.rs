//! Session management for AANP protocol
//!
//! Handles session initialization, acceptance, and state transitions as defined in the specification.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashSet;

/// Session initialization message from node to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInit {
    /// Protocol version
    pub protocol_version: String,
    /// Node UUID
    pub node_uuid: Uuid,
    /// Supported features
    pub features: Vec<String>,
    /// Optional features
    pub optional_features: Vec<String>,
    /// Latency compensation support
    pub latency_comp: bool,
    /// Node capabilities
    pub node_capabilities: NodeCapabilities,
}

/// Node capabilities as defined in the specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Hardware platform description
    pub hardware: String,
    /// DAC name
    pub dac_name: String,
    /// DAC chip model
    pub dac_chip: String,
    /// Maximum supported sample rate
    pub max_sample_rate: u32,
    /// Supported audio formats
    pub supported_formats: Vec<String>,
    /// Native format
    pub native_format: String,
    /// Maximum number of channels
    pub max_channels: u8,
    /// Buffer range in milliseconds
    pub buffer_range_ms: [u32; 2],
    /// Whether hardware volume control is supported
    pub has_hardware_volume: bool,
    /// Volume range (normalized 0.0-1.0)
    pub volume_range: [f32; 2],
    /// Supported volume curves
    pub volume_curve: String,
    /// CPU information
    pub cpu_info: CpuInfo,
    /// DSP capabilities
    pub dsp_capabilities: DspCapabilities,
}

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// Architecture
    pub arch: String,
    /// Number of cores
    pub cores: u8,
    /// CPU frequency in MHz
    pub freq_mhz: u32,
}

/// DSP capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspCapabilities {
    /// Whether EQ is supported
    pub can_eq: bool,
    /// Whether resampling is supported
    pub can_resample: bool,
    /// Whether convolution is supported
    pub can_convolve: bool,
}

/// Session acceptance message from server to node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAccept {
    /// Protocol version
    pub protocol_version: String,
    /// Session ID
    pub session_id: String,
    /// Active features
    pub active_features: Vec<String>,
    /// Optional features (accepted)
    pub optional_features: Vec<String>,
    /// RTP configuration
    pub rtp_config: RtpConfig,
    /// RTP extensions configuration
    pub rtp_extensions: RtpExtensions,
    /// Recommended configuration
    pub recommended_config: RecommendedConfig,
    /// Latency information
    pub latency: LatencyInfo,
    /// Micro-PLL configuration
    pub micro_pll: MicroPllConfig,
    /// Volume configuration
    pub volume: VolumeConfig,
    /// Buffer configuration
    pub buffer: BufferConfig,
}

/// RTP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpConfig {
    /// SSRC (Synchronization Source Identifier)
    pub ssrc: u32,
    /// Payload type
    pub payload_type: u8,
    /// Timestamp rate (sample rate)
    pub timestamp_rate: u32,
    /// Initial sequence number
    pub initial_sequence: u16,
    /// Initial timestamp
    pub initial_timestamp: u32,
}

/// RTP extensions configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpExtensions {
    /// Gapless playback extension
    pub gapless: GaplessExtension,
    /// CRC32 extension
    pub crc32: Crc32Extension,
}

/// Gapless playback extension configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaplessExtension {
    /// Whether extension is enabled
    pub enabled: bool,
    /// Extension ID
    pub extension_id: u8,
}

/// CRC32 extension configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crc32Extension {
    /// Whether extension is enabled
    pub enabled: bool,
    /// Extension ID
    pub extension_id: u8,
    /// Window size for CRC checking
    pub window: u32,
}

/// Recommended configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedConfig {
    /// Sample rate
    pub sample_rate: u32,
    /// Format
    pub format: String,
    /// Buffer size in milliseconds
    pub buffer_ms: u32,
    /// Reason for recommendation
    pub reason: String,
}

/// Latency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyInfo {
    /// DAC latency in milliseconds
    pub dac_ms: f64,
    /// Pipeline latency in milliseconds
    pub pipeline_ms: f64,
    /// Compensation mode
    pub comp_mode: String,
}

/// Micro-PLL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroPllConfig {
    /// Whether Micro-PLL is enabled
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

/// Volume configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    /// Initial volume level (0.0-1.0)
    pub initial_level: f32,
    /// Mute state
    pub mute: bool,
    /// Control mode (software/hardware/auto)
    pub control_mode: String,
    /// Curve type
    pub curve_type: String,
}

/// Buffer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferConfig {
    /// Target buffer size in milliseconds
    pub target_ms: u32,
    /// Minimum buffer size in milliseconds
    pub min_ms: u32,
    /// Maximum buffer size in milliseconds
    pub max_ms: u32,
    /// Start threshold in milliseconds
    pub start_threshold_ms: u32,
}

/// Session manager for handling session lifecycle
pub struct SessionManager {
    /// Current session state
    pub state: SessionState,
    /// Session ID
    pub session_id: Option<String>,
    /// Node UUID
    pub node_uuid: Option<Uuid>,
    /// Supported features
    pub supported_features: HashSet<String>,
    /// Active features
    pub active_features: HashSet<String>,
}

/// Session states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Disconnected state
    Disconnected,
    /// Idle state
    Idle,
    /// Negotiating state
    Negotiating,
    /// Buffering state
    Buffering,
    /// Playing state
    Playing,
    /// Paused state
    Paused,
    /// Error state
    Error,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            state: SessionState::Disconnected,
            session_id: None,
            node_uuid: None,
            supported_features: HashSet::new(),
            active_features: HashSet::new(),
        }
    }

    /// Initialize session with node capabilities
    pub fn initialize_session(&mut self, init_msg: &SessionInit) -> Result<SessionAccept, String> {
        // Validate protocol version
        if init_msg.protocol_version != "0.4" {
            return Err("Unsupported protocol version".to_string());
        }

        // Update session state
        self.state = SessionState::Negotiating;
        self.node_uuid = Some(init_msg.node_uuid);
        
        // Build accepted features
        let mut active_features = Vec::new();
        let mut optional_features = Vec::new();
        
        // Accept supported features
        for feature in &init_msg.features {
            if Self::is_supported_feature(feature) {
                active_features.push(feature.clone());
            }
        }
        
        // Accept optional features
        for feature in &init_msg.optional_features {
            if Self::is_optional_feature(feature) {
                optional_features.push(feature.clone());
            }
        }
        
        // Generate session ID
        let session_id = format!("srv-{}", generate_session_id());
        
        // Create session accept message
        let session_accept = SessionAccept {
            protocol_version: "0.4".to_string(),
            session_id: session_id.clone(),
            active_features: active_features.clone(),
            optional_features: optional_features.clone(),
            rtp_config: RtpConfig {
                ssrc: generate_ssrc(&session_id),
                payload_type: 96, // L24 format
                timestamp_rate: 48000, // Default sample rate
                initial_sequence: 0,
                initial_timestamp: 0,
            },
            rtp_extensions: RtpExtensions {
                gapless: GaplessExtension {
                    enabled: active_features.contains(&"gapless".to_string()),
                    extension_id: 1,
                },
                crc32: Crc32Extension {
                    enabled: active_features.contains(&"crc_verify".to_string()),
                    extension_id: 2,
                    window: 64,
                },
            },
            recommended_config: RecommendedConfig {
                sample_rate: 48000,
                format: "S24LE".to_string(),
                buffer_ms: 150,
                reason: "Optimal for your hardware and network".to_string(),
            },
            latency: LatencyInfo {
                dac_ms: 1.34,
                pipeline_ms: 0.62,
                comp_mode: "exact".to_string(),
            },
            micro_pll: MicroPllConfig {
                enabled: active_features.contains(&"micro_pll".to_string()),
                ppm_limit: 150,
                adjustment_interval_ms: 100,
                slew_rate_ppm_per_sec: 10,
                ema_window: 8,
            },
            volume: VolumeConfig {
                initial_level: 0.75,
                mute: false,
                control_mode: "software".to_string(),
                curve_type: "logarithmic".to_string(),
            },
            buffer: BufferConfig {
                target_ms: 150,
                min_ms: 50,
                max_ms: 500,
                start_threshold_ms: 100,
            },
        };

        // Update session state
        self.session_id = Some(session_id);
        self.active_features = active_features.iter().cloned().collect();
        
        // Transition to buffering state
        self.state = SessionState::Buffering;
        
        Ok(session_accept)
    }

    /// Check if a feature is supported
    fn is_supported_feature(feature: &str) -> bool {
        matches!(feature, 
            "micro_pll" | "crc_verify" | "volume_control" | "gapless" | "capabilities"
        )
    }

    /// Check if a feature is optional
    fn is_optional_feature(feature: &str) -> bool {
        matches!(feature, 
            "dsp_transfer" | "convolution" | "rtcp_sr"
        )
    }

    /// Handle session state transitions
    pub fn transition_state(&mut self, new_state: SessionState) {
        self.state = new_state;
    }

    /// Get current session state
    pub fn get_state(&self) -> SessionState {
        self.state
    }

    /// Get session ID
    pub fn get_session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }

    /// Get node UUID
    pub fn get_node_uuid(&self) -> Option<&Uuid> {
        self.node_uuid.as_ref()
    }
}

/// Generate a session ID (mock implementation)
fn generate_session_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// Generate SSRC (mock implementation)
fn generate_ssrc(session_id: &str) -> u32 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    
    let mut hasher = DefaultHasher::new();
    session_id.hash(&mut hasher);
    hasher.finish() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::new();
        assert_eq!(manager.state, SessionState::Disconnected);
        assert!(manager.session_id.is_none());
        assert!(manager.node_uuid.is_none());
    }

    #[test]
    fn test_session_initialization() {
        let mut manager = SessionManager::new();
        
        let init_msg = SessionInit {
            protocol_version: "0.4".to_string(),
            node_uuid: Uuid::new_v4(),
            features: vec![
                "micro_pll".to_string(),
                "crc_verify".to_string(),
                "volume_control".to_string(),
                "gapless".to_string(),
                "capabilities".to_string(),
            ],
            optional_features: vec![
                "dsp_transfer".to_string(),
            ],
            latency_comp: true,
            node_capabilities: NodeCapabilities {
                hardware: "Raspberry Pi 4".to_string(),
                dac_name: "HiFiBerry DAC+".to_string(),
                dac_chip: "PCM5122".to_string(),
                max_sample_rate: 192000,
                supported_formats: vec!["F32".to_string(), "S24LE".to_string(), "S16LE".to_string()],
                native_format: "S24LE".to_string(),
                max_channels: 2,
                buffer_range_ms: [50, 500],
                has_hardware_volume: true,
                volume_range: [0.0, 1.0],
                volume_curve: "logarithmic".to_string(),
                cpu_info: CpuInfo {
                    arch: "ARMv8".to_string(),
                    cores: 4,
                    freq_mhz: 1500,
                },
                dsp_capabilities: DspCapabilities {
                    can_eq: false,
                    can_resample: false,
                    can_convolve: false,
                },
            },
        };
        
        let result = manager.initialize_session(&init_msg);
        assert!(result.is_ok());
        assert_eq!(manager.state, SessionState::Buffering);
    }
}