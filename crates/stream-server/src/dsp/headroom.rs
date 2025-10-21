/// Headroom control and clipping detection module
///
/// Provides configurable headroom to prevent clipping in the DSP chain,
/// with optional auto-compensation and clip detection/counting.

use std::sync::atomic::{AtomicU64, Ordering};

/// Convert dB to linear gain
#[inline]
fn db_to_linear(db: f32) -> f64 {
    10_f64.powf(db as f64 / 20.0)
}

/// Headroom control processor
///
/// Applies a gain reduction at the beginning of the DSP chain to prevent
/// clipping from subsequent processing (EQ, etc.). Can detect and count clipping events.
#[derive(Debug)]
pub struct HeadroomControl {
    /// Headroom in dB (0 to -6, typically -3)
    /// Negative values reduce gain to create headroom
    headroom_db: f32,

    /// Apply makeup gain after processing (future feature)
    auto_compensate: bool,

    /// Enable clip detection and counting
    clip_detection: bool,

    /// Count of detected clips (atomic for thread safety)
    clip_count: AtomicU64,

    /// Pre-computed linear gain from headroom_db
    gain: f64,
}

impl HeadroomControl {
    /// Create a new headroom control with default settings
    /// Default: -3 dB headroom, no auto-compensation, clip detection enabled
    pub fn new() -> Self {
        Self {
            headroom_db: -3.0,
            auto_compensate: false,
            clip_detection: true,
            clip_count: AtomicU64::new(0),
            gain: db_to_linear(-3.0),
        }
    }

    /// Set headroom in dB (0 to -6)
    /// Negative values reduce gain. For example, -3 dB creates 3 dB of headroom.
    pub fn set_headroom_db(&mut self, db: f32) {
        self.headroom_db = db.clamp(-6.0, 0.0);
        self.gain = db_to_linear(self.headroom_db);
    }

    /// Get current headroom setting in dB
    pub fn headroom_db(&self) -> f32 {
        self.headroom_db
    }

    /// Enable or disable auto-compensation
    pub fn set_auto_compensate(&mut self, enabled: bool) {
        self.auto_compensate = enabled;
    }

    /// Check if auto-compensation is enabled
    pub fn auto_compensate(&self) -> bool {
        self.auto_compensate
    }

    /// Enable or disable clip detection
    pub fn set_clip_detection(&mut self, enabled: bool) {
        self.clip_detection = enabled;
    }

    /// Check if clip detection is enabled
    pub fn clip_detection(&self) -> bool {
        self.clip_detection
    }

    /// Get the number of detected clips
    pub fn clip_count(&self) -> u64 {
        self.clip_count.load(Ordering::Relaxed)
    }

    /// Reset the clip counter
    pub fn reset_clip_count(&mut self) {
        self.clip_count.store(0, Ordering::Relaxed);
    }

    /// Process an interleaved audio buffer
    ///
    /// Applies headroom gain reduction and optionally detects clipping.
    /// The buffer is modified in-place for efficiency.
    ///
    /// # Arguments
    /// * `samples` - Interleaved audio samples (e.g., LRLRLR for stereo)
    pub fn process(&mut self, samples: &mut [f64]) {
        // Apply headroom gain
        for sample in samples.iter_mut() {
            *sample *= self.gain;

            // Detect clipping if enabled
            if self.clip_detection && sample.abs() >= 1.0 {
                self.clip_count.fetch_add(1, Ordering::Relaxed);
                // Hard limit to prevent actual clipping
                *sample = sample.clamp(-1.0, 1.0);
            }
        }
    }

    /// Check if any clipping has been detected
    pub fn has_clipped(&self) -> bool {
        self.clip_count() > 0
    }
}

impl Default for HeadroomControl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let control = HeadroomControl::new();
        assert_eq!(control.headroom_db(), -3.0);
        assert!(!control.auto_compensate());
        assert!(control.clip_detection());
        assert_eq!(control.clip_count(), 0);
    }

    #[test]
    fn test_set_headroom() {
        let mut control = HeadroomControl::new();
        control.set_headroom_db(-6.0);
        assert_eq!(control.headroom_db(), -6.0);

        // Test clamping
        control.set_headroom_db(-10.0);
        assert_eq!(control.headroom_db(), -6.0);

        control.set_headroom_db(2.0);
        assert_eq!(control.headroom_db(), 0.0);
    }

    #[test]
    fn test_process_applies_gain() {
        let mut control = HeadroomControl::new();
        control.set_headroom_db(-6.0); // Half amplitude

        let mut samples = vec![1.0, 0.5, -0.5, -1.0];
        control.process(&mut samples);

        // -6 dB ≈ 0.501 linear gain
        assert!((samples[0] - 0.501).abs() < 0.001);
        assert!((samples[1] - 0.250).abs() < 0.001);
        assert!((samples[2] + 0.250).abs() < 0.001);
        assert!((samples[3] + 0.501).abs() < 0.001);
    }

    #[test]
    fn test_clip_detection() {
        let mut control = HeadroomControl::new();
        control.set_headroom_db(0.0); // No headroom, full gain
        control.set_clip_detection(true);

        // This should clip
        let mut samples = vec![1.5, -1.2];
        control.process(&mut samples);

        assert_eq!(control.clip_count(), 2);
        assert!(control.has_clipped());

        // Samples should be hard-limited
        assert_eq!(samples[0], 1.0);
        assert_eq!(samples[1], -1.0);
    }

    #[test]
    fn test_reset_clip_count() {
        let mut control = HeadroomControl::new();
        control.set_headroom_db(0.0);

        let mut samples = vec![1.5];
        control.process(&mut samples);
        assert_eq!(control.clip_count(), 1);

        control.reset_clip_count();
        assert_eq!(control.clip_count(), 0);
        assert!(!control.has_clipped());
    }

    #[test]
    fn test_clip_detection_can_be_disabled() {
        let mut control = HeadroomControl::new();
        control.set_clip_detection(false);
        control.set_headroom_db(0.0);

        let mut samples = vec![1.5, -1.2];
        control.process(&mut samples);

        // No clips should be counted
        assert_eq!(control.clip_count(), 0);

        // Samples should NOT be limited when clip detection is disabled
        assert_eq!(samples[0], 1.5);
        assert_eq!(samples[1], -1.2);
    }
}
