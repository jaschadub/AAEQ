//! High-quality sample rate conversion using sinc interpolation
//!
//! Implements professional-quality resampling with multiple quality presets
//! for converting between different sample rates (e.g., 44.1kHz to 48kHz,
//! or 96kHz to 48kHz). Uses the rubato library for sinc-based resampling.

use anyhow::{Context, Result};
use rubato::{
    Resampler as RubatoResampler, SincFixedIn, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use serde::{Deserialize, Serialize};

/// Resampling quality preset
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResamplerQuality {
    /// Fast resampling - lower quality, minimal CPU usage
    /// Good for real-time monitoring or less critical applications
    Fast,
    /// Balanced resampling - good quality, moderate CPU usage
    /// Recommended for most use cases
    Balanced,
    /// High quality - excellent quality, higher CPU usage
    /// For audiophile-grade resampling
    High,
    /// Ultra quality - maximum quality, highest CPU usage
    /// For mastering and archival purposes
    Ultra,
}

impl ResamplerQuality {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResamplerQuality::Fast => "Fast",
            ResamplerQuality::Balanced => "Balanced",
            ResamplerQuality::High => "High",
            ResamplerQuality::Ultra => "Ultra",
        }
    }

    /// Get sinc interpolation parameters for this quality preset
    fn get_params(&self) -> SincInterpolationParameters {
        match self {
            ResamplerQuality::Fast => SincInterpolationParameters {
                sinc_len: 64,
                f_cutoff: 0.915_503_25,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::Blackman,
            },
            ResamplerQuality::Balanced => SincInterpolationParameters {
                sinc_len: 128,
                f_cutoff: 0.925_736_56,
                interpolation: SincInterpolationType::Cubic,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris,
            },
            ResamplerQuality::High => SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.947_331_56,
                interpolation: SincInterpolationType::Cubic,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            },
            ResamplerQuality::Ultra => SincInterpolationParameters {
                sinc_len: 512,
                f_cutoff: 0.947_331_56,
                interpolation: SincInterpolationType::Cubic,
                oversampling_factor: 512,
                window: WindowFunction::BlackmanHarris2,
            },
        }
    }
}

impl Default for ResamplerQuality {
    fn default() -> Self {
        ResamplerQuality::Balanced
    }
}

/// High-quality sample rate converter
pub struct Resampler {
    quality: ResamplerQuality,
    input_rate: u32,
    output_rate: u32,
    resampler: Option<SincFixedIn<f64>>,
}

impl Resampler {
    /// Create a new resampler
    ///
    /// # Arguments
    /// * `quality` - Quality preset to use
    /// * `input_rate` - Input sample rate (Hz)
    /// * `output_rate` - Output sample rate (Hz)
    /// * `channels` - Number of audio channels (typically 2 for stereo)
    pub fn new(quality: ResamplerQuality, input_rate: u32, output_rate: u32, channels: usize) -> Result<Self> {
        let resampler = if input_rate != output_rate {
            let params = quality.get_params();
            let chunk_size = 1024; // Process 1024 frames at a time

            let resampler = SincFixedIn::<f64>::new(
                output_rate as f64 / input_rate as f64,
                2.0, // Max resample ratio difference
                params,
                chunk_size,
                channels,
            ).context("Failed to create resampler")?;

            Some(resampler)
        } else {
            None
        };

        Ok(Self {
            quality,
            input_rate,
            output_rate,
            resampler,
        })
    }

    /// Process interleaved stereo samples
    ///
    /// Converts from input sample rate to output sample rate.
    /// Returns the resampled data, which may have a different length.
    pub fn process(&mut self, samples: &[f64]) -> Result<Vec<f64>> {
        // If no resampling needed, return input as-is
        if self.resampler.is_none() {
            return Ok(samples.to_vec());
        }

        let resampler = self.resampler.as_mut().unwrap();

        // Convert interleaved samples to planar format (rubato expects planar)
        let num_channels = 2; // Stereo
        let num_frames = samples.len() / num_channels;

        let mut planar_input = vec![vec![0.0; num_frames]; num_channels];
        for (i, sample) in samples.iter().enumerate() {
            let channel = i % num_channels;
            let frame = i / num_channels;
            planar_input[channel][frame] = *sample;
        }

        // Process through resampler
        let planar_output = resampler.process(&planar_input, None)
            .context("Resampling failed")?;

        // Convert back to interleaved format
        let output_frames = planar_output[0].len();
        let mut interleaved_output = Vec::with_capacity(output_frames * num_channels);

        for frame in 0..output_frames {
            for channel in 0..num_channels {
                interleaved_output.push(planar_output[channel][frame]);
            }
        }

        Ok(interleaved_output)
    }

    /// Get the current quality preset
    pub fn quality(&self) -> ResamplerQuality {
        self.quality
    }

    /// Get input sample rate
    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    /// Get output sample rate
    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    /// Check if resampling is active (rates differ)
    pub fn is_active(&self) -> bool {
        self.resampler.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_resampling_needed() {
        let mut resampler = Resampler::new(
            ResamplerQuality::Balanced,
            48000,
            48000,
            2,
        ).unwrap();

        let input = vec![0.1, 0.2, 0.3, 0.4];
        let output = resampler.process(&input).unwrap();

        assert_eq!(input, output);
        assert!(!resampler.is_active());
    }

    #[test]
    fn test_resampling_creates_different_length() {
        let mut resampler = Resampler::new(
            ResamplerQuality::Fast,
            44100,
            48000,
            2,
        ).unwrap();

        // Create some test data (stereo chunks)
        let chunk_frames = 1024;
        let num_samples = chunk_frames * 2; // Stereo
        let input: Vec<f64> = (0..num_samples)
            .map(|i| (i as f64 * 440.0 * 2.0 * std::f64::consts::PI / 44100.0).sin() * 0.1)
            .collect();

        let output = resampler.process(&input).unwrap();

        // Upsampling from 44.1 to 48 kHz should produce more samples
        // Ratio is 48000/44100 ≈ 1.088, so output should be ~1114 frames (2228 samples)
        assert!(output.len() > input.len(), "Upsampling should produce more samples");
        assert!(resampler.is_active());
    }

    #[test]
    fn test_quality_presets() {
        let qualities = [
            ResamplerQuality::Fast,
            ResamplerQuality::Balanced,
            ResamplerQuality::High,
            ResamplerQuality::Ultra,
        ];

        for quality in qualities {
            let resampler = Resampler::new(quality, 44100, 48000, 2);
            assert!(resampler.is_ok(), "Failed to create resampler with {:?} quality", quality);
        }
    }

    #[test]
    fn test_quality_as_str() {
        assert_eq!(ResamplerQuality::Fast.as_str(), "Fast");
        assert_eq!(ResamplerQuality::Balanced.as_str(), "Balanced");
        assert_eq!(ResamplerQuality::High.as_str(), "High");
        assert_eq!(ResamplerQuality::Ultra.as_str(), "Ultra");
    }
}
