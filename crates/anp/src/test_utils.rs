//! Test utilities for AANP protocol implementation

use crate::{
    SessionInit, SessionAccept, NodeCapabilities, CpuInfo, DspCapabilities,
    HealthMessage, ConnectionHealth, PlaybackHealth, LatencyHealth,
    ClockHealth, IntegrityHealth, ErrorHealth, VolumeHealth, DspHealth,
    ErrorCode, ErrorSeverity, Feature, FeatureSet, RtpPayloadType,
    RtpHeader, RtpPacket, GaplessExtension, Crc32Extension,
    VolumeSet, VolumeResult, ErrorMessage, ErrorDetails,
};

/// Create a test session init message
pub fn create_test_session_init() -> SessionInit {
    SessionInit {
        protocol_version: "0.4".to_string(),
        node_uuid: uuid::Uuid::new_v4(),
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
            hardware: "Raspberry Pi 4 Model B".to_string(),
            dac_name: "HiFiBerry DAC+ Pro".to_string(),
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
    }
}

/// Create a test session accept message
pub fn create_test_session_accept() -> SessionAccept {
    SessionAccept {
        protocol_version: "0.4".to_string(),
        session_id: "srv-test-123456".to_string(),
        active_features: vec![
            "micro_pll".to_string(),
            "crc_verify".to_string(),
            "volume_control".to_string(),
            "gapless".to_string(),
        ],
        optional_features: vec![
            "dsp_transfer".to_string(),
        ],
        rtp_config: crate::RtpConfig {
            ssrc: 0x12345678,
            payload_type: 96,
            timestamp_rate: 48000,
            initial_sequence: 0,
            initial_timestamp: 0,
        },
        rtp_extensions: crate::RtpExtensions {
            gapless: crate::GaplessExtension {
                enabled: true,
                extension_id: 1,
            },
            crc32: crate::Crc32Extension {
                enabled: true,
                extension_id: 2,
                window: 64,
            },
        },
        recommended_config: crate::RecommendedConfig {
            sample_rate: 48000,
            format: "S24LE".to_string(),
            buffer_ms: 150,
            reason: "Optimal for your hardware and network".to_string(),
        },
        latency: crate::LatencyInfo {
            dac_ms: 1.34,
            pipeline_ms: 0.62,
            comp_mode: "exact".to_string(),
        },
        micro_pll: crate::MicroPllConfig {
            enabled: true,
            ppm_limit: 150,
            adjustment_interval_ms: 100,
            slew_rate_ppm_per_sec: 10,
            ema_window: 8,
        },
        volume: crate::VolumeConfig {
            initial_level: 0.75,
            mute: false,
            control_mode: "software".to_string(),
            curve_type: "logarithmic".to_string(),
        },
        buffer: crate::BufferConfig {
            target_ms: 150,
            min_ms: 50,
            max_ms: 500,
            start_threshold_ms: 100,
        },
    }
}

/// Create a test health message
pub fn create_test_health_message() -> HealthMessage {
    HealthMessage {
        timestamp_us: 1234567890123456,
        connection: ConnectionHealth {
            state: "connected".to_string(),
            uptime_seconds: 3600,
            packets_received: 172800,
            packets_lost: 3,
            bytes_received: 497664000,
        },
        playback: PlaybackHealth {
            state: "playing".to_string(),
            buffer_ms: 140.1,
            buffer_health: "good".to_string(),
            buffer_fill_percent: 93,
        },
        latency: LatencyHealth {
            network_ms: 5.2,
            jitter_buffer_ms: 140.1,
            dac_ms: 1.34,
            pipeline_ms: 0.62,
            total_ms: 147.26,
        },
        clock_sync: ClockHealth {
            drift_ppm: 1.2,
            phase_us: 3.8,
            pll_state: "locked".to_string(),
            adjustment_ppm: 1.5,
        },
        integrity: IntegrityHealth {
            crc_ok: 2700,
            crc_fail: 0,
            last_crc_fail_seq: None,
        },
        errors: ErrorHealth {
            xruns: 0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            last_xrun_timestamp_us: None,
        },
        volume: VolumeHealth {
            level: 0.75,
            mute: false,
            hardware_control: true,
            gain_db: -5.1,
        },
        dsp: DspHealth {
            current_profile_hash: 12345678,
            eq_active: true,
            convolution_active: false,
        },
    }
}

/// Create a test error message
pub fn create_test_error_message() -> ErrorMessage {
    ErrorMessage {
        code: "E304".to_string(),
        category: "audio".to_string(),
        severity: ErrorSeverity::Warning,
        message: "Buffer underrun detected".to_string(),
        details: Some(ErrorDetails {
            context: Some("Audio playback".to_string()),
            timestamp_us: Some(1234567890123456),
            resource_id: Some("session-123".to_string()),
            fields: None,
        }),
        recovery_action: Some("increase_buffer".to_string()),
    }
}

/// Create a test feature set
pub fn create_test_feature_set() -> FeatureSet {
    let mut features = FeatureSet::new();
    features.add_supported(Feature::MicroPll);
    features.add_supported(Feature::CrcVerify);
    features.add_supported(Feature::VolumeControl);
    features.add_supported(Feature::Gapless);
    features.add_optional(Feature::DspTransfer);
    features
}

/// Create a test RTP header
pub fn create_test_rtp_header() -> RtpHeader {
    RtpHeader::new(96, 1234, 56789, 0x12345678)
}

/// Create a test RTP packet
pub fn create_test_rtp_packet() -> RtpPacket {
    let header = create_test_rtp_header();
    RtpPacket {
        header,
        payload: vec![0; 100],
        extensions: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_init_creation() {
        let init = create_test_session_init();
        assert_eq!(init.protocol_version, "0.4");
        assert!(!init.node_uuid.is_nil());
    }

    #[test]
    fn test_session_accept_creation() {
        let accept = create_test_session_accept();
        assert_eq!(accept.protocol_version, "0.4");
        assert_eq!(accept.session_id, "srv-test-123456");
    }

    #[test]
    fn test_health_message_creation() {
        let health = create_test_health_message();
        assert_eq!(health.connection.state, "connected");
        assert_eq!(health.playback.state, "playing");
    }

    #[test]
    fn test_error_message_creation() {
        let error = create_test_error_message();
        assert_eq!(error.code, "E304");
        assert_eq!(error.category, "audio");
        assert_eq!(error.severity, ErrorSeverity::Warning);
    }

    #[test]
    fn test_feature_set_creation() {
        let features = create_test_feature_set();
        assert!(features.is_supported(Feature::MicroPll));
        assert!(features.is_optional(Feature::DspTransfer));
    }

    #[test]
    fn test_rtp_header_creation() {
        let header = create_test_rtp_header();
        assert_eq!(header.version, 2);
        assert_eq!(header.payload_type, 96);
    }
}