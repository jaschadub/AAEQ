/// Room Ambience Simulator - Adds subtle early reflections
///
/// Creates a sense of acoustic space with short early reflections.
/// Simulates the natural ambience of a listening room.
pub struct RoomAmbience {
    enabled: bool,
    mix: f64,           // Wet/dry mix
    delay_lines: Vec<Vec<f64>>, // Multiple short delay lines for reflections
    delay_indices: Vec<usize>,
}

impl RoomAmbience {
    /// Create a new RoomAmbience processor with preset parameters
    pub fn new() -> Self {
        // Create 4 delay lines with different lengths (early reflections)
        // Lengths in samples at 48kHz: ~5ms, ~7ms, ~11ms, ~13ms
        let delay_lengths = [240, 336, 528, 624];
        let delay_lines = delay_lengths.iter()
            .map(|&len| vec![0.0; len])
            .collect();

        Self {
            enabled: false,
            mix: 0.15,          // Subtle ambience
            delay_lines,
            delay_indices: vec![0; 4],
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
        // Feedback coefficients for each delay line (decreasing for each reflection)
        let gains = [0.4, 0.3, 0.2, 0.15];

        let mut reflections = 0.0;

        // Process each delay line
        for (i, delay_line) in self.delay_lines.iter_mut().enumerate() {
            // Read delayed sample
            let delayed = delay_line[self.delay_indices[i]];
            reflections += delayed * gains[i];

            // Write input to delay line
            delay_line[self.delay_indices[i]] = x;

            // Increment index with wrap
            self.delay_indices[i] = (self.delay_indices[i] + 1) % delay_line.len();
        }

        // Mix dry and wet
        x * (1.0 - self.mix) + reflections * self.mix
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        for delay_line in &mut self.delay_lines {
            delay_line.fill(0.0);
        }
        self.delay_indices.fill(0);
    }
}

impl Default for RoomAmbience {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_when_disabled() {
        let mut processor = RoomAmbience::new();
        let mut buffer = vec![0.0, 0.5, -0.5];
        let original = buffer.clone();
        processor.process(&mut buffer);
        assert_eq!(buffer, original);
    }

    #[test]
    fn test_reset() {
        let mut processor = RoomAmbience::new();
        processor.set_enabled(true);
        processor.process(&mut vec![1.0; 1000]);
        processor.reset();

        for delay_line in &processor.delay_lines {
            assert!(delay_line.iter().all(|&x| x == 0.0));
        }
        assert!(processor.delay_indices.iter().all(|&x| x == 0));
    }
}
