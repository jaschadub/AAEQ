/// DSP (Digital Signal Processing) modules
///
/// Contains real-time audio processing components:
/// - Dither: High-quality dithering and noise shaping for bit-depth reduction
/// - EQ: Parametric equalization with biquad IIR filters
/// - Headroom: Gain control and clipping prevention
/// - Resampler: High-quality sample rate conversion with sinc interpolation
///
/// DSP Enhancers & Filters:
/// - Tone/Character: Tube Warmth, Tape Saturation, Transformer, Exciter, Transient Enhancer
/// - Dynamic Processors: Compressor, Limiter, Expander/Noise Gate
/// - Spatial/Psychoacoustic: Stereo Width, Crossfeed, Room Ambience
pub mod dither;
pub mod eq;
pub mod headroom;
pub mod resampler;

// DSP Enhancers & Filters
pub mod tube_warmth;
pub mod tape_saturation;
pub mod transformer;
pub mod exciter;
pub mod transient_enhancer;
pub mod compressor;
pub mod limiter;
pub mod expander;
pub mod stereo_width;
pub mod crossfeed;
pub mod room_ambience;
pub mod exclusivity;

// Re-export commonly used types for convenience
pub use dither::{Dither, DitherMode, NoiseShaping};
pub use eq::{BiquadFilter, EqProcessor};
pub use headroom::HeadroomControl;
pub use resampler::{Resampler, ResamplerQuality};

// Re-export DSP enhancers and filters
pub use tube_warmth::TubeWarmth;
pub use tape_saturation::TapeSaturation;
pub use transformer::Transformer;
pub use exciter::Exciter;
pub use transient_enhancer::TransientEnhancer;
pub use compressor::Compressor;
pub use limiter::Limiter;
pub use expander::Expander;
pub use stereo_width::StereoWidth;
pub use crossfeed::Crossfeed;
pub use room_ambience::RoomAmbience;
pub use exclusivity::{DspEffect, ExclusivityGroup, ConflictError, validate_toggle, is_effect_enabled, get_enabled_effects};
