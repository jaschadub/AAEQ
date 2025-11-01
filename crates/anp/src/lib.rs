//! AANP (AAEQ Node Protocol) v0.4 Implementation
//!
//! This crate implements the AAEQ Node Protocol specification for high-fidelity,
//! low-latency network audio streaming with advanced features like clock synchronization
//! and error recovery.
//!
//! # Features
//!
//! - High-fidelity, low-latency audio transport
//! - Advanced clock synchronization with Micro-PLL
//! - Bit-perfect delivery with CRC verification
//! - Remote volume control with multiple curve types
//! - Comprehensive health telemetry
//! - Robust error handling and recovery
//!
//! # Protocol Compliance
//!
//! This implementation strictly follows the AANP v0.4 specification with:
//! - Proper RTP header structure
//! - Correct endianness handling for audio samples
//! - Standardized error codes and recovery protocols
//! - Complete feature negotiation framework
//! - Comprehensive health telemetry system

pub mod protocol;
pub mod discovery;
pub mod session;
pub mod rtp;
pub mod websocket;
pub mod health;
pub mod errors;
pub mod features;

// Re-export key types, being explicit to avoid ambiguous glob re-exports
pub use protocol::{PROTOCOL_VERSION, VolumeCurve};
pub use discovery::*;
pub use session::{SessionInit, SessionAccept, SessionManager, SessionState};
pub use rtp::{RtpHeader, RtpPacket, RtpPacketBuilder, RtpPayloadType, RtpPayloadUtils,
    RtpStreamManager, RtpExtensions, GaplessExtension as RtpGaplessExtension,
    Crc32Extension as RtpCrc32Extension};
pub use websocket::{ControlMessage, WebSocketManager};
pub use health::{HealthMessage, HealthManager};
pub use errors::{ErrorCode, ErrorMessage as ErrorMsg, ErrorHandler, ErrorSeverity};
pub use features::{Feature, FeatureSet};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_imports() {
        // Test that all modules can be imported and used
        let _ = PROTOCOL_VERSION;
        let _ = RtpPayloadType::L24;
        // Feature enum is now in features module
        let _ = features::Feature::MicroPll;
    }

    #[test]
    fn test_feature_sets() {
        let mut features = FeatureSet::new();
        features.add_supported(features::Feature::MicroPll);
        features.add_supported(features::Feature::CrcVerify);
        features.add_optional(features::Feature::DspTransfer);

        assert!(features.is_supported(features::Feature::MicroPll));
        assert!(features.is_optional(features::Feature::DspTransfer));
    }

    #[test]
    fn test_error_codes() {
        let error = ErrorCode::ConnectionTimeout;
        assert_eq!(error.code(), "E102");
        assert_eq!(error.category(), "connection");
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_health_metrics() {
        let mut manager = HealthManager::new();
        let health = manager.get_health_message();
        
        assert!(health.timestamp_us > 0);
        assert_eq!(health.connection.state, "idle");
        assert_eq!(health.playback.state, "idle");
    }
}