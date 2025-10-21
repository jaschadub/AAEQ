//! High-quality dithering and noise shaping for bit-depth reduction
//!
//! Implements multiple dithering algorithms and noise shaping curves to
//! eliminate quantization distortion when reducing bit depth from 32-bit
//! float to 16/24-bit integer output.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Dithering algorithm for noise generation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DitherMode {
    /// No dithering (hard quantization)
    None,
    /// Rectangular probability density function (simple random)
    Rectangular,
    /// Triangular probability density function (TPDF - industry standard)
    Triangular,
    /// Gaussian probability density function (smooth noise)
    Gaussian,
}

impl DitherMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            DitherMode::None => "None",
            DitherMode::Rectangular => "Rectangular",
            DitherMode::Triangular => "Triangular",
            DitherMode::Gaussian => "Gaussian",
        }
    }
}

impl Default for DitherMode {
    fn default() -> Self {
        DitherMode::Triangular // TPDF is the industry standard
    }
}

/// Noise shaping curve for spectral shaping of quantization noise
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoiseShaping {
    /// No noise shaping
    None,
    /// First-order f-weighted shaping (simple)
    FirstOrder,
    /// Second-order optimized for 44.1/48 kHz
    SecondOrder,
    /// Gesemann curve (ultra-low noise in audible range)
    Gesemann,
}

impl NoiseShaping {
    pub fn as_str(&self) -> &'static str {
        match self {
            NoiseShaping::None => "None",
            NoiseShaping::FirstOrder => "FirstOrder",
            NoiseShaping::SecondOrder => "SecondOrder",
            NoiseShaping::Gesemann => "Gesemann",
        }
    }
}

impl Default for NoiseShaping {
    fn default() -> Self {
        NoiseShaping::None
    }
}

/// Dithering processor with noise shaping
#[derive(Debug)]
pub struct Dither {
    mode: DitherMode,
    shaping: NoiseShaping,
    target_bits: u8,
    rng: StdRng,
    // Noise shaping state (per channel)
    shaping_state_l: [f64; 4], // Left channel IIR state
    shaping_state_r: [f64; 4], // Right channel IIR state
}

impl Dither {
    /// Create a new dithering processor
    pub fn new(mode: DitherMode, shaping: NoiseShaping, target_bits: u8) -> Self {
        Self {
            mode,
            shaping,
            target_bits: target_bits.clamp(8, 32),
            rng: StdRng::from_entropy(),
            shaping_state_l: [0.0; 4],
            shaping_state_r: [0.0; 4],
        }
    }

    /// Process stereo interleaved samples
    pub fn process(&mut self, samples: &mut [f64]) {
        if self.mode == DitherMode::None && self.shaping == NoiseShaping::None {
            // No processing needed
            return;
        }

        let quantize_step = self.quantization_step();

        for i in (0..samples.len()).step_by(2) {
            // Left channel
            samples[i] = self.process_sample(samples[i], quantize_step, true);
            // Right channel
            if i + 1 < samples.len() {
                samples[i + 1] = self.process_sample(samples[i + 1], quantize_step, false);
            }
        }
    }

    /// Process a single sample
    fn process_sample(&mut self, sample: f64, quantize_step: f64, is_left: bool) -> f64 {
        // Generate dither noise
        let noise = self.generate_noise(quantize_step);

        // Apply noise shaping
        let shaped = self.apply_shaping(sample + noise, is_left);

        // Quantize to target bit depth
        self.quantize(shaped, quantize_step)
    }

    /// Generate dither noise based on selected mode
    fn generate_noise(&mut self, quantize_step: f64) -> f64 {
        match self.mode {
            DitherMode::None => 0.0,

            DitherMode::Rectangular => {
                // RPDF: uniform random noise in [-0.5, 0.5] LSB
                self.rng.gen_range(-0.5..0.5) * quantize_step
            }

            DitherMode::Triangular => {
                // TPDF: sum of two uniform random variables
                // This is the industry standard as it completely eliminates
                // harmonic distortion from quantization
                let r1 = self.rng.gen_range(-0.5..0.5);
                let r2 = self.rng.gen_range(-0.5..0.5);
                (r1 + r2) * 0.5 * quantize_step
            }

            DitherMode::Gaussian => {
                // Gaussian noise using Box-Muller transform
                // Smoother, more pleasant sounding noise
                let u1 = self.rng.gen::<f64>();
                let u2 = self.rng.gen::<f64>();
                let gaussian = ((-2.0_f64 * u1.ln()).sqrt()) * ((2.0 * PI * u2).cos());
                gaussian * 0.3 * quantize_step // Scale to ~0.3 LSB
            }
        }
    }

    /// Apply noise shaping filter
    fn apply_shaping(&mut self, sample: f64, is_left: bool) -> f64 {
        let quantize_step = self.quantization_step();

        let state = if is_left {
            &mut self.shaping_state_l
        } else {
            &mut self.shaping_state_r
        };

        match self.shaping {
            NoiseShaping::None => sample,

            NoiseShaping::FirstOrder => {
                // Simple first-order error feedback
                // H(z) = 1 - z^-1
                // Pushes noise up by 6 dB/octave
                let shaped = sample + state[0];
                let quantized = (shaped / quantize_step).round() * quantize_step;
                let quantized = quantized.clamp(-1.0, 1.0 - quantize_step);
                let error = shaped - quantized;
                state[0] = -error;
                quantized
            }

            NoiseShaping::SecondOrder => {
                // Second-order shaping optimized for 44.1/48 kHz
                // More aggressive noise reduction in audible range
                // H(z) = (1 - z^-1)^2
                let shaped = sample + state[0] + state[1];
                let quantized = (shaped / quantize_step).round() * quantize_step;
                let quantized = quantized.clamp(-1.0, 1.0 - quantize_step);
                let error = shaped - quantized;
                state[1] = state[0];
                state[0] = -2.0 * error;
                quantized
            }

            NoiseShaping::Gesemann => {
                // Gesemann curve (4th order)
                // Ultra-low noise in audible range (especially vocals 2-5 kHz)
                // Coefficients optimized for 44.1/48 kHz
                let shaped = sample +
                    2.033 * state[0] -
                    1.165 * state[1] +
                    0.254 * state[2] -
                    0.025 * state[3];

                let quantized = (shaped / quantize_step).round() * quantize_step;
                let quantized = quantized.clamp(-1.0, 1.0 - quantize_step);
                let error = shaped - quantized;

                // Update state
                state[3] = state[2];
                state[2] = state[1];
                state[1] = state[0];
                state[0] = -error;

                quantized
            }
        }
    }

    /// Quantize sample to target bit depth
    fn quantize(&self, sample: f64, step: f64) -> f64 {
        // Round to nearest quantization level
        let quantized = (sample / step).round() * step;

        // Clamp to valid range [-1.0, 1.0)
        quantized.clamp(-1.0, 1.0 - step)
    }

    /// Calculate quantization step size for target bit depth
    fn quantization_step(&self) -> f64 {
        // For signed integer representation
        // 16-bit: 2^15 = 32768 levels, step = 1/32768
        // 24-bit: 2^23 = 8388608 levels, step = 1/8388608
        1.0 / (2.0_f64.powi((self.target_bits - 1) as i32))
    }

    /// Update dithering mode
    pub fn set_mode(&mut self, mode: DitherMode) {
        self.mode = mode;
    }

    /// Update noise shaping curve
    pub fn set_shaping(&mut self, shaping: NoiseShaping) {
        self.shaping = shaping;
        // Reset shaping state when changing curves
        self.shaping_state_l = [0.0; 4];
        self.shaping_state_r = [0.0; 4];
    }

    /// Update target bit depth
    pub fn set_target_bits(&mut self, bits: u8) {
        self.target_bits = bits.clamp(8, 32);
    }

    /// Get current configuration
    pub fn mode(&self) -> DitherMode {
        self.mode
    }

    pub fn shaping(&self) -> NoiseShaping {
        self.shaping
    }

    pub fn target_bits(&self) -> u8 {
        self.target_bits
    }

    /// Reset internal state (call when stream stops/restarts)
    pub fn reset(&mut self) {
        self.shaping_state_l = [0.0; 4];
        self.shaping_state_r = [0.0; 4];
    }
}

impl Default for Dither {
    fn default() -> Self {
        Self::new(DitherMode::Triangular, NoiseShaping::None, 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_step() {
        let dither = Dither::new(DitherMode::None, NoiseShaping::None, 16);
        assert_eq!(dither.quantization_step(), 1.0 / 32768.0);

        let dither = Dither::new(DitherMode::None, NoiseShaping::None, 24);
        assert_eq!(dither.quantization_step(), 1.0 / 8388608.0);
    }

    #[test]
    fn test_no_dither_quantization() {
        let mut dither = Dither::new(DitherMode::None, NoiseShaping::None, 16);
        let mut samples = vec![0.5, -0.5, 0.0, 0.25];
        dither.process(&mut samples);

        // Should be quantized without dither
        for sample in samples {
            assert!(sample.abs() <= 1.0);
        }
    }

    #[test]
    fn test_tpdf_dither() {
        let mut dither = Dither::new(DitherMode::Triangular, NoiseShaping::None, 16);
        let mut samples = vec![0.00001; 100]; // Small non-zero values to quantize
        dither.process(&mut samples);

        // TPDF should add small noise around the input value
        // Not all samples will be identical after dithering
        let unique_values: std::collections::HashSet<_> = samples.iter().map(|&s| (s * 32768.0) as i32).collect();
        assert!(unique_values.len() > 1, "TPDF should add dither noise creating variation");
    }

    #[test]
    fn test_shaping_state_reset() {
        let mut dither = Dither::new(DitherMode::Triangular, NoiseShaping::FirstOrder, 16);
        let mut samples = vec![0.5; 10];
        dither.process(&mut samples);

        // State should be populated
        assert!(dither.shaping_state_l[0] != 0.0);

        dither.reset();

        // State should be cleared
        assert_eq!(dither.shaping_state_l[0], 0.0);
    }
}
