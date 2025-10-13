# Stream Server Implementation Summary

## Overview

Implementation of the core streaming infrastructure as specified in `v2_ROADMAP.md`. This represents **Milestone 1 (M1)** foundations from the roadmap.

## What Has Been Implemented

### 1. New Crate: `stream-server`

Location: `crates/stream-server/`

A new library crate providing the foundational types and abstractions for audio streaming.

### 2. Core Audio Types (`src/types.rs`)

Implemented the fundamental data structures for audio streaming:

- **`AudioBlock<'a>`**: Zero-copy reference to interleaved stereo audio frames
  - Contains 64-bit float samples (native DSP format)
  - Includes sample rate and channel count
  - Implements `Copy` for efficient passing
  - Validation methods to ensure data integrity

- **`SampleFormat`**: Enum for output formats
  - `F64` (64-bit float - native)
  - `F32` (32-bit float)
  - `S24LE` (24-bit signed integer, little-endian)
  - `S16LE` (16-bit signed integer, little-endian)
  - Helper methods for byte sizes and bit depths

- **`OutputConfig`**: Configuration for output sinks
  - Target sample rate
  - Channel count
  - Output format
  - Buffer size in milliseconds (for jitter compensation)
  - Exclusive mode flag (for WASAPI/CoreAudio)
  - Helper methods for buffer calculations

### 3. OutputSink Trait (`src/sink.rs`)

Defines the abstract interface for all output implementations:

```rust
#[async_trait]
pub trait OutputSink: Send + Sync {
    fn name(&self) -> &'static str;
    async fn open(&mut self, cfg: OutputConfig) -> Result<()>;
    async fn write(&mut self, block: AudioBlock<'_>) -> Result<()>;
    async fn drain(&mut self) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
    fn latency_ms(&self) -> u32;
    fn is_open(&self) -> bool;
}
```

Also includes:
- **`SinkStats`**: Performance monitoring (frames written, underruns, overruns, buffer fill)
- Mock implementation for testing

### 4. OutputManager (`src/manager.rs`)

Central manager for routing audio to different output sinks:

- **Registration**: Register multiple output sinks
- **Selection**: Select active sink by index or name
- **Routing**: Write audio blocks to the active sink
- **Lifecycle**: Proper open/close management
- **Statistics**: Track performance metrics per sink
- **Thread-safe**: Provides `SharedOutputManager` type using `Arc<RwLock<>>`

Key methods:
- `register_sink()` - Add a new sink
- `select_sink()` / `select_sink_by_name()` - Choose active output
- `write()` - Send audio to active sink
- `drain()` / `close_active()` - Lifecycle management
- `active_sink_*()` - Query active sink state

### 5. Audio Processing Utilities (`src/convert.rs`)

High-quality audio format conversion and processing:

#### Format Conversion
- **`convert_format()`**: Convert AudioBlock to target sample format
  - Automatic bit-depth conversion with dithering
  - Proper byte packing for 24-bit audio
  - Handles all supported formats

- **`convert_with_gain()`**: Convert with pre-gain adjustment
  - dB to linear conversion
  - Apply gain before format conversion

#### Dithering
- **TPDF (Triangular Probability Density Function) dithering**
  - Applied when reducing bit depth
  - Reduces quantization noise
  - Maintains audio quality

#### Analysis
- **`calculate_rms_dbfs()`**: Calculate RMS level in dBFS
- **`calculate_peak_dbfs()`**: Calculate peak level in dBFS

#### Dynamics Processing
- **`apply_soft_limiter()`**: Prevent clipping with soft knee
  - Uses tanh for smooth limiting
  - Configurable threshold

## Testing

All modules include comprehensive unit tests:
- **20 passing tests** covering all functionality
- Mock implementations for trait testing
- Edge case validation
- Format conversion accuracy

## Integration with Roadmap

This implementation provides the foundation for the v2 roadmap:

### ✅ Completed (M1 Foundation)
- Core audio types (64-bit float processing)
- OutputSink trait interface
- OutputManager for sink routing
- Format conversion with dithering
- TPDF dither implementation
- Buffer size calculations

### 🔄 Next Steps (M1 Completion)
- Implement actual sink adapters:
  - `sink_local_coreaudio` (macOS)
  - `sink_local_wasapi` (Windows)
  - `sink_local_alsa` (Linux)
- Add resampling support (using rubato - already included as dependency)

### 📋 Future Milestones
- **M2**: DLNA/UPnP HTTP PCM streaming
- **M3**: AirPlay 2 sender integration
- **M4**: Output Manager UI + HTTP control API
- **M5**: Optional NAA/RTP output

## Dependencies Added

### Workspace Dependencies (already available)
- `tokio` - Async runtime
- `async-trait` - Async trait support
- `anyhow` - Error handling
- `thiserror` - Error types
- `tracing` - Logging
- `serde` - Serialization

### New Dependencies
- `rubato = "0.15"` - High-quality resampling (for future use)
- `dasp_sample = "0.11"` - Sample type conversions
- `fastrand = "2.0"` - Fast random number generation (for dithering)

## Fidelity & Safety

Following the roadmap's fidelity rules:

- ✅ All processing in 64-bit float internally
- ✅ TPDF dithering when reducing bit depth
- ✅ No lossy codecs (except future AirPlay ALAC)
- ✅ Clean trait-based architecture for extensibility
- ⏳ -3 dB pre-gain (to be applied in DSP core)
- ⏳ Optional soft limiter (utility provided, needs integration)

## Usage Example

```rust
use stream_server::*;

// Create manager
let mut manager = OutputManager::new();

// Register sinks (mock example)
manager.register_sink(Box::new(LocalDacSink::new()));
manager.register_sink(Box::new(DlnaSink::new()));

// Configure and select output
let config = OutputConfig {
    sample_rate: 48000,
    channels: 2,
    format: SampleFormat::S24LE,
    buffer_ms: 150,
    exclusive: false,
};

manager.select_sink_by_name("local_dac", config).await?;

// Stream audio
let frames = vec![0.0; 4800]; // 100ms of audio at 48kHz stereo
let block = AudioBlock::new(&frames, 48000, 2);
manager.write(block).await?;

// Check statistics
if let Some(stats) = manager.active_sink_stats() {
    println!("Frames written: {}", stats.frames_written);
}
```

## File Structure

```
crates/stream-server/
├── Cargo.toml           # Dependencies
└── src/
    ├── lib.rs           # Module exports
    ├── types.rs         # AudioBlock, SampleFormat, OutputConfig
    ├── sink.rs          # OutputSink trait
    ├── manager.rs       # OutputManager
    └── convert.rs       # Format conversion & processing
```

## Architecture Alignment

This implementation directly follows the architecture specified in `v2_ROADMAP.md`:

```
AAEQ DSP Core (64f PCM)
        ↓
    AudioBlock
        ↓
   OutputManager
        ↓
    OutputSink (trait)
        ↓
    [Adapters TBD]
```

The abstractions are designed to support all planned output types:
- Local DACs (CoreAudio/WASAPI/ALSA)
- UPnP/DLNA PCM streaming
- AirPlay 2 (ALAC)
- HQPlayer NAA / RTP

## Quality Assurance

- All code compiles without warnings (after fixes)
- All tests pass (20/20)
- No unsafe code used
- Proper error handling with `anyhow::Result`
- Documentation comments on public APIs
- Follows Rust best practices and idioms

## Next Development Priorities

1. **Implement local DAC sink** (pick one platform to start):
   - Use `cpal` for cross-platform support, or
   - Platform-specific implementations for lower latency

2. **Add resampling support**:
   - Integrate `rubato` for sample rate conversion
   - Support arbitrary input → output rate conversions

3. **Create buffering/jitter management**:
   - Ring buffer implementation
   - Adaptive buffering for network sinks

4. **Testing**:
   - Integration tests with real audio
   - Latency measurement
   - Buffer underrun/overrun testing
