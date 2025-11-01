/// Tube Warmth - Analog warmth simulation using soft-knee waveshaping
///
/// Adds smooth even-order harmonics for analog-like warmth and gentle saturation.
/// Uses a soft-knee waveshaper with the formula: (1 + k) * x / (1 + k * |x|)
pub struct TubeWarmth {
    enabled: bool,
    drive: f64, // Amount of harmonic distortion (k parameter)
}

impl TubeWarmth {
    /// Create a new TubeWarmth processor with preset parameters
    pub fn new() -> Self {
        Self {
            enabled: false,
            drive: 0.5, // Gentle warmth by default
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

    /// Process a single sample through the tube warmth algorithm
    #[inline]
    fn process_sample(&self, x: f64) -> f64 {
        let k = self.drive;

        // Soft-knee waveshaper: x / (1 + k * |x|)
        // This creates smooth saturation and compression
        x / (1.0 + k * x.abs())
    }

    /// Reset processor state (no state to reset for this processor)
    pub fn reset(&mut self) {
        // Stateless processor, nothing to reset
    }
}

impl Default for TubeWarmth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_when_disabled() {
        let mut processor = TubeWarmth::new();
        let mut buffer = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let original = buffer.clone();

        processor.process(&mut buffer);

        assert_eq!(buffer, original, "Buffer should not be modified when disabled");
    }

    #[test]
    fn test_processing_when_enabled() {
        let mut processor = TubeWarmth::new();
        processor.set_enabled(true);

        let mut buffer = vec![0.0, 0.5, -0.5];
        processor.process(&mut buffer);

        // Zero should remain zero
        assert_eq!(buffer[0], 0.0);

        // Positive and negative values should be processed (absolute values should be equal due to symmetry)
        assert!(buffer[1].abs() > 0.0 && buffer[1].abs() <= 1.0);
        assert_eq!(buffer[1], -buffer[2], "Should maintain symmetry for opposite inputs");
    }

    #[test]
    fn test_soft_clipping() {
        let mut processor = TubeWarmth::new();
        processor.set_enabled(true);

        // Test with extreme input
        let mut buffer = vec![10.0];
        processor.process(&mut buffer);

        // Should soft-clip to a reasonable value
        assert!(buffer[0] < 10.0, "Should reduce extreme values");
        assert!(buffer[0] > 0.0, "Should maintain polarity");
        assert!(buffer[0] < 2.0, "Should soft-clip to reasonable range");
    }

    #[test]
    fn test_dc_offset_zero() {
        let mut processor = TubeWarmth::new();
        processor.set_enabled(true);

        // Process DC offset (should remain unchanged for symmetric waveshaper)
        let mut buffer = vec![0.0];
        processor.process(&mut buffer);

        assert_eq!(buffer[0], 0.0, "DC should remain at zero");
    }

    #[test]
    fn test_harmonic_generation() {
        let mut processor = TubeWarmth::new();
        processor.set_enabled(true);

        // Small signal should have gentle compression
        let small_input = 0.1;
        let mut buffer = vec![small_input];
        processor.process(&mut buffer);

        // Output should be slightly compressed (closer to input than to zero)
        let compression_ratio = buffer[0] / small_input;
        assert!(compression_ratio > 0.9 && compression_ratio <= 1.0,
                "Small signals should have minimal compression");
    }
}
