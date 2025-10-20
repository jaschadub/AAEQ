# AAEQ Development Roadmap

> Strategic plan for evolving AAEQ from adaptive EQ manager to intelligent mastering environment

**Last Updated:** 2025-10-19

---

## 🎯 Vision

Transform AAEQ into a next-generation intelligent audio processing platform that combines:
- **Automated intelligence** (per-track EQ switching) ✓ Already leading
- **Network flexibility** (DLNA, WiiM, StreamMagic) ✓ Already leading
- **DSP precision** (HQPlayer-inspired processing depth) ← Target area
- **Visual fidelity tools** (pipeline visualization, analysis) ← Target area
- **Room correction** (convolution, measurement integration) ← Target area

---

## 📊 Priority Matrix

| Feature Area | Priority | Complexity | User Impact | Technical Debt Risk |
|-------------|----------|------------|-------------|-------------------|
| Pipeline Visualization | **HIGH** | Low | High | Low |
| Headroom/Clipping Control | **HIGH** | Low | High | Low |
| Advanced DSP Tab (Phase 1) | **HIGH** | Medium | High | Medium |
| Dithering & Noise Shaping | MEDIUM | Medium | Medium | Low |
| High-Rate Resampling | MEDIUM | High | Medium | Medium |
| Room Correction/Convolution | MEDIUM | High | High | High |
| Filter Design Options | LOW | High | Medium | High |
| NAA-like Protocol | LOW | Very High | Low | High |

---

## 🚀 Implementation Phases

### **Phase 1: Foundation & Quick Wins** (v0.6.0 - v0.7.0)
*Target: 2-3 months | Priority: HIGH | Risk: LOW*

Quick wins that add significant value with minimal architectural changes.

#### 1.1 Pipeline Visualization
**Goal:** Show complete signal flow with interactive toggles

**Implementation:**
```rust
// crates/ui-egui/src/pipeline_view.rs
pub struct PipelineView {
    stages: Vec<PipelineStage>,
    node_positions: HashMap<String, Pos2>,
}

pub enum PipelineStage {
    Input { device: String, format: SampleFormat },
    Gain { db: f32, enabled: bool },
    Eq { preset: String, bands: Vec<EqBand>, enabled: bool },
    Compressor { threshold: f32, ratio: f32, enabled: bool },
    Filter { type: FilterType, enabled: bool },
    Dither { mode: DitherMode, enabled: bool },
    Output { device: String, format: SampleFormat },
}
```

**UI Design:**
- Horizontal flow diagram with boxes for each stage
- Green highlight for active stages, gray for bypassed
- Click to toggle/configure each stage
- Real-time metrics per stage (gain reduction, latency, etc.)

**Technical Approach:**
- Use `egui::Ui::horizontal()` with custom painting
- Store pipeline state in `DspView`
- Add bypass toggles to existing DSP chain
- Display latency contribution per stage

**Dependencies:**
- None (uses existing DSP infrastructure)

**Estimated Effort:** 1-2 weeks

---

#### 1.2 Headroom & Clipping Control
**Goal:** Prevent clipping with auto-gain compensation

**Implementation:**
```rust
// crates/stream-server/src/dsp/headroom.rs
pub struct HeadroomControl {
    headroom_db: f32,        // -0 to -6 dB
    auto_compensate: bool,   // Apply makeup gain after processing
    clip_detection: bool,    // Monitor for clipping
    clip_count: AtomicU64,   // Detected clips
}

impl HeadroomControl {
    pub fn process(&mut self, samples: &mut [f64]) {
        let gain = db_to_linear(self.headroom_db);
        for sample in samples.iter_mut() {
            *sample *= gain;

            // Clip detection
            if self.clip_detection && sample.abs() >= 1.0 {
                self.clip_count.fetch_add(1, Ordering::Relaxed);
                *sample = sample.clamp(-1.0, 1.0); // Hard limit
            }
        }
    }
}
```

**UI Elements:**
- Headroom slider: 0 dB to -6 dB (default: -3 dB)
- Auto-compensate checkbox
- Clip counter with reset button
- Visual warning when clips detected

**Technical Approach:**
- Insert at beginning of DSP chain
- Calculate makeup gain based on EQ peak
- Add to `DspView` settings
- Persist in database per profile

**Dependencies:**
- None (simple gain stage)

**Estimated Effort:** 1 week

---

#### 1.3 Device-Aware DSP Templates
**Goal:** Profiles can save DSP settings (sample rate, filters, etc.)

**Implementation:**
```rust
// crates/persistence/migrations/008_dsp_profile_settings.sql
CREATE TABLE dsp_profile_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER NOT NULL,
    sample_rate INTEGER NOT NULL DEFAULT 48000,
    buffer_ms INTEGER NOT NULL DEFAULT 150,
    headroom_db REAL NOT NULL DEFAULT -3.0,
    dither_mode TEXT NOT NULL DEFAULT 'none',
    filter_type TEXT NOT NULL DEFAULT 'linear',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (profile_id) REFERENCES profile(id) ON DELETE CASCADE
);
```

**UI Changes:**
- Settings tab shows DSP config per profile
- "Copy DSP settings from..." dropdown
- Template library (Headphones, Speakers, Car, etc.)

**Technical Approach:**
- Extend `Profile` struct with DSP settings
- Load settings when switching profiles
- Apply to streaming config on start

**Dependencies:**
- Database migration
- Profile switching logic

**Estimated Effort:** 1 week

---

### **Phase 2: DSP Quality Enhancements** (v0.8.0 - v0.9.0)
*Target: 3-4 months | Priority: MEDIUM | Risk: MEDIUM*

Core DSP improvements that enhance audio quality.

#### 2.1 Dithering & Noise Shaping
**Goal:** High-quality bit-depth reduction

**Implementation:**
```rust
// crates/stream-server/src/dsp/dither.rs
pub enum DitherMode {
    None,
    Rectangular,    // Simple random
    Triangular,     // TPDF (Triangular PDF)
    Gaussian,       // Smooth noise
}

pub enum NoiseShaping {
    None,
    FirstOrder,     // f-weighted
    SecondOrder,    // 44.1/48 kHz optimized
    Gesemann,       // Ultra-low noise
}

pub struct Dither {
    mode: DitherMode,
    shaping: NoiseShaping,
    rng: rand::rngs::ThreadRng,
    shaping_state: [f64; 2], // IIR state
}

impl Dither {
    pub fn apply(&mut self, sample: f64, target_bits: u8) -> f64 {
        let quantize_step = 1.0 / (2.0_f64.powi(target_bits as i32 - 1));

        // Generate dither noise
        let noise = match self.mode {
            DitherMode::None => 0.0,
            DitherMode::Rectangular => self.rng.gen_range(-0.5..0.5) * quantize_step,
            DitherMode::Triangular => {
                let r1 = self.rng.gen_range(-0.5..0.5);
                let r2 = self.rng.gen_range(-0.5..0.5);
                (r1 + r2) * 0.5 * quantize_step
            }
            DitherMode::Gaussian => {
                // Box-Muller transform
                let u1 = self.rng.gen::<f64>();
                let u2 = self.rng.gen::<f64>();
                (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos() * 0.3 * quantize_step
            }
        };

        // Apply noise shaping
        let shaped = self.apply_shaping(sample + noise);

        // Quantize
        (shaped / quantize_step).round() * quantize_step
    }

    fn apply_shaping(&mut self, sample: f64) -> f64 {
        match self.shaping {
            NoiseShaping::None => sample,
            NoiseShaping::FirstOrder => {
                let shaped = sample + self.shaping_state[0];
                let error = shaped - shaped.round();
                self.shaping_state[0] = -error;
                shaped
            }
            // Additional shaping curves...
            _ => sample,
        }
    }
}
```

**UI Elements:**
- Dither mode dropdown
- Noise shaping dropdown
- Target bit depth selector (16/24/32)
- "Analyze noise floor" button

**Rust Crates:**
- `rand = "0.8"` (already in project)

**Estimated Effort:** 2-3 weeks

---

#### 2.2 High-Rate Resampling
**Goal:** Optional upsampling for better DAC performance

**Implementation:**
```rust
// crates/stream-server/src/dsp/resampler.rs
use rubato::{Resampler, SincFixedIn, InterpolationType, WindowFunction};

pub struct HighQualityResampler {
    resampler: Option<Box<dyn Resampler<f64>>>,
    input_rate: u32,
    output_rate: u32,
    mode: ResamplingMode,
}

pub enum ResamplingMode {
    None,
    BestQuality,      // Linear phase, high CPU
    LowLatency,       // Minimum phase, low CPU
    BalancedHQ,       // Middle ground
}

impl HighQualityResampler {
    pub fn new(input_rate: u32, output_rate: u32, mode: ResamplingMode) -> Self {
        if input_rate == output_rate || matches!(mode, ResamplingMode::None) {
            return Self {
                resampler: None,
                input_rate,
                output_rate,
                mode,
            };
        }

        let params = match mode {
            ResamplingMode::BestQuality => {
                SincFixedIn::<f64>::new(
                    output_rate as f64 / input_rate as f64,
                    1.0,  // Max cutoff
                    256,  // Sinc length
                    256,  // Window size
                    2,    // Channels
                ).unwrap()
            }
            ResamplingMode::LowLatency => {
                SincFixedIn::<f64>::new(
                    output_rate as f64 / input_rate as f64,
                    0.95,
                    64,
                    64,
                    2,
                ).unwrap()
            }
            ResamplingMode::BalancedHQ => {
                SincFixedIn::<f64>::new(
                    output_rate as f64 / input_rate as f64,
                    0.98,
                    128,
                    128,
                    2,
                ).unwrap()
            }
            _ => unreachable!(),
        };

        Self {
            resampler: Some(Box::new(params)),
            input_rate,
            output_rate,
            mode,
        }
    }
}
```

**UI Elements:**
- Output sample rate selector (48k, 96k, 192k, 384k)
- Resampling quality preset
- CPU usage indicator
- Latency impact display

**Rust Crates:**
- `rubato = "0.15"` (high-quality resampler)
- OR `soxr = "0.3"` (libsoxr bindings)

**Estimated Effort:** 2-3 weeks

---

### **Phase 3: Advanced Features** (v1.0.0+)
*Target: 6+ months | Priority: LOW-MEDIUM | Risk: HIGH*

Major architectural additions requiring significant investment.

#### 3.1 Room Correction / Convolution Engine
**Goal:** Load and apply FIR impulse responses for room EQ

**Implementation:**
```rust
// crates/stream-server/src/dsp/convolution.rs
use rustfft::{FftPlanner, num_complex::Complex};

pub struct ConvolutionEngine {
    impulse: Vec<f64>,
    fft_size: usize,
    overlap_buffer: Vec<f64>,
    fft_planner: FftPlanner<f64>,
    ir_fft: Vec<Complex<f64>>,
}

impl ConvolutionEngine {
    pub fn load_impulse_response(&mut self, path: &Path) -> Result<()> {
        // Load WAV file
        let mut reader = hound::WavReader::open(path)?;
        let samples: Vec<f64> = reader.samples::<i32>()
            .map(|s| s.unwrap() as f64 / i32::MAX as f64)
            .collect();

        // Validate
        if samples.len() > 65536 {
            return Err(anyhow!("Impulse response too long (max 65536 samples)"));
        }

        // Pre-compute FFT of impulse response
        self.compute_ir_fft(&samples);
        self.impulse = samples;

        Ok(())
    }

    pub fn process_block(&mut self, input: &[f64], output: &mut [f64]) {
        // Overlap-add convolution in frequency domain
        // ... FFT → multiply → IFFT → overlap-add
    }
}
```

**UI Elements:**
- "Load Impulse Response" button (.wav, .cfg)
- IR file browser/manager
- Pre/post convolution spectrum display
- Per-profile IR assignment

**Rust Crates:**
- `hound = "3.5"` (WAV reading)
- `rustfft` (already in project)

**Integration Points:**
- REW (Room EQ Wizard) export compatibility
- Dirac Live room correction import

**Estimated Effort:** 4-6 weeks

---

#### 3.2 Advanced Filter Design
**Goal:** User-selectable upsampling/antialiasing filters

**Implementation:**
```rust
// crates/stream-server/src/dsp/filters.rs
pub enum FilterType {
    LinearPhase,      // Symmetric FIR, pre/post ringing
    MinimumPhase,     // Asymmetric FIR, minimal pre-ring
    Apodizing,        // Reduces pre-ringing artifacts
    ShortSinc,        // Low latency, slight roll-off
    LongSinc,         // Steep roll-off, high latency
}

pub struct FilterDesigner {
    taps: usize,
    cutoff: f64,
    transition_band: f64,
}

impl FilterDesigner {
    pub fn design_fir(&self, filter_type: FilterType) -> Vec<f64> {
        match filter_type {
            FilterType::LinearPhase => self.design_linear_phase(),
            FilterType::MinimumPhase => self.design_minimum_phase(),
            // ... implementation via Parks-McClellan or windowed sinc
        }
    }
}
```

**UI Elements:**
- Filter type dropdown in DSP tab
- Frequency response plot
- Impulse response plot
- Phase response toggle

**Rust Crates:**
- Consider `fir = "0.6"` or custom implementation

**Estimated Effort:** 6-8 weeks

---

#### 3.3 AAEQ Network Audio Adapter (NAA) Protocol
**Goal:** Custom low-latency Rust-to-Rust streaming

**Implementation:**
```rust
// crates/aaeq-naa (new crate)
pub struct AaeqNaaServer {
    bind_addr: SocketAddr,
    codec: AudioCodec,
    buffer_ms: u32,
}

pub enum AudioCodec {
    PCM { sample_rate: u32, bit_depth: u8 },
    Flac { compression: u8 },
}

impl AaeqNaaServer {
    pub async fn start(&mut self) -> Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;

        loop {
            let (stream, addr) = listener.accept().await?;
            tracing::info!("NAA client connected: {}", addr);

            tokio::spawn(self.handle_client(stream));
        }
    }

    async fn handle_client(&self, mut stream: TcpStream) -> Result<()> {
        // Custom protocol:
        // 1. Handshake (capabilities negotiation)
        // 2. Format setup (sample rate, channels, codec)
        // 3. Streaming loop (minimal overhead)
    }
}
```

**Protocol Design:**
```
AAEQ NAA Protocol v1.0

Handshake:
  Client → Server: AAEQ_NAA\n<version>\n<capabilities>
  Server → Client: OK\n<format>\n<buffer_size>

Stream Frame:
  [4 bytes] timestamp (ms)
  [2 bytes] payload size
  [N bytes] audio data (PCM/FLAC)
```

**Estimated Effort:** 8-12 weeks (full protocol design + implementation)

---

## 🛠️ Technical Dependencies

### New Rust Crates Required

| Crate | Purpose | Priority | Version |
|-------|---------|----------|---------|
| `rubato` | High-quality resampling | Phase 2 | 0.15 |
| `hound` | WAV file I/O | Phase 3 | 3.5 |
| `rand` | Dithering RNG | Phase 2 | 0.8 (already in) |
| `fir` | FIR filter design | Phase 3 | 0.6 (optional) |

### Architecture Changes

**Phase 1:** Minimal (add views, extend settings)
**Phase 2:** Moderate (new DSP stages in pipeline)
**Phase 3:** Major (new crates, protocol design)

---

## 📈 Success Metrics

### User Engagement
- Time spent in DSP view vs. simple mode
- Number of custom profiles created per user
- Feature adoption rates (convolution, resampling, etc.)

### Performance
- CPU usage remains < 20% on typical hardware
- Latency stays < 200ms for local DAC
- No audio dropouts under load

### Quality
- Measured THD+N improvement with dithering
- User-reported sound quality improvements
- A/B testing vs. HQPlayer (subjective)

---

## 🚧 Known Challenges

### Challenge 1: CPU Performance
**Risk:** Convolution + upsampling + EQ = high CPU load

**Mitigation:**
- Profile code paths with `criterion`
- Offer "Quality" vs "Performance" presets
- Use SIMD where possible (`std::simd`)
- Offload FFT to GPU (future)

### Challenge 2: Complexity Creep
**Risk:** AAEQ becomes as complex as HQPlayer

**Mitigation:**
- Keep "Simple Mode" as default
- Progressive disclosure (hide advanced options)
- Maintain clean codebase with modular DSP stages

### Challenge 3: Testing Audio Quality
**Risk:** Hard to objectively test improvements

**Mitigation:**
- Automated THD+N measurements
- Reference test signals (1 kHz tone, sweep)
- User A/B testing surveys
- Collaborate with audiophile community

---

## 📝 Documentation Needs

### User-Facing
- [ ] DSP pipeline explanation (what each stage does)
- [ ] Filter types guide (when to use each)
- [ ] Room correction tutorial (REW integration)
- [ ] Troubleshooting guide (latency, dropouts)

### Developer-Facing
- [ ] DSP architecture diagram
- [ ] Adding new DSP stages (plugin API?)
- [ ] Testing audio processing code
- [ ] Contribution guidelines

---

## 🎓 Learning Resources

### DSP Theory
- *Digital Signal Processing* by Oppenheim & Schafer
- *Understanding Digital Signal Processing* by Lyons
- Julius O. Smith's DSP online books

### Rust Audio
- `cpal` documentation (audio I/O)
- `rustfft` examples
- `dasp` ecosystem tour

### Room Correction
- REW documentation
- Dirac Live white papers
- Bob Katz mastering articles

---

## 🗺️ Milestone Summary

### v0.6.0 - "Visual Insight"
- Pipeline visualization
- Headroom control
- Enhanced spectrum analyzer

### v0.7.0 - "Device Intelligence"
- Device-aware profiles
- DSP templates library
- Per-profile settings

### v0.8.0 - "Precision Audio"
- Dithering & noise shaping
- High-rate resampling
- Quality modes

### v0.9.0 - "Advanced DSP"
- Filter design options
- Room correction (basic)

### v1.0.0 - "Mastering Suite"
- Full convolution engine
- AAEQ NAA protocol
- Complete HQPlayer feature parity (where relevant)

---

## 🤝 Community Involvement

### Open Questions for Users
1. Which features are most important? (Survey)
2. What DACs/systems are you using? (Hardware survey)
3. Would you use room correction? (Interest poll)

### Beta Testing Program
- Early access to Phase 2 features
- Feedback on CPU usage and quality
- A/B comparison with HQPlayer

### Contribution Opportunities
- DSP filter implementations
- UI/UX improvements
- Documentation and tutorials
- Platform-specific testing

---

## 📅 Release Schedule (Tentative)

| Version | Target Date | Focus |
|---------|------------|-------|
| v0.6.0 | Q1 2025 | Visualization & Headroom |
| v0.7.0 | Q2 2025 | Device Intelligence |
| v0.8.0 | Q3 2025 | DSP Quality |
| v0.9.0 | Q4 2025 | Advanced Features |
| v1.0.0 | Q1 2026 | Mastering Suite |

---

## 🎯 Conclusion

This roadmap balances:
- **Quick wins** (Phase 1) that add value immediately
- **Quality improvements** (Phase 2) that differentiate AAEQ
- **Advanced features** (Phase 3) that approach HQPlayer territory

**Core principle:** Never lose sight of AAEQ's unique strength - **intelligent automation**. Every feature should serve the goal of making high-quality audio processing effortless.

---

*This is a living document. Priorities may shift based on user feedback, technical discoveries, and market changes.*
