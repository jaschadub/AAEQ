/// Exciter / Harmonic Enhancer - Adds "air" and presence
///
/// Synthesizes harmonics above 6 kHz to add brightness and air to the signal.
/// Uses band-splitting and harmonic generation on the upper band.
pub struct Exciter {
    enabled: bool,
    amount: f64, // Amount of harmonic enhancement
    // Simple high-shelf filter state
    hp_z1: f64,
}

impl Exciter {
    /// Create a new Exciter processor with preset parameters
    pub fn new() -> Self {
        Self {
            enabled: false,
            amount: 0.3, // Moderate excitement
            hp_z1: 0.0,
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
        // Simple 1-pole high-pass filter (~6kHz at 48kHz)
        let alpha = 0.8; // Cutoff coefficient
        let hp = x - self.hp_z1;
        self.hp_z1 = alpha * self.hp_z1 + (1.0 - alpha) * x;

        // Generate harmonics on high-frequency content
        let harmonics = (hp * 2.0).tanh(); // Soft saturation

        // Mix back with original
        x + harmonics * self.amount
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        self.hp_z1 = 0.0;
    }
}

impl Default for Exciter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_when_disabled() {
        let mut processor = Exciter::new();
        let mut buffer = vec![0.0, 0.5, -0.5];
        let original = buffer.clone();
        processor.process(&mut buffer);
        assert_eq!(buffer, original);
    }

    #[test]
    fn test_reset() {
        let mut processor = Exciter::new();
        processor.set_enabled(true);
        processor.process(&mut [0.5; 10]);
        processor.reset();
        assert_eq!(processor.hp_z1, 0.0);
    }
}
