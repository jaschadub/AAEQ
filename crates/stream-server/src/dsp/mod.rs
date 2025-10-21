/// DSP (Digital Signal Processing) modules
///
/// Contains real-time audio processing components:
/// - EQ: Parametric equalization with biquad IIR filters
/// - Headroom: Gain control and clipping prevention

pub mod eq;
pub mod headroom;

// Re-export commonly used types for convenience
pub use eq::{BiquadFilter, EqProcessor};
pub use headroom::HeadroomControl;
