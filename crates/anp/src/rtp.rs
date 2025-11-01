//! RTP transport implementation for AANP protocol
//!
//! Implements RTP packet handling with proper endianness as required by the specification.

use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

/// RTP header structure as defined in the specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpHeader {
    /// Version (2 bits)
    pub version: u8,
    /// Padding flag (1 bit)
    pub padding: bool,
    /// Extension flag (1 bit)
    pub extension: bool,
    /// CSRC count (4 bits)
    pub csrc_count: u8,
    /// Marker bit (1 bit)
    pub marker: bool,
    /// Payload type (7 bits)
    pub payload_type: u8,
    /// Sequence number (16 bits)
    pub sequence_number: u16,
    /// Timestamp (32 bits)
    pub timestamp: u32,
    /// SSRC (32 bits)
    pub ssrc: u32,
}

impl RtpHeader {
    /// Create a new RTP header
    pub fn new(
        payload_type: u8,
        sequence_number: u16,
        timestamp: u32,
        ssrc: u32,
    ) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
        }
    }

    /// Pack RTP header into bytes (network byte order)
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut bytes = [0u8; 12];
        
        // Version (bits 0-1) + Padding (bit 2) + Extension (bit 3) + CSRC Count (bits 4-7)
        bytes[0] = (self.version << 6) | ((self.padding as u8) << 5) | ((self.extension as u8) << 4) | self.csrc_count;
        
        // Marker (bit 7) + Payload Type (bits 0-6)
        bytes[1] = (self.marker as u8) << 7 | (self.payload_type & 0x7F);
        
        // Sequence number (16 bits)
        bytes[2..4].copy_from_slice(&self.sequence_number.to_be_bytes());
        
        // Timestamp (32 bits)
        bytes[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        
        // SSRC (32 bits)
        bytes[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
        
        bytes
    }

    /// Parse RTP header from bytes
    pub fn from_bytes(bytes: &[u8; 12]) -> Self {
        let version = (bytes[0] >> 6) & 0x03;
        let padding = (bytes[0] >> 5) & 0x01 != 0;
        let extension = (bytes[0] >> 4) & 0x01 != 0;
        let csrc_count = bytes[0] & 0x0F;
        
        let marker = (bytes[1] >> 7) & 0x01 != 0;
        let payload_type = bytes[1] & 0x7F;
        
        let sequence_number = u16::from_be_bytes([bytes[2], bytes[3]]);
        let timestamp = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let ssrc = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        
        Self {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
        }
    }
}

/// RTP payload types as defined in the specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpPayloadType {
    /// 24-bit Linear PCM (L24) - network byte order
    L24,
    /// 16-bit Linear PCM (L16) - network byte order
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

/// RTP packet structure
#[derive(Debug, Clone)]
pub struct RtpPacket {
    /// RTP header
    pub header: RtpHeader,
    /// RTP payload (audio data)
    pub payload: Vec<u8>,
    /// Extension data (if present)
    pub extensions: Option<RtpExtensions>,
}

/// RTP extensions as defined in the specification
#[derive(Debug, Clone)]
pub struct RtpExtensions {
    /// Gapless playback extension
    pub gapless: Option<GaplessExtension>,
    /// CRC32 extension
    pub crc32: Option<Crc32Extension>,
}

/// Gapless playback extension data
#[derive(Debug, Clone)]
pub struct GaplessExtension {
    /// Extension ID
    pub id: u8,
    /// Track end flag
    pub track_end: bool,
    /// Track start flag
    pub track_start: bool,
    /// Reserved bits
    pub reserved: u8,
}

impl GaplessExtension {
    /// Create a new gapless extension
    pub fn new(id: u8, track_end: bool, track_start: bool) -> Self {
        Self {
            id,
            track_end,
            track_start,
            reserved: 0,
        }
    }

    /// Pack gapless extension into bytes
    pub fn to_bytes(&self) -> [u8; 1] {
        let mut bytes = [0u8; 1];
        bytes[0] = (self.id << 4) | 
                  ((self.track_end as u8) << 3) | 
                  ((self.track_start as u8) << 2) | 
                  (self.reserved & 0x03);
        bytes
    }

    /// Parse gapless extension from bytes
    pub fn from_bytes(bytes: &[u8; 1]) -> Self {
        let id = (bytes[0] >> 4) & 0x0F;
        let track_end = (bytes[0] >> 3) & 0x01 != 0;
        let track_start = (bytes[0] >> 2) & 0x01 != 0;
        let reserved = bytes[0] & 0x03;
        
        Self {
            id,
            track_end,
            track_start,
            reserved,
        }
    }
}

/// CRC32 extension data
#[derive(Debug, Clone)]
pub struct Crc32Extension {
    /// Extension ID
    pub id: u8,
    /// CRC32 value
    pub crc32: u32,
}

impl Crc32Extension {
    /// Create a new CRC32 extension
    pub fn new(id: u8, crc32: u32) -> Self {
        Self { id, crc32 }
    }

    /// Pack CRC32 extension into bytes
    pub fn to_bytes(&self) -> [u8; 4] {
        self.crc32.to_be_bytes()
    }

    /// Parse CRC32 extension from bytes
    pub fn from_bytes(bytes: &[u8; 4]) -> Self {
        Self {
            id: 0, // Would be set from the extension header
            crc32: u32::from_be_bytes(*bytes),
        }
    }
}

/// RTP packet builder
pub struct RtpPacketBuilder {
    /// Payload type
    payload_type: RtpPayloadType,
    /// Sequence number
    sequence_number: u16,
    /// Timestamp
    timestamp: u32,
    /// SSRC
    ssrc: u32,
    /// Payload data
    payload: Vec<u8>,
    /// Extension data
    extensions: Option<RtpExtensions>,
}

impl RtpPacketBuilder {
    /// Create a new packet builder
    pub fn new(payload_type: RtpPayloadType, ssrc: u32) -> Self {
        Self {
            payload_type,
            sequence_number: 0,
            timestamp: 0,
            ssrc,
            payload: Vec::new(),
            extensions: None,
        }
    }

    /// Set sequence number
    pub fn sequence_number(mut self, seq: u16) -> Self {
        self.sequence_number = seq;
        self
    }

    /// Set timestamp
    pub fn timestamp(mut self, ts: u32) -> Self {
        self.timestamp = ts;
        self
    }

    /// Set payload data
    pub fn payload(mut self, data: Vec<u8>) -> Self {
        self.payload = data;
        self
    }

    /// Set extensions
    pub fn extensions(mut self, ext: RtpExtensions) -> Self {
        self.extensions = Some(ext);
        self
    }

    /// Build the RTP packet
    pub fn build(self) -> RtpPacket {
        let header = RtpHeader::new(
            self.payload_type.payload_type(),
            self.sequence_number,
            self.timestamp,
            self.ssrc,
        );
        
        RtpPacket {
            header,
            payload: self.payload,
            extensions: self.extensions,
        }
    }
}

/// RTP payload handling utilities
pub struct RtpPayloadUtils;

impl RtpPayloadUtils {
    /// Pack S24LE sample to network byte order (as required by spec)
    pub fn pack_s24le_sample(sample: i32) -> [u8; 3] {
        // Clamp to 24-bit range
        let clamped = sample.clamp(-8388608, 8388607);
        let bytes = clamped.to_be_bytes(); // Big-endian (network order)
        [bytes[1], bytes[2], bytes[3]] // Take lower 3 bytes
    }

    /// Unpack S24LE sample from network byte order (as required by spec)
    pub fn unpack_s24le_sample(bytes: &[u8; 3]) -> i32 {
        let sign_extended = if bytes[0] & 0x80 != 0 { 0xFF } else { 0x00 };
        i32::from_be_bytes([sign_extended, bytes[0], bytes[1], bytes[2]])
    }

    /// Calculate frame count from payload size
    pub fn calculate_frames_from_payload(
        payload_bytes: usize,
        channels: u8,
        bytes_per_sample: u8,
    ) -> u32 {
        (payload_bytes as u32) / ((channels as u32) * (bytes_per_sample as u32))
    }

    /// Calculate timestamp increment
    pub fn calculate_timestamp_increment(
        frames_in_packet: u32,
        timestamp_rate: u32,
    ) -> u32 {
        // For audio, timestamp increments by number of frames
        frames_in_packet
    }
}

/// RTP stream manager
pub struct RtpStreamManager {
    /// Current sequence number
    sequence_number: u16,
    /// Current timestamp
    timestamp: u32,
    /// SSRC
    ssrc: u32,
    /// Timestamp rate
    timestamp_rate: u32,
    /// Payload type
    payload_type: RtpPayloadType,
    /// Buffer for outgoing packets
    packet_buffer: VecDeque<RtpPacket>,
}

impl RtpStreamManager {
    /// Create a new RTP stream manager
    pub fn new(ssrc: u32, timestamp_rate: u32, payload_type: RtpPayloadType) -> Self {
        Self {
            sequence_number: 0,
            timestamp: 0,
            ssrc,
            timestamp_rate,
            payload_type,
            packet_buffer: VecDeque::new(),
        }
    }

    /// Create next RTP packet
    pub fn create_next_packet(&mut self, payload: Vec<u8>) -> RtpPacket {
        let frames_in_packet = RtpPayloadUtils::calculate_frames_from_payload(
            payload.len(),
            2, // stereo
            3, // 24-bit samples
        );
        
        let timestamp_increment = RtpPayloadUtils::calculate_timestamp_increment(
            frames_in_packet,
            self.timestamp_rate,
        );
        
        let packet = RtpPacketBuilder::new(self.payload_type, self.ssrc)
            .sequence_number(self.sequence_number)
            .timestamp(self.timestamp)
            .payload(payload)
            .build();
            
        // Update state
        self.sequence_number = self.sequence_number.wrapping_add(1);
        self.timestamp += timestamp_increment;
        
        packet
    }

    /// Get current sequence number
    pub fn get_sequence_number(&self) -> u16 {
        self.sequence_number
    }

    /// Get current timestamp
    pub fn get_timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Reset stream state
    pub fn reset(&mut self) {
        self.sequence_number = 0;
        self.timestamp = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtp_header_serialization() {
        let header = RtpHeader::new(96, 1234, 56789, 0x12345678);
        let bytes = header.to_bytes();
        
        // Verify basic structure
        assert_eq!(bytes[0], 0x80); // Version 2, padding 0, extension 0, csrc 0
        assert_eq!(bytes[1], 0x60); // Marker 0, payload type 96
        
        // Verify sequence number
        assert_eq!(u16::from_be_bytes([bytes[2], bytes[3]]), 1234);
        
        // Verify timestamp
        assert_eq!(u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]), 56789);
        
        // Verify SSRC
        assert_eq!(u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]), 0x12345678);
    }

    #[test]
    fn test_rtp_header_deserialization() {
        let original = RtpHeader::new(96, 1234, 56789, 0x12345678);
        let bytes = original.to_bytes();
        let parsed = RtpHeader::from_bytes(&bytes);
        
        assert_eq!(original.version, parsed.version);
        assert_eq!(original.padding, parsed.padding);
        assert_eq!(original.extension, parsed.extension);
        assert_eq!(original.csrc_count, parsed.csrc_count);
        assert_eq!(original.marker, parsed.marker);
        assert_eq!(original.payload_type, parsed.payload_type);
        assert_eq!(original.sequence_number, parsed.sequence_number);
        assert_eq!(original.timestamp, parsed.timestamp);
        assert_eq!(original.ssrc, parsed.ssrc);
    }

    #[test]
    fn test_s24le_packing() {
        // Test normal case
        let sample = 123456;
        let packed = RtpPayloadUtils::pack_s24le_sample(sample);
        let unpacked = RtpPayloadUtils::unpack_s24le_sample(&packed);
        assert_eq!(sample, unpacked);
        
        // Test clamping
        let sample = 10000000; // Too large
        let packed = RtpPayloadUtils::pack_s24le_sample(sample);
        let unpacked = RtpPayloadUtils::unpack_s24le_sample(&packed);
        assert_eq!(unpacked, 8388607); // Max 24-bit value
        
        let sample = -10000000; // Too small
        let packed = RtpPayloadUtils::pack_s24le_sample(sample);
        let unpacked = RtpPayloadUtils::unpack_s24le_sample(&packed);
        assert_eq!(unpacked, -8388608); // Min 24-bit value
    }

    #[test]
    fn test_rtp_stream_manager() {
        let mut manager = RtpStreamManager::new(0x12345678, 48000, RtpPayloadType::L24);

        // For 100 bytes payload: 100 / (2 channels * 3 bytes_per_sample) = 16 frames
        let packet1 = manager.create_next_packet(vec![0; 100]);
        let packet2 = manager.create_next_packet(vec![0; 100]);

        assert_eq!(packet1.header.sequence_number, 0);
        assert_eq!(packet2.header.sequence_number, 1);
        assert_eq!(packet1.header.timestamp, 0);
        assert_eq!(packet2.header.timestamp, 16); // Incremented by 16 frames
    }
}