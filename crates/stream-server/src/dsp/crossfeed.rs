/// Crossfeed - Headphone crossfeed for natural imaging
///
/// Simulates speaker crosstalk to reduce fatigue and create more natural
/// imaging when listening on headphones. Based on Bauer/Meier model.
pub struct Crossfeed {
    enabled: bool,
    mix: f64,          // Amount of crossfeed (0.0 to 1.0)
    // Filter state for each channel
    left_z1: f64,
    left_z2: f64,
    right_z1: f64,
    right_z2: f64,
}

impl Crossfeed {
    /// Create a new Crossfeed processor with preset parameters
    pub fn new() -> Self {
        Self {
            enabled: false,
            mix: 0.7,     // Moderate crossfeed
            left_z1: 0.0,
            left_z2: 0.0,
            right_z1: 0.0,
            right_z2: 0.0,
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

        for chunk in buffer.chunks_exact_mut(2) {
            let (new_left, new_right) = self.process_sample_pair(chunk[0], chunk[1]);
            chunk[0] = new_left;
            chunk[1] = new_right;
        }
    }

    /// Process a stereo sample pair
    #[inline]
    fn process_sample_pair(&mut self, left: f64, right: f64) -> (f64, f64) {
        // Simple low-pass filtered crossfeed
        // Mix some of the opposite channel with delay and filtering

        // Low-pass filter coefficients (simulate head shadow)
        let lpf_coeff = 0.85;

        // Filter left channel
        self.left_z1 = lpf_coeff * self.left_z1 + (1.0 - lpf_coeff) * right;

        // Filter right channel
        self.right_z1 = lpf_coeff * self.right_z1 + (1.0 - lpf_coeff) * left;

        // Mix crossfeed
        let new_left = left + self.left_z1 * self.mix * 0.3;
        let new_right = right + self.right_z1 * self.mix * 0.3;

        (new_left, new_right)
    }

    /// Process mono buffer (no effect)
    pub fn process(&mut self, _buffer: &mut [f64]) {
        // Crossfeed only applies to stereo
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        self.left_z1 = 0.0;
        self.left_z2 = 0.0;
        self.right_z1 = 0.0;
        self.right_z2 = 0.0;
    }
}

impl Default for Crossfeed {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset() {
        let mut processor = Crossfeed::new();
        processor.set_enabled(true);
        processor.process_stereo(&mut [1.0, 0.5, 1.0, 0.5]);
        processor.reset();
        assert_eq!(processor.left_z1, 0.0);
        assert_eq!(processor.right_z1, 0.0);
    }
}
