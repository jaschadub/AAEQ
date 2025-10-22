/// Real-time DSP audio processing module
///
/// Implements parametric EQ using biquad IIR filters for low-latency
/// audio processing in the streaming pipeline.
use aaeq_core::EqPreset;
use std::f64::consts::PI;

/// Biquad filter for parametric EQ
/// Uses Direct Form II transposed structure for numerical stability
#[derive(Debug, Clone)]
pub struct BiquadFilter {
    // Filter coefficients
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    // State variables (per channel)
    z1: Vec<f64>,
    z2: Vec<f64>,
}

impl BiquadFilter {
    /// Create a new biquad filter for the given number of channels
    pub fn new(channels: usize) -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: vec![0.0; channels],
            z2: vec![0.0; channels],
        }
    }

    /// Configure as a parametric EQ (peaking filter)
    ///
    /// # Arguments
    /// * `frequency` - Center frequency in Hz
    /// * `gain` - Gain in dB (-12 to +12 typical)
    /// * `q` - Q factor (bandwidth), typical 0.7-5.0
    /// * `sample_rate` - Sample rate in Hz
    pub fn set_peaking(&mut self, frequency: f64, gain_db: f64, q: f64, sample_rate: f64) {
        let a = 10_f64.powf(gain_db / 40.0); // Amplitude
        let w0 = 2.0 * PI * frequency / sample_rate; // Angular frequency
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        // Peaking EQ coefficients
        let a0 = 1.0 + alpha / a;
        self.b0 = (1.0 + alpha * a) / a0;
        self.b1 = (-2.0 * cos_w0) / a0;
        self.b2 = (1.0 - alpha * a) / a0;
        self.a1 = (-2.0 * cos_w0) / a0;
        self.a2 = (1.0 - alpha / a) / a0;
    }

    /// Process a single sample for a given channel
    #[inline]
    fn process_sample(&mut self, sample: f64, channel: usize) -> f64 {
        // Direct Form II Transposed
        let output = self.b0 * sample + self.z1[channel];
        self.z1[channel] = self.b1 * sample - self.a1 * output + self.z2[channel];
        self.z2[channel] = self.b2 * sample - self.a2 * output;
        output
    }

    /// Reset filter state (clear delays)
    pub fn reset(&mut self) {
        for i in 0..self.z1.len() {
            self.z1[i] = 0.0;
            self.z2[i] = 0.0;
        }
    }
}

/// Multi-band parametric EQ processor
pub struct EqProcessor {
    filters: Vec<BiquadFilter>,
    channels: usize,
    sample_rate: u32,
    enabled: bool,
}

impl EqProcessor {
    /// Create a new EQ processor
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            filters: Vec::new(),
            channels,
            sample_rate,
            enabled: false,
        }
    }

    /// Load an EQ preset
    pub fn load_preset(&mut self, preset: &EqPreset) {
        self.filters.clear();

        // Create a biquad filter for each band
        for band in &preset.bands {
            let mut filter = BiquadFilter::new(self.channels);

            // Use Q factor of 1.0 (moderate bandwidth) for all bands
            // This gives smooth, musical results
            filter.set_peaking(
                band.frequency as f64,
                band.gain as f64,
                1.0, // Default Q = 1.0
                self.sample_rate as f64,
            );

            self.filters.push(filter);
        }

        self.enabled = !self.filters.is_empty();
    }

    /// Apply EQ to an interleaved audio buffer
    ///
    /// # Arguments
    /// * `buffer` - Interleaved audio samples (e.g., LRLRLR for stereo)
    ///
    /// # Note
    /// This modifies the buffer in-place for efficiency
    pub fn process(&mut self, buffer: &mut [f64]) {
        if !self.enabled || self.filters.is_empty() {
            return;
        }

        let channels = self.channels;
        let frame_count = buffer.len() / channels;

        // Process each frame (all channels)
        for frame_idx in 0..frame_count {
            for ch in 0..channels {
                let sample_idx = frame_idx * channels + ch;
                let mut sample = buffer[sample_idx];

                // Apply each EQ band in series
                for filter in &mut self.filters {
                    sample = filter.process_sample(sample, ch);
                }

                buffer[sample_idx] = sample;
            }
        }
    }

    /// Enable/disable EQ processing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if EQ is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Reset all filter states
    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
    }

    /// Get number of active EQ bands
    pub fn band_count(&self) -> usize {
        self.filters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biquad_creation() {
        let filter = BiquadFilter::new(2);
        assert_eq!(filter.z1.len(), 2);
        assert_eq!(filter.z2.len(), 2);
    }

    #[test]
    fn test_eq_processor_creation() {
        let processor = EqProcessor::new(48000, 2);
        assert_eq!(processor.channels, 2);
        assert_eq!(processor.sample_rate, 48000);
        assert!(!processor.is_enabled());
    }

    #[test]
    fn test_load_preset() {
        use aaeq_core::EqBand;

        let mut processor = EqProcessor::new(48000, 2);

        let preset = EqPreset {
            name: "Test".to_string(),
            bands: vec![
                EqBand { frequency: 100, gain: 3.0 },
                EqBand { frequency: 1000, gain: -2.0 },
                EqBand { frequency: 10000, gain: 1.5 },
            ],
        };

        processor.load_preset(&preset);
        assert_eq!(processor.band_count(), 3);
        assert!(processor.is_enabled());
    }

    #[test]
    fn test_process_silence() {
        use aaeq_core::EqBand;

        let mut processor = EqProcessor::new(48000, 2);

        let preset = EqPreset {
            name: "Test".to_string(),
            bands: vec![
                EqBand { frequency: 1000, gain: 6.0 },
            ],
        };

        processor.load_preset(&preset);

        // Process silence - should remain approximately silent
        let mut buffer = vec![0.0; 1024];
        processor.process(&mut buffer);

        // All samples should be very close to zero
        for &sample in &buffer {
            assert!(sample.abs() < 1e-10);
        }
    }

    #[test]
    fn test_process_modifies_signal() {
        use aaeq_core::EqBand;

        let mut processor = EqProcessor::new(48000, 2);

        let preset = EqPreset {
            name: "Test".to_string(),
            bands: vec![
                EqBand { frequency: 1000, gain: 6.0 },
            ],
        };

        processor.load_preset(&preset);

        // Create a 1kHz test tone
        let sample_rate = 48000.0;
        let freq = 1000.0;
        let mut buffer = vec![0.0; 480]; // 10ms of stereo audio

        for i in 0..buffer.len() / 2 {
            let t = i as f64 / sample_rate;
            let sample = (2.0 * PI * freq * t).sin() * 0.1;
            buffer[i * 2] = sample;     // Left
            buffer[i * 2 + 1] = sample; // Right
        }

        let original = buffer.clone();
        processor.process(&mut buffer);

        // The EQ should modify the signal (boost at 1kHz)
        let mut changed = false;
        for i in 0..buffer.len() {
            if (buffer[i] - original[i]).abs() > 1e-6 {
                changed = true;
                break;
            }
        }
        assert!(changed, "EQ should modify the signal");
    }
}
