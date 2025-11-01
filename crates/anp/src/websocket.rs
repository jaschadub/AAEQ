//! WebSocket control channel implementation for AANP protocol
//!
//! Implements the WebSocket control channel with snake_case message naming as specified.

use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tokio::sync::mpsc;
use std::collections::HashMap;
use futures_util::{SinkExt, StreamExt};

// Re-export types from other modules
use crate::session::{SessionInit, SessionAccept, RtpConfig, RtpExtensions,
    GaplessExtension, Crc32Extension, RecommendedConfig, LatencyInfo,
    MicroPllConfig, VolumeConfig, BufferConfig};
use crate::health::HealthMessage;

/// Control message types as defined in the specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlMessage {
    /// Session initialization from node
    #[serde(rename = "session_init")]
    SessionInit(SessionInit),
    
    /// Session acceptance from server
    #[serde(rename = "session_accept")]
    SessionAccept(SessionAccept),
    
    /// Volume set command
    #[serde(rename = "volume_set")]
    VolumeSet(VolumeSet),
    
    /// Volume get request
    #[serde(rename = "volume_get")]
    VolumeGet(VolumeGet),
    
    /// Volume result response
    #[serde(rename = "volume_result")]
    VolumeResult(VolumeResult),
    
    /// Health telemetry
    #[serde(rename = "health")]
    Health(HealthMessage),
    
    /// DSP update
    #[serde(rename = "dsp_update")]
    DspUpdate(DspUpdate),
    
    /// DSP update acknowledgment
    #[serde(rename = "dsp_update_ack")]
    DspUpdateAck(DspUpdateAck),
    
    /// Stream pause
    #[serde(rename = "stream_pause")]
    StreamPause(StreamPause),
    
    /// Stream resumed
    #[serde(rename = "stream_resume")]
    StreamResume(StreamResume),
    
    /// Stream stopped
    #[serde(rename = "stream_stop")]
    StreamStop(StreamStop),
    
    /// Error notification
    #[serde(rename = "error")]
    Error(ErrorMessage),
}

/// Volume control message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSet {
    /// Volume level (0.0-1.0)
    pub level: f32,
    /// Mute state
    pub mute: bool,
    /// Ramp time in milliseconds
    pub ramp_ms: Option<u32>,
    /// Ramp shape
    pub ramp_shape: Option<String>,
}

/// Volume get request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeGet {
    // Empty struct (no fields needed)
}

/// Volume result response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeResult {
    /// Status of operation
    pub status: String,
    /// Current volume level
    pub level: f32,
    /// Mute state
    pub mute: bool,
    /// Hardware control flag
    pub hardware_control: bool,
    /// DAC volume in dB
    pub dac_volume_db: f32,
    /// Gain in dB
    pub gain_db: f32,
    /// Curve type
    pub curve_type: String,
}

// Health-related types are imported from health.rs module

/// Stream pause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamPause {
    // Empty struct
}

/// Stream resumed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamResume {
    // Empty struct
}

/// Stream stop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStop {
    // Empty struct
}

/// DSP update message as defined in the specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspUpdate {
    /// Profile ID
    pub profile_id: u32,
    /// Profile name
    pub profile_name: String,
    /// Headroom in dB
    pub headroom_db: f32,
    /// Dithering type
    pub dithering: String,
    /// Equalizer configuration
    pub equalizer: Option<EqualizerConfig>,
    /// Convolution configuration
    pub convolution: Option<ConvolutionConfig>,
}

/// Equalizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqualizerConfig {
    /// Name
    pub name: String,
    /// Enabled flag
    pub enabled: bool,
    /// EQ bands
    pub bands: Vec<EqBand>,
}

/// EQ band definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    /// Frequency in Hz
    pub frequency: u32,
    /// Gain in dB
    pub gain: f32,
    /// Q factor
    pub q: f32,
    /// Filter type
    #[serde(rename = "type")]
    pub filter_type: String,
}

/// Convolution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvolutionConfig {
    /// Enabled flag
    pub enabled: bool,
    /// Filter ID
    pub filter_id: String,
    /// Delay in samples
    pub delay_samples: u32,
    /// Gain in dB
    pub gain_db: f32,
}

/// DSP update acknowledgment message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspUpdateAck {
    /// Profile ID
    pub profile_id: u32,
    /// Status
    pub status: String,
    /// Profile hash
    pub profile_hash: u32,
    /// Applied features
    pub applied: DspAppliedFeatures,
    /// Errors
    pub errors: Vec<DspError>,
    /// Fallback options
    pub fallback: Option<HashMap<String, String>>,
}

/// DSP applied features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspAppliedFeatures {
    /// Equalizer applied
    pub equalizer: bool,
    /// Headroom applied
    pub headroom: bool,
    /// Dithering applied
    pub dithering: bool,
    /// Convolution applied
    pub convolution: bool,
}

/// DSP error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspError {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
}

/// Error message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// Error code
    pub code: String,
    /// Category
    pub category: String,
    /// Severity
    pub severity: String,
    /// Message
    pub message: String,
    /// Details
    pub details: Option<HashMap<String, serde_json::Value>>,
    /// Recovery action
    pub recovery_action: Option<String>,
}

/// WebSocket connection manager
pub struct WebSocketManager {
    /// WebSocket stream
    pub stream: Option<WebSocketStream<tokio::net::TcpStream>>,
    /// Message sender
    pub message_sender: mpsc::UnboundedSender<ControlMessage>,
    /// Message receiver
    pub message_receiver: mpsc::UnboundedReceiver<ControlMessage>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            stream: None,
            message_sender: tx,
            message_receiver: rx,
        }
    }

    /// Send a control message
    pub async fn send_message(&mut self, message: ControlMessage) -> Result<(), String> {
        // Convert to JSON and then to WebSocket message
        let json = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize message: {:?}", e))?;
        
        let ws_message = Message::Text(json);
        
        if let Some(stream) = &mut self.stream {
            stream.send(ws_message)
                .await
                .map_err(|e| format!("Failed to send message: {:?}", e))?;
        }
        
        Ok(())
    }

    /// Receive a control message
    pub async fn receive_message(&mut self) -> Result<Option<ControlMessage>, String> {
        if let Some(stream) = &mut self.stream {
            let message = match stream.next().await {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => return Err(format!("WebSocket error: {:?}", e)),
                None => return Err("Stream ended".to_string()),
            };
                
            match message {
                Message::Text(text) => {
                    let parsed: ControlMessage = serde_json::from_str(&text)
                        .map_err(|e| format!("Failed to parse message: {:?}", e))?;
                    Ok(Some(parsed))
                }
                Message::Binary(_) => {
                    // Handle binary messages if needed
                    Ok(None)
                }
                Message::Ping(_) => {
                    // Handle ping
                    Ok(None)
                }
                Message::Pong(_) => {
                    // Handle pong
                    Ok(None)
                }
                Message::Close(_) => {
                    // Handle close
                    Ok(None)
                }
                Message::Frame(_) => {
                    // Handle frame
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Set the WebSocket stream
    pub fn set_stream(&mut self, stream: WebSocketStream<tokio::net::TcpStream>) {
        self.stream = Some(stream);
    }
}

/// Control message handler
pub struct ControlMessageHandler {
    /// Handler state
    pub state: HandlerState,
}

/// Handler state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerState {
    /// Disconnected
    Disconnected,
    /// Connected
    Connected,
    /// Session negotiating
    Negotiating,
    /// Playing
    Playing,
    /// Error
    Error,
}

impl ControlMessageHandler {
    /// Create a new handler
    pub fn new() -> Self {
        Self {
            state: HandlerState::Disconnected,
        }
    }

    /// Handle incoming session_init message
    pub fn handle_session_init(&mut self, init: &SessionInit) -> Result<SessionAccept, String> {
        // Validate node UUID
        if init.node_uuid.is_nil() {
            return Err("Invalid node UUID".to_string());
        }

        // Update state
        self.state = HandlerState::Negotiating;

        // Create session accept response
        let session_accept = SessionAccept {
            protocol_version: "0.4".to_string(),
            session_id: format!("srv-{}", generate_session_id()),
            active_features: init.features.clone(),
            optional_features: init.optional_features.clone(),
            rtp_config: Default::default(),
            rtp_extensions: Default::default(),
            recommended_config: Default::default(),
            latency: Default::default(),
            micro_pll: Default::default(),
            volume: Default::default(),
            buffer: Default::default(),
        };

        // Update state
        self.state = HandlerState::Connected;

        Ok(session_accept)
    }

    /// Handle volume_set message
    pub fn handle_volume_set(&mut self, volume_set: &VolumeSet) -> VolumeResult {
        VolumeResult {
            status: "success".to_string(),
            level: volume_set.level,
            mute: volume_set.mute,
            hardware_control: false, // Default to software control
            dac_volume_db: 0.0, // Would be calculated based on hardware
            gain_db: calculate_gain_db(volume_set.level),
            curve_type: "logarithmic".to_string(),
        }
    }

    /// Handle health message
    pub fn handle_health(&mut self, health: &HealthMessage) -> Result<(), String> {
        // Process health data
        // This would typically update internal state and telemetry
        Ok(())
    }

    /// Get current handler state
    pub fn get_state(&self) -> HandlerState {
        self.state
    }
}

/// Default implementations for control message structs
impl Default for SessionAccept {
    fn default() -> Self {
        Self {
            protocol_version: "0.4".to_string(),
            session_id: "srv-default".to_string(),
            active_features: vec![],
            optional_features: vec![],
            rtp_config: RtpConfig {
                ssrc: 0,
                payload_type: 96,
                timestamp_rate: 48000,
                initial_sequence: 0,
                initial_timestamp: 0,
            },
            rtp_extensions: RtpExtensions {
                gapless: GaplessExtension {
                    enabled: false,
                    extension_id: 1,
                },
                crc32: Crc32Extension {
                    enabled: false,
                    extension_id: 2,
                    window: 64,
                },
            },
            recommended_config: RecommendedConfig {
                sample_rate: 48000,
                format: "S24LE".to_string(),
                buffer_ms: 150,
                reason: "Default configuration".to_string(),
            },
            latency: LatencyInfo {
                dac_ms: 1.34,
                pipeline_ms: 0.62,
                comp_mode: "exact".to_string(),
            },
            micro_pll: MicroPllConfig {
                enabled: true,
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
        }
    }
}

impl Default for RtpConfig {
    fn default() -> Self {
        Self {
            ssrc: 0,
            payload_type: 96,
            timestamp_rate: 48000,
            initial_sequence: 0,
            initial_timestamp: 0,
        }
    }
}

impl Default for RtpExtensions {
    fn default() -> Self {
        Self {
            gapless: GaplessExtension {
                enabled: false,
                extension_id: 1,
            },
            crc32: Crc32Extension {
                enabled: false,
                extension_id: 2,
                window: 64,
            },
        }
    }
}

impl Default for RecommendedConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            format: "S24LE".to_string(),
            buffer_ms: 150,
            reason: "Default configuration".to_string(),
        }
    }
}

impl Default for LatencyInfo {
    fn default() -> Self {
        Self {
            dac_ms: 1.34,
            pipeline_ms: 0.62,
            comp_mode: "exact".to_string(),
        }
    }
}

impl Default for MicroPllConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ppm_limit: 150,
            adjustment_interval_ms: 100,
            slew_rate_ppm_per_sec: 10,
            ema_window: 8,
        }
    }
}

impl Default for VolumeConfig {
    fn default() -> Self {
        Self {
            initial_level: 0.75,
            mute: false,
            control_mode: "software".to_string(),
            curve_type: "logarithmic".to_string(),
        }
    }
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            target_ms: 150,
            min_ms: 50,
            max_ms: 500,
            start_threshold_ms: 100,
        }
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

/// Calculate gain in dB from volume level
fn calculate_gain_db(level: f32) -> f32 {
    if level == 0.0 {
        f32::NEG_INFINITY // -∞ dB
    } else {
        40.0 * level.log10() // 40 * log10(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_calculation() {
        // Test mute case
        let gain = calculate_gain_db(0.0);
        assert!(gain.is_infinite());
        assert!(gain.is_sign_negative());
        
        // Test normal case
        let gain = calculate_gain_db(0.5);
        assert!(gain < 0.0); // Should be negative
        assert_eq!(gain, 40.0 * 0.5f32.log10());
    }

    #[test]
    fn test_websocket_manager_creation() {
        let manager = WebSocketManager::new();
        assert!(manager.stream.is_none());
        assert!(manager.message_receiver.is_empty());
    }

    #[test]
    fn test_control_message_serialization() {
        let volume_set = VolumeSet {
            level: 0.75,
            mute: false,
            ramp_ms: Some(100),
            ramp_shape: Some("s_curve".to_string()),
        };

        let control_msg = ControlMessage::VolumeSet(volume_set);
        let json = serde_json::to_string(&control_msg).unwrap();
        
        // Should contain snake_case keys
        assert!(json.contains("\"level\""));
        assert!(json.contains("\"mute\""));
        assert!(json.contains("\"ramp_ms\""));
        assert!(json.contains("\"ramp_shape\""));
    }
}