//! Core protocol definitions and constants for AANP v0.4

/// Protocol version string
pub const PROTOCOL_VERSION: &str = "0.4";

/// RTP payload types as defined in the specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpPayloadType {
    /// 24-bit Linear PCM (L24)
    L24,
    /// 16-bit Linear PCM (L16)
    L16,
    /// Standard 16-bit Linear PCM (PT 10)
    L16Standard,
    /// Standard 16-bit Linear PCM Stereo (PT 11)
    L16Stereo,
}

impl RtpPayloadType {
    /// Get the RTP payload type number
    pub fn payload_type(&self) -> u8 {
        match self {
            RtpPayloadType::L24 => 96,
            RtpPayloadType::L16 => 97,
            RtpPayloadType::L16Standard => 10,
            RtpPayloadType::L16Stereo => 11,
        }
    }

    /// Get the format name
    pub fn format_name(&self) -> &'static str {
        match self {
            RtpPayloadType::L24 => "L24",
            RtpPayloadType::L16 => "L16",
            RtpPayloadType::L16Standard => "L16 (standard)",
            RtpPayloadType::L16Stereo => "L16 Stereo (standard)",
        }
    }
}

/// Volume curve types supported by the protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeCurve {
    /// Linear volume curve
    Linear,
    /// Logarithmic volume curve (recommended)
    Logarithmic,
    /// Exponential volume curve
    Exponential,
}

impl VolumeCurve {
    /// Get the curve type name
    pub fn name(&self) -> &'static str {
        match self {
            VolumeCurve::Linear => "linear",
            VolumeCurve::Logarithmic => "logarithmic",
            VolumeCurve::Exponential => "exponential",
        }
    }
}

/// Session states as defined in the specification
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

impl SessionState {
    /// Get the state name
    pub fn name(&self) -> &'static str {
        match self {
            SessionState::Disconnected => "disconnected",
            SessionState::Idle => "idle",
            SessionState::Negotiating => "negotiating",
            SessionState::Buffering => "buffering",
            SessionState::Playing => "playing",
            SessionState::Paused => "paused",
            SessionState::Error => "error",
        }
    }
}

/// Error codes as defined in the specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

// Feature enum is now defined in features.rs to avoid duplication