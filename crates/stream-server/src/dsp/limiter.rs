/// Limiter - Peak limiting to prevent clipping
///
/// Provides transparent peak limiting with look-ahead to prevent output clipping.
/// Uses a simple delay line for look-ahead window.
pub struct Limiter {
    enabled: bool,
    threshold: f64,  // Linear threshold (typically 0.95 to leave headroom)
    ceiling: f64,    // Hard ceiling
    envelope: f64,   // Gain reduction envelope
    release_coeff: f64,
    // Simplified look-ahead: just store a few recent samples
    delay_buffer: Vec<f64>,
    delay_index: usize,
}

impl Limiter {
    /// Create a new Limiter processor with preset parameters
    pub fn new() -> Self {
        let look_ahead_samples = 48; // ~1ms at 48kHz
        Self {
            enabled: false,
            threshold: 0.95,
            ceiling: 1.0,
            envelope: 1.0,  // Start with unity gain
            release_coeff: 0.9995, // Fast release for transparency
            delay_buffer: vec![0.0; look_ahead_samples],
            delay_index: 0,
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
        // Store input in delay buffer
        self.delay_buffer[self.delay_index] = x;
        self.delay_index = (self.delay_index + 1) % self.delay_buffer.len();

        // Get delayed sample
        let delayed = self.delay_buffer[self.delay_index];

        // Peak detection on current input (look-ahead)
        let abs_x = x.abs();
        let target_gain = if abs_x > self.threshold {
            self.threshold / abs_x
        } else {
            1.0
        };

        // Smooth gain reduction envelope (instant attack, smooth release)
        if target_gain < self.envelope {
            self.envelope = target_gain; // Instant attack
        } else {
            self.envelope = self.release_coeff * self.envelope + (1.0 - self.release_coeff) * target_gain;
        }

        // Apply gain reduction to delayed signal
        let limited = delayed * self.envelope;

        // Hard clip at ceiling as safety
        limited.clamp(-self.ceiling, self.ceiling)
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        self.envelope = 1.0;
        self.delay_buffer.fill(0.0);
        self.delay_index = 0;
    }
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new()
    }
}
