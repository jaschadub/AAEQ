/// Transient Enhancer - Restores attack and punch
///
/// Enhances or reduces transients using envelope detection and dynamic gain adjustment.
/// Helps restore attack lost from saturation or compression.
pub struct TransientEnhancer {
    enabled: bool,
    amount: f64,      // Amount of enhancement
    envelope: f64,    // Envelope follower state
    prev_sample: f64, // Previous sample for transient detection
}

impl TransientEnhancer {
    /// Create a new TransientEnhancer processor with preset parameters
    pub fn new() -> Self {
        Self {
            enabled: false,
            amount: 0.5,  // Moderate enhancement
            envelope: 0.0,
            prev_sample: 0.0,
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

        // Envelope follower with fast attack, slow release
        let attack_coeff = 0.1;
        let release_coeff = 0.9995;

        if abs_x > self.envelope {
            self.envelope = attack_coeff * abs_x + (1.0 - attack_coeff) * self.envelope;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * abs_x;
        }

        // Detect transients (sudden increases in level)
        let delta = abs_x - self.prev_sample.abs();
        self.prev_sample = x;

        // Apply gain boost during transients
        let transient_gain = if delta > 0.0 {
            1.0 + self.amount * delta.min(0.5)
        } else {
            1.0
        };

        x * transient_gain
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.prev_sample = 0.0;
    }
}

impl Default for TransientEnhancer {
    fn default() -> Self {
        Self::new()
    }
}
