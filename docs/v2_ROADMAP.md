# AAEQ Stream Server — Technical Spec (v0.2)

## 1) Purpose

Turn AAEQ into a **universal, lossless “smart EQ pre-amp”**: take 64-bit float audio from the DSP core and **output** it to:

* Local DACs (CoreAudio / WASAPI / ALSA)
* Network streamers via **UPnP/DLNA (PCM)**
* **AirPlay 2** (ALAC 16/44.1–48 as a pragmatic fallback)
* (Optional) **HQPlayer NAA** / **RTP PCM (AES67-style)**

All paths preserve fidelity (EQ in 64-bit float → dithered 24-bit PCM out, unless format constrained).

---

## 2) Architecture Overview

```
AAEQ Player(s) / System Mix
        │
        ▼
┌───────────────────────────┐
│  AAEQ DSP Core (64f PCM)  │  ← GEQ/PEQ/AAEQ Suggest, limiter, pre-gain
└───────────┬───────────────┘
            │ frames (interleaved/stereo)
            ▼
┌───────────────────────────┐
│   Stream Server (Rust)    │
│  • OutputManager          │
│  • Latency/Jitter Buffer  │
│  • Resampler + Dither     │
│  • Protocol Adapters      │───► UPnP/DLNA  (PCM)
│                           │───► AirPlay 2  (ALAC 16/44.1–48)
│                           │───► Local DAC  (CoreAudio/WASAPI/ALSA)
│                           │───► NAA/RTP    (Optional)
└───────────────────────────┘
```

---

## 3) Core Traits & Data Types (Rust)

```rust
/// Interleaved stereo 64f frames from DSP
pub struct AudioBlock<'a> {
    pub frames: &'a [f64],  // len = n_frames * n_channels
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Copy, Debug)]
pub enum SampleFormat { F64, F32, S24LE, S16LE }

#[derive(Clone, Debug)]
pub struct OutputConfig {
    pub sample_rate: u32,         // target (e.g., 48000)
    pub channels: u16,            // usually 2
    pub format: SampleFormat,     // e.g., S24LE for PCM
    pub buffer_ms: u32,           // network jitter buffer (e.g., 150)
    pub exclusive: bool,          // WASAPI/CA exclusive if available
}

#[async_trait::async_trait]
pub trait OutputSink: Send + Sync {
    fn name(&self) -> &'static str;
    async fn open(&mut self, cfg: OutputConfig) -> anyhow::Result<()>;
    async fn write(&mut self, block: AudioBlock<'_>) -> anyhow::Result<()>;
    async fn drain(&mut self) -> anyhow::Result<()>;
    async fn close(&mut self) -> anyhow::Result<()>;

    /// Report end-to-end latency in ms (buffering + device/protocol)
    fn latency_ms(&self) -> u32;
}

/// Managed by OutputManager; adapters implement OutputSink.
pub struct OutputManager {
    sinks: Vec<Box<dyn OutputSink>>,
    active: Option<usize>,
}
```

**Adapters to implement:**

* `sink_local_coreaudio` (macOS), `sink_local_wasapi` (Windows), `sink_local_alsa` (Linux)
* `sink_dlna_pcm`
* `sink_airplay2`
* `sink_naa` / `sink_rtp` (feature-gated)

---

## 4) Buffering, Resampling, Dither

* **Incoming:** 64-bit float from DSP.
* **Resampling:** only when required by target (`rubato`/`speexdsp` high-quality SRC).
* **Bit-depth conversion:** 64f → 24-bit PCM (default) with **TPDF dither**.
* **Headroom:** DSP core maintains −3 dB pre-gain; optional soft limiter.
* **Jitter/latency buffer:** target 100–250 ms for network sinks; <20–30 ms for local DACs.

---

## 5) Protocol Adapters

### 5.1 UPnP/DLNA (PCM)

* **Control:** advertise as a **Renderer** and/or act as a **MediaServer + HTTP WAV stream**; many streamers (WiiM/Bluesound/HEOS) can pull PCM from a URL.
* **Transport:** serve **chunked WAV/RAW PCM** over HTTP; announce via DIDL-Lite.
* **Seek:** optional; AAEQ primarily does live streaming.
* **Libs:** `gupnp` (via FFI), `libupnp-sys`, or `gstreamer-rs` (RTSP/RTP L16).

**Modes:**

* **Push:** AAEQ drives renderer via AVTransport → set URI ([http://aaeq.local/stream](http://aaeq.local/stream)).
* **Pull:** Renderer pulls from AAEQ’s HTTP stream (most robust).

### 5.2 AirPlay 2 (ALAC 16/44.1–48)

* **Codec:** ALAC is **lossless** but capped at 16-bit; good universal fallback.
* **Latency:** ~2s; not for live mixing, fine for listening.
* **Implementation:** integrate `shairport-sync` style sender or `airplay2-rs` where feasible.

### 5.3 Local DACs

* **macOS:** CoreAudio HAL; support shared & exclusive; expose device selector.
* **Windows:** WASAPI shared/exclusive; enable loopback **or** direct output.
* **Linux:** ALSA or (better) PipeWire node; optional creation of “AAEQ Sink” for system routing.

### 5.4 (Optional) NAA / RTP (AES67-style)

* For pro/audiophile users. Implement **RTP L16/L24** with PTP clock drift handling or integrate HQPlayer NAA client.

---

## 6) Control API (local HTTP)

Expose a **local REST/gRPC** control surface for UI and CLI:

```
GET  /v1/outputs                 # list adapters & status
POST /v1/outputs/select          # { "name": "dlna", "device": "WiiM Ultra", "cfg": {...} }
POST /v1/outputs/start           # begin streaming
POST /v1/outputs/stop
GET  /v1/outputs/metrics         # latency, underruns, SR, format

POST /v1/route                   # { "input": "SystemMix|App|File", "output": "dlna|dac|airplay", "device": "…" }
GET  /v1/route

GET  /v1/capabilities            # supported SR/formats per adapter
```

**Security:** bind to `127.0.0.1` by default; optional token; no remote control unless explicitly enabled.

---

## 7) Configuration (TOML)

```toml
[stream]
default_output = "dlna"
target_sample_rate = 48000
target_format = "S24LE"
buffer_ms = 150

[dlna]
announce_name = "AAEQ Renderer"
http_bind = "0.0.0.0:8090"

[airplay]
enable = true

[dac]
exclusive = true
device_preference = "Topping D90SE"
```

---

## 8) UX Hooks

* **Output selector** (dropdown): Local DAC / UPnP / AirPlay / NAA
* **Device picker** (contextual): list discovered renderers/DACs
* **Format display:** `48 kHz · 24-bit PCM · 140 ms`
* **Test tone** & **latency meter**
* **Quick switch** remembers per-device configs

---

## 9) Fidelity & Safety Rules

* Always process in **64f** internally.
* Downstream conversions: **dither** when reducing bit depth.
* Keep **pre-gain −3 dB** by default; limiter opt-in for Simple mode, opt-out available in Advanced mode.
* **No lossy codecs** except AirPlay’s ALAC (still lossless) and only when chosen.

---

## 10) Testing Plan

* **Unit:** sample-rate/format conversions, dither, buffer math.
* **Integration:** loopback self-tests (inject tone → capture at renderer).
* **Device matrix:** WiiM, Bluesound, HEOS, Sonos (UPnP quirks), several USB DACs.
* **Stress:** long playback, SR changes, network jitter simulation.
* **QA Metrics:** underruns/overflows, measured latency, drift correction.

---

## 11) Milestones

* **M1:** Local DAC sink (CoreAudio/WASAPI/ALSA) + stable buffering/dither.
* **M2:** DLNA/UPnP HTTP PCM streaming (pull mode) + device discovery.
* **M3:** AirPlay 2 sender integration.
* **M4:** Output Manager UI + local HTTP control API.
* **M5:** Optional NAA/RTP output + clock sync improvements.

---

## 12) Libraries (suggested)

* **Audio I/O:** `cpal`, `coreaudio-sys`, `wasapi`, `alsa`, `pipewire`
* **DSP:** `biquad`, `realfft`, `rubato`/`speexdsp`
* **Networking:** `hyper`, `axum`, `tokio`
* **UPnP/DLNA:** `libupnp-sys` (FFI) or `gstreamer-rs` RTP/RTSP pipeline
* **AirPlay 2:** `shairport-sync` integration or `airplay2-rs` (where license permits)

---

### TL;DR

This spec gives AAEQ a clean **OutputSink** interface, high-fidelity conversion rules, robust buffering, and multiple **lossless transports**. Start with **Local DAC** (M1), then **DLNA PCM** (M2) to reach most streamers, and add **AirPlay 2** as a practical universal fallback.
