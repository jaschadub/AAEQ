/// Stereo Width - Mid/Side processing for stereo image control
///
/// Adjusts the perceived stereo width by manipulating the Side component
/// of the stereo signal using Mid/Side matrix.
pub struct StereoWidth {
    enabled: bool,
    width: f64, // Width control: 0.0 = mono, 1.0 = normal, 2.0 = wide
}

impl StereoWidth {
    /// Create a new StereoWidth processor with preset parameters
    pub fn new() -> Self {
        Self {
            enabled: false,
            width: 1.5, // Moderately widened
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

    /// Process stereo audio buffer (interleaved L/R pairs)
    /// Buffer format: [L, R, L, R, ...]
    pub fn process_stereo(&mut self, buffer: &mut [f64]) {
        if !self.enabled {
            return;
        }

        // Process in stereo pairs
        for chunk in buffer.chunks_exact_mut(2) {
            let left = chunk[0];
            let right = chunk[1];

            // Encode to Mid/Side
            let mid = (left + right) * 0.5;
            let side = (left - right) * 0.5;

            // Adjust side component
            let side_adjusted = side * self.width;

            // Decode back to Left/Right
            chunk[0] = mid + side_adjusted;  // Left
            chunk[1] = mid - side_adjusted;  // Right
        }
    }

    /// Process mono buffer (no effect, pass through)
    pub fn process(&mut self, _buffer: &mut [f64]) {
        // Stereo width has no effect on mono signals
    }

    /// Reset processor state (no state to reset)
    pub fn reset(&mut self) {
        // Stateless processor
    }
}

impl Default for StereoWidth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mono_unchanged() {
        let mut processor = StereoWidth::new();
        processor.set_enabled(true);
        processor.width = 0.0; // Full mono

        let mut buffer = vec![1.0, 1.0]; // Identical L/R = mono
        processor.process_stereo(&mut buffer);

        // Mono signal should remain mono
        assert_eq!(buffer[0], buffer[1]);
    }

    #[test]
    fn test_width_zero_creates_mono() {
        let mut processor = StereoWidth::new();
        processor.set_enabled(true);
        processor.width = 0.0;

        let mut buffer = vec![1.0, 0.5]; // Stereo signal
        processor.process_stereo(&mut buffer);

        // Should collapse to mono (L == R)
        assert_eq!(buffer[0], buffer[1]);
    }

    #[test]
    fn test_width_one_preserves() {
        let mut processor = StereoWidth::new();
        processor.set_enabled(true);
        processor.width = 1.0;

        let mut buffer = vec![1.0, 0.5];
        let original = buffer.clone();
        processor.process_stereo(&mut buffer);

        // Should preserve original stereo image
        assert!((buffer[0] - original[0]).abs() < 1e-10);
        assert!((buffer[1] - original[1]).abs() < 1e-10);
    }
}
