use anyhow::Result;
use tracing::warn;

/// ALAC encoder configuration
#[derive(Clone, Debug)]
pub struct AlacConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u8,
    pub frames_per_packet: u32,
}

impl Default for AlacConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            bit_depth: 16,
            frames_per_packet: 352,
        }
    }
}

/// ALAC encoder
///
/// Note: This is a simplified implementation. For production use, you should:
/// 1. Use Apple's official ALAC encoder (requires FFI to C library)
/// 2. Use FFmpeg's ALAC encoder
/// 3. Implement a pure Rust ALAC encoder (significant effort)
///
/// This implementation provides PCM passthrough with ALAC framing for testing.
pub struct AlacEncoder {
    config: AlacConfig,
    frames_buffered: Vec<i16>,
}

impl AlacEncoder {
    pub fn new(config: AlacConfig) -> Self {
        Self {
            config,
            frames_buffered: Vec::new(),
        }
    }

    /// Get the format string (fmtp) for SDP
    pub fn fmtp_string(&self) -> String {
        // ALAC fmtp format: <frames per packet> <compatible version> <bit depth>
        // <rice history mult> <rice initial history> <rice parameter limit>
        // <num channels> <max run> <max coded frame size> <average bit rate> <sample rate>
        format!(
            "{} 0 {} 40 10 14 {} 255 0 0 {}",
            self.config.frames_per_packet,
            self.config.bit_depth,
            self.config.channels,
            self.config.sample_rate
        )
    }

    /// Encode PCM samples to ALAC
    ///
    /// Input: Interleaved 16-bit PCM samples
    /// Output: ALAC-encoded packets (currently just framed PCM as a stub)
    pub fn encode(&mut self, pcm_data: &[i16]) -> Result<Vec<Vec<u8>>> {
        // Add samples to buffer
        self.frames_buffered.extend_from_slice(pcm_data);

        let mut packets = Vec::new();
        let samples_per_packet = self.config.frames_per_packet as usize * self.config.channels as usize;

        // Process complete packets
        while self.frames_buffered.len() >= samples_per_packet {
            let packet_samples: Vec<i16> = self.frames_buffered.drain(..samples_per_packet).collect();

            // Create ALAC packet
            // Note: This is a simplified stub. Real ALAC encoding involves:
            // - Adaptive Rice coding
            // - LPC prediction
            // - Frame header construction
            // - Checksum calculation

            let packet = self.create_alac_packet(&packet_samples)?;
            packets.push(packet);
        }

        Ok(packets)
    }

    /// Create an ALAC packet from PCM samples
    /// This is a stub implementation that creates a simple frame structure
    fn create_alac_packet(&self, samples: &[i16]) -> Result<Vec<u8>> {
        warn!(
            "Using stub ALAC encoder - audio will be PCM, not true ALAC compression"
        );

        // Simple packet structure:
        // - 4 bytes: packet size
        // - 1 byte: channels
        // - 2 bytes: unused
        // - 1 byte: hasSize flag
        // - 4 bytes: unused
        // - 4 bytes: frame length
        // - PCM data (big-endian 16-bit)

        let frame_length = samples.len() / self.config.channels as usize;
        let packet_size = 16 + (samples.len() * 2); // Header + data

        let mut packet = Vec::with_capacity(packet_size);

        // Packet size (big-endian)
        packet.extend_from_slice(&(packet_size as u32).to_be_bytes());

        // Channels
        packet.push(self.config.channels as u8);

        // Unused
        packet.push(0);
        packet.push(0);

        // hasSize flag
        packet.push(1);

        // Unused
        packet.extend_from_slice(&[0u8; 4]);

        // Frame length (big-endian)
        packet.extend_from_slice(&(frame_length as u32).to_be_bytes());

        // PCM data (convert to big-endian)
        for &sample in samples {
            packet.extend_from_slice(&sample.to_be_bytes());
        }

        Ok(packet)
    }

    /// Flush any remaining buffered samples
    pub fn flush(&mut self) -> Result<Vec<Vec<u8>>> {
        if self.frames_buffered.is_empty() {
            return Ok(Vec::new());
        }

        // Pad with zeros to complete packet
        let samples_per_packet = self.config.frames_per_packet as usize * self.config.channels as usize;
        while self.frames_buffered.len() < samples_per_packet {
            self.frames_buffered.push(0);
        }

        self.encode(&[])
    }
}

/// Convert f64 PCM samples to i16
pub fn f64_to_i16(samples: &[f64]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alac_config_default() {
        let config = AlacConfig::default();
        assert_eq!(config.sample_rate, 44100);
        assert_eq!(config.channels, 2);
        assert_eq!(config.bit_depth, 16);
    }

    #[test]
    fn test_fmtp_string() {
        let config = AlacConfig::default();
        let encoder = AlacEncoder::new(config);
        let fmtp = encoder.fmtp_string();

        assert!(fmtp.contains("352")); // frames per packet
        assert!(fmtp.contains("16")); // bit depth
        assert!(fmtp.contains("2")); // channels
        assert!(fmtp.contains("44100")); // sample rate
    }

    #[test]
    fn test_f64_to_i16() {
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let converted = f64_to_i16(&samples);

        assert_eq!(converted[0], 0);
        assert!((converted[1] - 16383).abs() < 10); // ~0.5 * 32767
        assert!((converted[2] + 16383).abs() < 10); // ~-0.5 * 32767
        assert_eq!(converted[3], 32767);
        assert_eq!(converted[4], -32767);
    }

    #[test]
    fn test_encode_complete_packet() {
        let config = AlacConfig {
            frames_per_packet: 10,
            ..Default::default()
        };
        let mut encoder = AlacEncoder::new(config);

        // 10 frames * 2 channels = 20 samples
        let samples = vec![0i16; 20];
        let packets = encoder.encode(&samples).unwrap();

        assert_eq!(packets.len(), 1);
        assert!(!packets[0].is_empty());
    }

    #[test]
    fn test_encode_incomplete_packet() {
        let config = AlacConfig {
            frames_per_packet: 10,
            ..Default::default()
        };
        let mut encoder = AlacEncoder::new(config);

        // Only 5 samples (incomplete packet)
        let samples = vec![0i16; 5];
        let packets = encoder.encode(&samples).unwrap();

        // Should not produce any packets yet
        assert_eq!(packets.len(), 0);
    }
}
