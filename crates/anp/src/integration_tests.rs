//! Integration tests for AANP protocol implementation
//!
//! These tests verify the end-to-end functionality of the AANP protocol implementation.

#[cfg(test)]
mod integration_tests {
    use crate::*;
    use tokio::time::{timeout, Duration};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_complete_session_lifecycle() {
        // Test full session initialization and acceptance
        let mut session_manager = SessionManager::new();
        
        // Create session init
        let session_init = SessionInit {
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

        // Initialize session
        let session_accept = session_manager.initialize_session(&session_init);
        assert!(session_accept.is_ok());
        
        // Verify session state
        assert_eq!(session_manager.get_state(), SessionState::Buffering);
        assert!(session_manager.get_session_id().is_some());
        assert!(session_manager.get_node_uuid().is_some());
    }

    #[tokio::test]
    async fn test_rtp_packet_handling() {
        // Test RTP packet creation and validation
        let mut stream_manager = RtpStreamManager::new(0x12345678, 48000, RtpPayloadType::L24);
        
        // Create test packet
        let payload = vec![0x00; 100]; // 100 bytes of audio data
        let packet = stream_manager.create_next_packet(payload);
        
        // Verify packet structure
        assert_eq!(packet.header.payload_type, 96); // L24 format
        assert_eq!(packet.header.ssrc, 0x12345678);
        assert_eq!(stream_manager.get_sequence_number(), 1);
        
        // Test packet serialization
        let bytes = packet.header.to_bytes();
        assert_eq!(bytes.len(), 12);
        
        // Test deserialization
        let parsed = RtpHeader::from_bytes(&bytes);
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.payload_type, 96);
    }

    #[tokio::test]
    async fn test_websocket_message_handling() {
        // Test WebSocket message serialization
        let volume_set = VolumeSet {
            level: 0.75,
            mute: false,
            ramp_ms: Some(100),
            ramp_shape: Some("s_curve".to_string()),
        };

        let json = serde_json::to_string(&volume_set).unwrap();
        assert!(json.contains("\"level\""));
        assert!(json.contains("\"mute\""));
        assert!(json.contains("\"ramp_ms\""));
        assert!(json.contains("\"ramp_shape\""));
    }

    #[tokio::test]
    async fn test_health_telemetry() {
        // Test health message generation
        let mut health_manager = HealthManager::new();
        
        // Update various health metrics
        health_manager.update_connection_metrics(
            "connected",
            3600,
            172800,
            3,
            497664000,
        );
        
        health_manager.update_playback_metrics(
            "playing",
            140.1,
            "good",
            93,
        );
        
        let health_message = health_manager.get_health_message();
        
        // Verify all fields are populated
        assert!(health_message.timestamp_us > 0);
        assert_eq!(health_message.connection.state, "connected");
        assert_eq!(health_message.playback.state, "playing");
        assert_eq!(health_message.playback.buffer_ms, 140.1);
        assert_eq!(health_message.timestamp_us, health_manager.timestamp);
    }

    #[tokio::test]
    async fn test_error_handling() {
        // Test error reporting and recovery
        let mut error_handler = ErrorHandler::new();
        
        // Report various errors
        let error = error_handler.report_error(
            ErrorCode::BufferUnderrun,
            Some(ErrorDetails {
                context: Some("Audio playback".to_string()),
                timestamp_us: Some(1234567890123456),
                resource_id: Some("session-123".to_string()),
                fields: None,
            })
        );
        
        // Verify error properties
        assert_eq!(error.code, "E304");
        assert_eq!(error.category, "audio");
        assert_eq!(error.severity, ErrorSeverity::Warning);
        assert!(error.recovery_action.is_some());
        
        // Test fatal error detection
        let fatal_error = ErrorCode::ConnectionUnreachable;
        assert!(error_handler.is_fatal(fatal_error));
        
        // Test non-fatal error detection
        let warning_error = ErrorCode::BufferUnderrun;
        assert!(!error_handler.is_fatal(warning_error));
    }

    #[tokio::test]
    async fn test_feature_negotiation() {
        // Test feature negotiation
        let mut negotiator = FeatureNegotiator::new();
        
        // Initialize local features
        let local_features = vec![
            Feature::MicroPll,
            Feature::CrcVerify,
            Feature::VolumeControl,
            Feature::Gapless,
        ];
        negotiator.initialize_local_features(local_features);
        
        // Create remote features (subset)
        let mut remote_features = FeatureSet::new();
        remote_features.add_supported(Feature::MicroPll);
        remote_features.add_supported(Feature::CrcVerify);
        remote_features.add_optional(Feature::DspTransfer);
        
        // Process negotiation
        let negotiated = negotiator.process_remote_features(remote_features);
        
        // Verify results
        assert!(negotiated.is_supported(Feature::MicroPll));
        assert!(negotiated.is_supported(Feature::CrcVerify));
        assert!(!negotiated.is_supported(Feature::VolumeControl));
    }

    #[tokio::test]
    async fn test_discovery_integration() {
        // Test discovery record creation
        let uuid = uuid::Uuid::new_v4();
        let mut record = NodeDiscoveryRecord::new(uuid);
        
        // Set additional fields
        record.control_url = Some("wss://10.0.0.10:7443".to_string());
        record.dac_name = Some("HiFiBerry DAC+".to_string());
        record.hardware_platform = Some("RPi4".to_string());
        
        // Convert to TXT record
        let txt_record = record.to_txt_record();
        
        // Verify required fields
        assert_eq!(txt_record.get("uuid").unwrap(), &uuid.to_string());
        assert_eq!(txt_record.get("v").unwrap(), &"0.4.0".to_string());
        assert_eq!(txt_record.get("ctrl").unwrap(), &"wss://10.0.0.10:7443".to_string());
        assert_eq!(txt_record.get("dac").unwrap(), &"HiFiBerry DAC+".to_string());
        assert_eq!(txt_record.get("hw").unwrap(), &"RPi4".to_string());
        
        // Test round-trip parsing
        let parsed = NodeDiscoveryRecord::from_txt_record(&txt_record);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_protocol_compliance() {
        // Test compliance with specification requirements
        
        // Test RTP header compliance
        let header = RtpHeader::new(96, 1234, 56789, 0x12345678);
        let bytes = header.to_bytes();
        
        // Verify field values according to specification
        assert_eq!(bytes[0] & 0xC0, 0x80); // Version 2
        assert_eq!(bytes[0] & 0x20, 0x00); // Padding 0
        assert_eq!(bytes[0] & 0x10, 0x00); // Extension 0
        assert_eq!(bytes[0] & 0x0F, 0x00); // CSRC count 0
        assert_eq!(bytes[1] & 0x80, 0x00); // Marker 0
        assert_eq!(bytes[1] & 0x7F, 0x60); // Payload type 96
        
        // Test payload type constants
        assert_eq!(RtpPayloadType::L24.payload_type(), 96);
        assert_eq!(RtpPayloadType::L16.payload_type(), 97);
        
        // Test volume curve calculations
        let volume = 0.75;
        let gain_db = calculate_gain_db(volume);
        assert!(gain_db < 0.0); // Should be negative
        
        // Test UUID format compliance
        let uuid = uuid::Uuid::new_v4();
        assert!(uuid.to_string().len() >= 32); // UUID should be properly formatted
    }
}

/// Helper function for calculating gain in dB
fn calculate_gain_db(level: f32) -> f32 {
    if level == 0.0 {
        f32::NEG_INFINITY // -∞ dB
    } else {
        40.0 * level.log10() // 40 * log10(level)
    }
}