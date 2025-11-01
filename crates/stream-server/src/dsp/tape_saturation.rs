/// Tape Saturation - Analog tape emulation with soft compression
///
/// Adds soft compression, low-end glue, and high-frequency smoothing.
/// Uses asymmetric tanh() with slow DC bias to simulate tape hysteresis.
pub struct TapeSaturation {
    enabled: bool,
    drive: f64,          // Amount of saturation
    dc_bias: f64,        // Slow-moving DC bias for asymmetry
    dc_filter_coeff: f64, // Low-pass filter coefficient for DC tracking
}

impl TapeSaturation {
    /// Create a new TapeSaturation processor with preset parameters
    pub fn new() -> Self {
        Self {
            enabled: false,
            drive: 1.5,           // Moderate tape saturation
            dc_bias: 0.0,         // Start with no bias
            dc_filter_coeff: 0.9995, // Very slow DC tracking
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

    /// Process a single sample through the tape saturation algorithm
    #[inline]
    fn process_sample(&mut self, x: f64) -> f64 {
        // Update DC bias with slow-moving filter to track signal bias
        // This creates asymmetric saturation characteristic of tape
        self.dc_bias = self.dc_filter_coeff * self.dc_bias + (1.0 - self.dc_filter_coeff) * x;

        // Apply saturation with asymmetric bias
        let biased_input = x - self.dc_bias * 0.1; // Subtle bias influence
        let saturated = (self.drive * biased_input).tanh();

        // Normalize output
        saturated / self.drive
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        self.dc_bias = 0.0;
    }
}

impl Default for TapeSaturation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_when_disabled() {
        let mut processor = TapeSaturation::new();
        let mut buffer = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let original = buffer.clone();

        processor.process(&mut buffer);

        assert_eq!(buffer, original, "Buffer should not be modified when disabled");
    }

    #[test]
    fn test_processing_when_enabled() {
        let mut processor = TapeSaturation::new();
        processor.set_enabled(true);

        let mut buffer = vec![0.5, -0.5, 0.8];
        processor.process(&mut buffer);

        // All values should be processed and within reasonable bounds
        for sample in &buffer {
            assert!(sample.abs() <= 1.0, "Output should be bounded");
        }
    }

    #[test]
    fn test_soft_saturation() {
        let mut processor = TapeSaturation::new();
        processor.set_enabled(true);

        // Test with extreme input
        let mut buffer = vec![10.0];
        processor.process(&mut buffer);

        // Should saturate to a value close to 1.0 but not exceed it significantly
        assert!(buffer[0] < 2.0, "Should soft-saturate extreme values");
        assert!(buffer[0] > 0.5, "Should maintain polarity and significant level");
    }

    #[test]
    fn test_reset() {
        let mut processor = TapeSaturation::new();
        processor.set_enabled(true);

        // Process some samples to build up DC bias
        let mut buffer = vec![0.5; 100];
        processor.process(&mut buffer);

        // Reset should clear DC bias
        processor.reset();
        assert_eq!(processor.dc_bias, 0.0, "Reset should clear DC bias");
    }

    #[test]
    fn test_zero_input() {
        let mut processor = TapeSaturation::new();
        processor.set_enabled(true);

        let mut buffer = vec![0.0];
        processor.process(&mut buffer);

        // Zero input should produce near-zero output (might have slight bias)
        assert!(buffer[0].abs() < 0.01, "Zero input should produce near-zero output");
    }

    #[test]
    fn test_dc_bias_builds_slowly() {
        let mut processor = TapeSaturation::new();
        processor.set_enabled(true);

        // Process constant positive signal
        for _ in 0..10 {
            let mut buffer = vec![0.5];
            processor.process(&mut buffer);
        }

        // DC bias should be building but still small
        assert!(processor.dc_bias.abs() < 0.1, "DC bias should build slowly");
        assert!(processor.dc_bias.abs() > 0.0, "DC bias should be non-zero after processing");
    }
}
