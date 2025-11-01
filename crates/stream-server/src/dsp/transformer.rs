/// Transformer Color - Transformer saturation emulation
///
/// Adds subtle 2nd and 3rd harmonic content with low-frequency saturation.
/// Simulates the coloration of audio transformers commonly found in vintage gear.
pub struct Transformer {
    enabled: bool,
    drive: f64, // Amount of transformer saturation
}

impl Transformer {
    /// Create a new Transformer processor with preset parameters
    pub fn new() -> Self {
        Self {
            enabled: false,
            drive: 0.4, // Subtle coloration
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

    /// Process a single sample through the transformer coloration algorithm
    #[inline]
    fn process_sample(&self, x: f64) -> f64 {
        // Asymmetric waveshaper that generates 2nd and 3rd harmonics
        // Formula: x + k * x^2 + k/2 * x^3
        // This creates both even (2nd) and odd (3rd) harmonics

        let k = self.drive;
        let x2 = x * x;
        let x3 = x2 * x;

        // Mix dry signal with harmonic content
        let wet = x + k * x2 + (k * 0.5) * x3;

        // Soft clip to prevent excessive distortion
        wet.clamp(-1.2, 1.2) * 0.9 // Slight gain compensation
    }

    /// Reset processor state (no state to reset for this processor)
    pub fn reset(&mut self) {
        // Stateless processor, nothing to reset
    }
}

impl Default for Transformer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_when_disabled() {
        let mut processor = Transformer::new();
        let mut buffer = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let original = buffer.clone();

        processor.process(&mut buffer);

        assert_eq!(buffer, original, "Buffer should not be modified when disabled");
    }

    #[test]
    fn test_processing_when_enabled() {
        let mut processor = Transformer::new();
        processor.set_enabled(true);

        let mut buffer = vec![0.0, 0.5, -0.5];
        processor.process(&mut buffer);

        // Zero should remain zero (or very close due to FP math)
        assert!(buffer[0].abs() < 1e-10, "Zero input should produce zero output");

        // Positive and negative values should be different (asymmetric due to x^2 term)
        assert_ne!(buffer[1], -buffer[2], "Should be asymmetric due to even harmonics");
    }

    #[test]
    fn test_adds_harmonics() {
        let mut processor = Transformer::new();
        processor.set_enabled(true);

        // Small signal
        let input = 0.3;
        let mut buffer = vec![input];
        processor.process(&mut buffer);

        // Output should be slightly different due to harmonic content
        assert_ne!(buffer[0], input, "Should add harmonic content");
        assert!(buffer[0].abs() > input.abs() * 0.9, "Should not drastically reduce level");
    }

    #[test]
    fn test_asymmetric_saturation() {
        let mut processor = Transformer::new();
        processor.set_enabled(true);

        // Test positive input
        let mut pos_buffer = vec![0.5];
        processor.process(&mut pos_buffer);

        // Test negative input
        let mut neg_buffer = vec![-0.5];
        processor.process(&mut neg_buffer);

        // Due to x^2 term, positive and negative should not be exact opposites
        let ratio = (pos_buffer[0] / neg_buffer[0]).abs();
        assert!((ratio - 1.0).abs() > 0.01, "Should have asymmetric transfer curve");
    }

    #[test]
    fn test_soft_clipping() {
        let mut processor = Transformer::new();
        processor.set_enabled(true);

        // Test with larger input
        let mut buffer = vec![1.5];
        processor.process(&mut buffer);

        // Should be soft-clipped
        assert!(buffer[0] < 1.5, "Should reduce extreme values");
        assert!(buffer[0] <= 1.2, "Should respect clipping threshold");
    }

    #[test]
    fn test_zero_stays_zero() {
        let mut processor = Transformer::new();
        processor.set_enabled(true);

        let mut buffer = vec![0.0];
        processor.process(&mut buffer);

        assert!(buffer[0].abs() < 1e-10, "Zero should remain zero");
    }
}
