/// Expander / Noise Gate - Downward expansion
///
/// Reduces noise and cleans silence by attenuating signals below threshold.
/// Uses hysteresis to prevent chattering.
pub struct Expander {
    enabled: bool,
    threshold_db: f64,
    ratio: f64,
    envelope: f64,
    gate_open: bool,  // Hysteresis state
    attack_coeff: f64,
    release_coeff: f64,
}

impl Expander {
    /// Create a new Expander processor with preset parameters
    pub fn new() -> Self {
        // Preset: Gentle noise gate
        Self {
            enabled: false,
            threshold_db: -40.0,  // Gate closes below -40dB
            ratio: 2.0,           // Gentle expansion
            envelope: 0.0,
            gate_open: true,      // Start open
            attack_coeff: 0.9,    // Fast attack
            release_coeff: 0.9997, // Slow release to avoid chattering
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

        // Envelope detection
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

        // Hysteresis: add 3dB difference between open and close thresholds
        let open_threshold = self.threshold_db + 3.0;
        let close_threshold = self.threshold_db;

        // Update gate state with hysteresis
        if level_db > open_threshold {
            self.gate_open = true;
        } else if level_db < close_threshold {
            self.gate_open = false;
        }

        // Compute gain
        let gain = if self.gate_open {
            1.0 // Pass through when open
        } else {
            // Expand (reduce) when below threshold
            if level_db > close_threshold {
                1.0 // In hysteresis region, keep current state
            } else {
                let below_threshold = close_threshold - level_db;
                let gain_reduction_db = below_threshold * (self.ratio - 1.0);
                10.0_f64.powf(-gain_reduction_db / 20.0)
            }
        };

        x * gain
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.gate_open = true;
    }
}

impl Default for Expander {
    fn default() -> Self {
        Self::new()
    }
}
