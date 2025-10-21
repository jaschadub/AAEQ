/// DSP (Digital Signal Processing) modules
///
/// Contains real-time audio processing components:
/// - Dither: High-quality dithering and noise shaping for bit-depth reduction
/// - EQ: Parametric equalization with biquad IIR filters
/// - Headroom: Gain control and clipping prevention
/// - Resampler: High-quality sample rate conversion with sinc interpolation

pub mod dither;
pub mod eq;
pub mod headroom;
pub mod resampler;

// Re-export commonly used types for convenience
pub use dither::{Dither, DitherMode, NoiseShaping};
pub use eq::{BiquadFilter, EqProcessor};
pub use headroom::HeadroomControl;
pub use resampler::{Resampler, ResamplerQuality};
