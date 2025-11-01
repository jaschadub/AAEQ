//! Error handling implementation for AANP protocol
//!
//! Implements the standardized error codes and recovery protocols as specified.

use serde::{Deserialize, Serialize};

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Fatal error - session cannot continue
    Fatal,
    /// Warning - degraded performance
    Warning,
    /// Info - informational only
    Info,
}

/// Standardized error codes as defined in the specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// Connection errors
    ConnectionUnreachable,
    ConnectionTimeout,
    ConnectionRefused,
    WebSocketError,
    RtpPortBindFailed,

    /// Protocol errors
    VersionMismatch,
    InvalidSessionInit,
    InvalidMessageFormat,
    UnsupportedFeature,
    SsrcConflict,

    /// Audio errors
    UnsupportedSampleRate,
    UnsupportedFormat,
    DacOpenFailed,
    BufferUnderrun,
    BufferOverrun,
    CrcVerificationFailed,

    /// Clock errors
    DriftTooHigh,
    PllUnlock,
    TimestampDiscontinuity,

    /// DSP errors
    EqApplicationFailed,
    ConvolutionFailed,
    InsufficientCpu,
    ProfileHashMismatch,

    /// Volume errors
    HardwareVolumeUnavailable,
    VolumeOutOfRange,
}

impl ErrorCode {
    /// Get the error code string
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCode::ConnectionUnreachable => "E101",
            ErrorCode::ConnectionTimeout => "E102",
            ErrorCode::ConnectionRefused => "E103",
            ErrorCode::WebSocketError => "E104",
            ErrorCode::RtpPortBindFailed => "E105",
            ErrorCode::VersionMismatch => "E201",
            ErrorCode::InvalidSessionInit => "E202",
            ErrorCode::InvalidMessageFormat => "E203",
            ErrorCode::UnsupportedFeature => "E204",
            ErrorCode::SsrcConflict => "E205",
            ErrorCode::UnsupportedSampleRate => "E301",
            ErrorCode::UnsupportedFormat => "E302",
            ErrorCode::DacOpenFailed => "E303",
            ErrorCode::BufferUnderrun => "E304",
            ErrorCode::BufferOverrun => "E305",
            ErrorCode::CrcVerificationFailed => "E306",
            ErrorCode::DriftTooHigh => "E401",
            ErrorCode::PllUnlock => "E402",
            ErrorCode::TimestampDiscontinuity => "E403",
            ErrorCode::EqApplicationFailed => "E501",
            ErrorCode::ConvolutionFailed => "E502",
            ErrorCode::InsufficientCpu => "E503",
            ErrorCode::ProfileHashMismatch => "E504",
            ErrorCode::HardwareVolumeUnavailable => "E601",
            ErrorCode::VolumeOutOfRange => "E602",
        }
    }

    /// Get the error category
    pub fn category(&self) -> &'static str {
        match self {
            ErrorCode::ConnectionUnreachable
            | ErrorCode::ConnectionTimeout
            | ErrorCode::ConnectionRefused
            | ErrorCode::WebSocketError
            | ErrorCode::RtpPortBindFailed => "connection",
            ErrorCode::VersionMismatch
            | ErrorCode::InvalidSessionInit
            | ErrorCode::InvalidMessageFormat
            | ErrorCode::UnsupportedFeature
            | ErrorCode::SsrcConflict => "protocol",
            ErrorCode::UnsupportedSampleRate
            | ErrorCode::UnsupportedFormat
            | ErrorCode::DacOpenFailed
            | ErrorCode::BufferUnderrun
            | ErrorCode::BufferOverrun
            | ErrorCode::CrcVerificationFailed => "audio",
            ErrorCode::DriftTooHigh
            | ErrorCode::PllUnlock
            | ErrorCode::TimestampDiscontinuity => "clock",
            ErrorCode::EqApplicationFailed
            | ErrorCode::ConvolutionFailed
            | ErrorCode::InsufficientCpu
            | ErrorCode::ProfileHashMismatch => "dsp",
            ErrorCode::HardwareVolumeUnavailable
            | ErrorCode::VolumeOutOfRange => "volume",
        }
    }

    /// Get the error severity
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // Fatal errors
            ErrorCode::ConnectionUnreachable
            | ErrorCode::ConnectionRefused
            | ErrorCode::WebSocketError
            | ErrorCode::RtpPortBindFailed
            | ErrorCode::VersionMismatch
            | ErrorCode::InvalidSessionInit
            | ErrorCode::UnsupportedSampleRate
            | ErrorCode::UnsupportedFormat
            | ErrorCode::DacOpenFailed => ErrorSeverity::Fatal,

            // Warning errors
            ErrorCode::ConnectionTimeout
            | ErrorCode::InvalidMessageFormat
            | ErrorCode::UnsupportedFeature
            | ErrorCode::SsrcConflict
            | ErrorCode::BufferUnderrun
            | ErrorCode::BufferOverrun
            | ErrorCode::CrcVerificationFailed
            | ErrorCode::DriftTooHigh
            | ErrorCode::PllUnlock
            | ErrorCode::TimestampDiscontinuity
            | ErrorCode::EqApplicationFailed
            | ErrorCode::ConvolutionFailed
            | ErrorCode::InsufficientCpu
            | ErrorCode::ProfileHashMismatch
            | ErrorCode::VolumeOutOfRange => ErrorSeverity::Warning,

            // Info errors
            ErrorCode::HardwareVolumeUnavailable => ErrorSeverity::Info,
        }
    }

    /// Get human-readable error message
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::ConnectionUnreachable => "Network unreachable",
            ErrorCode::ConnectionTimeout => "Connection timeout",
            ErrorCode::ConnectionRefused => "Connection refused",
            ErrorCode::WebSocketError => "WebSocket error",
            ErrorCode::RtpPortBindFailed => "RTP port bind failed",
            ErrorCode::VersionMismatch => "Protocol version mismatch",
            ErrorCode::InvalidSessionInit => "Invalid session initialization",
            ErrorCode::InvalidMessageFormat => "Invalid message format",
            ErrorCode::UnsupportedFeature => "Unsupported feature",
            ErrorCode::SsrcConflict => "SSRC conflict detected",
            ErrorCode::UnsupportedSampleRate => "Unsupported sample rate",
            ErrorCode::UnsupportedFormat => "Unsupported audio format",
            ErrorCode::DacOpenFailed => "DAC open failed",
            ErrorCode::BufferUnderrun => "Buffer underrun detected",
            ErrorCode::BufferOverrun => "Buffer overrun detected",
            ErrorCode::CrcVerificationFailed => "CRC verification failed",
            ErrorCode::DriftTooHigh => "Clock drift too high",
            ErrorCode::PllUnlock => "PLL unlock detected",
            ErrorCode::TimestampDiscontinuity => "Timestamp discontinuity",
            ErrorCode::EqApplicationFailed => "EQ application failed",
            ErrorCode::ConvolutionFailed => "Convolution failed",
            ErrorCode::InsufficientCpu => "Insufficient CPU for DSP processing",
            ErrorCode::ProfileHashMismatch => "DSP profile hash mismatch",
            ErrorCode::HardwareVolumeUnavailable => "Hardware volume control unavailable",
            ErrorCode::VolumeOutOfRange => "Volume level out of range",
        }
    }
}

/// Error message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// Error code
    pub code: String,
    /// Category
    pub category: String,
    /// Severity
    pub severity: ErrorSeverity,
    /// Human-readable message
    pub message: String,
    /// Additional details
    pub details: Option<ErrorDetails>,
    /// Recovery action suggestion
    pub recovery_action: Option<String>,
}

/// Error details structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// Additional context information
    pub context: Option<String>,
    /// Timestamp
    pub timestamp_us: Option<u64>,
    /// Related resource identifier
    pub resource_id: Option<String>,
    /// Error-specific fields
    pub fields: Option<serde_json::Value>,
}

/// Error handler for managing error reporting and recovery
pub struct ErrorHandler {
    /// Last reported error
    pub last_error: Option<ErrorMessage>,
    /// Error counter
    pub error_count: u64,
}

impl ErrorHandler {
    /// Create a new error handler
    pub fn new() -> Self {
        Self {
            last_error: None,
            error_count: 0,
        }
    }

    /// Report an error
    pub fn report_error(&mut self, error_code: ErrorCode, details: Option<ErrorDetails>) -> ErrorMessage {
        let error_message = ErrorMessage {
            code: error_code.code().to_string(),
            category: error_code.category().to_string(),
            severity: error_code.severity(),
            message: error_code.message().to_string(),
            details,
            recovery_action: self.get_recovery_action(error_code),
        };

        self.last_error = Some(error_message.clone());
        self.error_count += 1;

        error_message
    }

    /// Get recovery action suggestion
    fn get_recovery_action(&self, error_code: ErrorCode) -> Option<String> {
        match error_code {
            ErrorCode::ConnectionUnreachable => Some("retry_connection".to_string()),
            ErrorCode::ConnectionTimeout => Some("increase_timeout".to_string()),
            ErrorCode::ConnectionRefused => Some("check_server_status".to_string()),
            ErrorCode::WebSocketError => Some("restart_websocket".to_string()),
            ErrorCode::RtpPortBindFailed => Some("change_port".to_string()),
            ErrorCode::VersionMismatch => Some("upgrade_protocol".to_string()),
            ErrorCode::InvalidSessionInit => Some("retry_session".to_string()),
            ErrorCode::InvalidMessageFormat => Some("validate_message".to_string()),
            ErrorCode::UnsupportedFeature => Some("disable_feature".to_string()),
            ErrorCode::SsrcConflict => Some("regenerate_ssrc".to_string()),
            ErrorCode::UnsupportedSampleRate => Some("change_sample_rate".to_string()),
            ErrorCode::UnsupportedFormat => Some("change_format".to_string()),
            ErrorCode::DacOpenFailed => Some("check_hardware".to_string()),
            ErrorCode::BufferUnderrun => Some("increase_buffer".to_string()),
            ErrorCode::BufferOverrun => Some("decrease_latency".to_string()),
            ErrorCode::CrcVerificationFailed => Some("check_network".to_string()),
            ErrorCode::DriftTooHigh => Some("adjust_clock".to_string()),
            ErrorCode::PllUnlock => Some("reset_pll".to_string()),
            ErrorCode::TimestampDiscontinuity => Some("reset_timestamps".to_string()),
            ErrorCode::EqApplicationFailed => Some("retry_eq".to_string()),
            ErrorCode::ConvolutionFailed => Some("retry_convolution".to_string()),
            ErrorCode::InsufficientCpu => Some("reduce_load".to_string()),
            ErrorCode::ProfileHashMismatch => Some("resync_profile".to_string()),
            ErrorCode::HardwareVolumeUnavailable => Some("fallback_to_software".to_string()),
            ErrorCode::VolumeOutOfRange => Some("clamp_volume".to_string()),
        }
    }

    /// Check if error is fatal
    pub fn is_fatal(&self, error_code: ErrorCode) -> bool {
        matches!(
            error_code,
            ErrorCode::ConnectionUnreachable
            | ErrorCode::ConnectionRefused
            | ErrorCode::WebSocketError
            | ErrorCode::RtpPortBindFailed
            | ErrorCode::VersionMismatch
            | ErrorCode::InvalidSessionInit
            | ErrorCode::UnsupportedSampleRate
            | ErrorCode::UnsupportedFormat
            | ErrorCode::DacOpenFailed
            | ErrorCode::DriftTooHigh
            | ErrorCode::PllUnlock
            | ErrorCode::EqApplicationFailed
            | ErrorCode::ConvolutionFailed
            | ErrorCode::InsufficientCpu
            | ErrorCode::ProfileHashMismatch
            | ErrorCode::HardwareVolumeUnavailable
        )
    }

    /// Get last reported error
    pub fn get_last_error(&self) -> Option<&ErrorMessage> {
        self.last_error.as_ref()
    }

    /// Get error count
    pub fn get_error_count(&self) -> u64 {
        self.error_count
    }

    /// Reset error counter
    pub fn reset_error_count(&mut self) {
        self.error_count = 0;
    }
}

/// Error recovery protocols
pub struct ErrorRecoveryProtocols;

impl ErrorRecoveryProtocols {
    /// Handle network interruption
    pub fn handle_network_interruption(
        &self,
        buffer_remaining_ms: u32,
        reconnect_attempts: u32,
    ) -> RecoveryAction {
        if buffer_remaining_ms > 0 && reconnect_attempts < 10 {
            RecoveryAction::RetryConnection
        } else {
            RecoveryAction::FailSession
        }
    }

    /// Handle buffer underrun
    pub fn handle_buffer_underrun(
        &self,
        xrun_count: u64,
        buffer_size: u32,
    ) -> RecoveryAction {
        if xrun_count > 5 {
            RecoveryAction::IncreaseBufferSize
        } else if buffer_size > 50 {
            RecoveryAction::DecreaseBufferSize
        } else {
            RecoveryAction::ContinuePlayback
        }
    }

    /// Handle CRC failure
    pub fn handle_crc_failure(
        &self,
        failure_rate: f64,
        packet_count: u64,
    ) -> RecoveryAction {
        if failure_rate > 0.01 && packet_count > 1000 {
            RecoveryAction::ReduceBitrate
        } else {
            RecoveryAction::ContinueMonitoring
        }
    }
}

/// Recovery actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Retry connection
    RetryConnection,
    /// Fail session
    FailSession,
    /// Increase buffer size
    IncreaseBufferSize,
    /// Decrease buffer size
    DecreaseBufferSize,
    /// Continue playback
    ContinuePlayback,
    /// Reduce bitrate
    ReduceBitrate,
    /// Continue monitoring
    ContinueMonitoring,
    /// Reset PLL
    ResetPll,
    /// Regenerate SSRC
    RegenerateSsrc,
}

/// Error state machine for tracking error conditions
pub struct ErrorStateMachine {
    /// Current error state
    pub current_state: ErrorState,
    /// Error history
    pub error_history: Vec<(ErrorCode, u64)>,
}

/// Error states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorState {
    /// Normal operation
    Normal,
    /// Warning condition
    Warning,
    /// Error condition
    Error,
    /// Critical error
    Critical,
}

impl ErrorStateMachine {
    /// Create a new error state machine
    pub fn new() -> Self {
        Self {
            current_state: ErrorState::Normal,
            error_history: Vec::new(),
        }
    }

    /// Transition to error state
    pub fn transition_to_error(&mut self, error_code: ErrorCode) {
        let timestamp = Self::get_current_timestamp();
        self.error_history.push((error_code, timestamp));
        
        self.current_state = match error_code.severity() {
            ErrorSeverity::Fatal => ErrorState::Critical,
            ErrorSeverity::Warning => ErrorState::Error,
            ErrorSeverity::Info => ErrorState::Warning,
        };
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
    pub fn get_state(&self) -> ErrorState {
        self.current_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_properties() {
        let error_code = ErrorCode::ConnectionTimeout;
        assert_eq!(error_code.code(), "E102");
        assert_eq!(error_code.category(), "connection");
        assert_eq!(error_code.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_error_severity() {
        let fatal_error = ErrorCode::ConnectionUnreachable;
        let warning_error = ErrorCode::ConnectionTimeout;
        let info_error = ErrorCode::HardwareVolumeUnavailable;
        
        assert_eq!(fatal_error.severity(), ErrorSeverity::Fatal);
        assert_eq!(warning_error.severity(), ErrorSeverity::Warning);
        assert_eq!(info_error.severity(), ErrorSeverity::Info);
    }

    #[test]
    fn test_error_handler() {
        let mut handler = ErrorHandler::new();
        
        let error = handler.report_error(
            ErrorCode::BufferUnderrun, 
            Some(ErrorDetails {
                context: Some("Audio playback".to_string()),
                timestamp_us: Some(1234567890123456),
                resource_id: Some("session-123".to_string()),
                fields: None,
            })
        );
        
        assert_eq!(error.code, "E304");
        assert_eq!(error.category, "audio");
        assert_eq!(error.severity, ErrorSeverity::Warning);
        assert_eq!(error.message, "Buffer underrun detected");
        assert!(error.recovery_action.is_some());
    }
}