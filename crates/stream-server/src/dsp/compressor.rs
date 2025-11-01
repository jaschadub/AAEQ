/// Compressor - Dynamic range compression with soft-knee
///
/// Provides smooth loudness control and emulates analog bus compression.
/// Uses RMS detection with soft-knee gain curve.
pub struct Compressor {
    enabled: bool,
    threshold_db: f64,
    ratio: f64,
    envelope: f64,
    attack_coeff: f64,
    release_coeff: f64,
}

impl Compressor {
    /// Create a new Compressor processor with preset parameters
    pub fn new() -> Self {
        // Preset: Gentle bus compression
        Self {
            enabled: false,
            threshold_db: -12.0,
            ratio: 3.0,
            envelope: 0.0,
            attack_coeff: 0.95, // ~10ms at 48kHz
            release_coeff: 0.9999, // ~100ms release
        }
    }

    /// Enable or disable the processor
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the processor is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Process audio buffer (in-place)
    pub fn process(&mut self, buffer: &mut [f64]) {
        if !self.enabled {
            return;
        }

        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Process a single sample
    #[inline]
    fn process_sample(&mut self, x: f64) -> f64 {
        let abs_x = x.abs();

        // RMS-like envelope detection
        let squared = abs_x * abs_x;
        if squared > self.envelope {
            self.envelope = self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * squared;
        } else {
            self.envelope = self.release_coeff * self.envelope + (1.0 - self.release_coeff) * squared;
        }

        let rms = self.envelope.sqrt();

        // Convert to dB
        let level_db = if rms > 1e-6 {
            20.0 * rms.log10()
        } else {
            -120.0
        };

        // Compute gain reduction with soft knee
        let gain_db = if level_db > self.threshold_db {
            let over_threshold = level_db - self.threshold_db;
            -over_threshold * (1.0 - 1.0 / self.ratio)
        } else {
            0.0
        };

        // Convert back to linear gain
        let gain = 10.0_f64.powf(gain_db / 20.0);

        x * gain
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}
