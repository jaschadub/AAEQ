use serde::{Deserialize, Serialize};

/// Interleaved stereo 64-bit float frames from DSP
#[derive(Clone, Copy, Debug)]
pub struct AudioBlock<'a> {
    /// Interleaved audio frames (len = n_frames * n_channels)
    pub frames: &'a [f64],
    /// Sample rate in Hz (e.g., 44100, 48000)
    pub sample_rate: u32,
    /// Number of channels (typically 2 for stereo)
    pub channels: u16,
}

impl<'a> AudioBlock<'a> {
    /// Create a new AudioBlock
    pub fn new(frames: &'a [f64], sample_rate: u32, channels: u16) -> Self {
        Self {
            frames,
            sample_rate,
            channels,
        }
    }

    /// Get the number of frames in this block
    pub fn num_frames(&self) -> usize {
        self.frames.len() / self.channels as usize
    }

    /// Validate that the frame count is consistent with channels
    pub fn is_valid(&self) -> bool {
        self.frames.len() % self.channels as usize == 0
    }
}

/// Sample format for output conversion
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleFormat {
    /// 64-bit float (native DSP format)
    F64,
    /// 32-bit float
    F32,
    /// 24-bit signed integer (little-endian)
    S24LE,
    /// 16-bit signed integer (little-endian)
    S16LE,
}

impl SampleFormat {
    /// Get the size in bytes per sample
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            SampleFormat::F64 => 8,
            SampleFormat::F32 => 4,
            SampleFormat::S24LE => 3,
            SampleFormat::S16LE => 2,
        }
    }

    /// Get the bit depth
    pub fn bit_depth(&self) -> u8 {
        match self {
            SampleFormat::F64 => 64,
            SampleFormat::F32 => 32,
            SampleFormat::S24LE => 24,
            SampleFormat::S16LE => 16,
        }
    }

    /// Check if this is a floating-point format
    pub fn is_float(&self) -> bool {
        matches!(self, SampleFormat::F64 | SampleFormat::F32)
    }
}

/// Configuration for an output sink
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Target sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels (typically 2)
    pub channels: u16,
    /// Output sample format
    pub format: SampleFormat,
    /// Buffer size in milliseconds (for network jitter)
    pub buffer_ms: u32,
    /// Use exclusive mode if available (WASAPI/CoreAudio)
    pub exclusive: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            format: SampleFormat::S24LE,
            buffer_ms: 150,
            exclusive: false,
        }
    }
}

impl OutputConfig {
    /// Calculate buffer size in frames
    pub fn buffer_frames(&self) -> usize {
        (self.sample_rate as u64 * self.buffer_ms as u64 / 1000) as usize
    }

    /// Calculate buffer size in bytes
    pub fn buffer_bytes(&self) -> usize {
        self.buffer_frames() * self.channels as usize * self.format.bytes_per_sample()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_block_validation() {
        let frames = vec![0.0; 480]; // 240 frames of stereo
        let block = AudioBlock::new(&frames, 48000, 2);

        assert!(block.is_valid());
        assert_eq!(block.num_frames(), 240);
    }

    #[test]
    fn test_audio_block_invalid() {
        let frames = vec![0.0; 481]; // Invalid: not divisible by 2
        let block = AudioBlock::new(&frames, 48000, 2);

        assert!(!block.is_valid());
    }

    #[test]
    fn test_sample_format_sizes() {
        assert_eq!(SampleFormat::F64.bytes_per_sample(), 8);
        assert_eq!(SampleFormat::F32.bytes_per_sample(), 4);
        assert_eq!(SampleFormat::S24LE.bytes_per_sample(), 3);
        assert_eq!(SampleFormat::S16LE.bytes_per_sample(), 2);
    }

    #[test]
    fn test_sample_format_bit_depth() {
        assert_eq!(SampleFormat::F64.bit_depth(), 64);
        assert_eq!(SampleFormat::S24LE.bit_depth(), 24);
    }

    #[test]
    fn test_output_config_buffer_calculation() {
        let cfg = OutputConfig {
            sample_rate: 48000,
            channels: 2,
            format: SampleFormat::S24LE,
            buffer_ms: 100,
            exclusive: false,
        };

        assert_eq!(cfg.buffer_frames(), 4800); // 48000 * 100 / 1000
        assert_eq!(cfg.buffer_bytes(), 28800); // 4800 * 2 * 3
    }
}
