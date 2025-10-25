# How It Works

## EQ Management Mode

AAEQ polls your WiiM device every second to check what's currently playing. When a track changes, it:

1. Checks for a **song-specific** mapping (`Artist - Title`)
2. Falls back to **album mapping** (`Artist - Album`)
3. Falls back to **genre mapping** (if genre is set)
4. Falls back to **default preset** (usually "Flat")

The resolved preset is only applied if it's different from the currently active one, preventing unnecessary device commands.

## DSP Streaming Mode

In DSP mode, AAEQ processes audio through a professional-grade signal chain:

1. **INPUT** - Captures audio from your selected input device (microphone, line-in, or system audio loopback)
2. **HEADROOM** - Applies configurable headroom reduction (e.g., -3 dB) with clip detection
3. **EQ** - Applies 10-band parametric EQ with custom or built-in presets
4. **RESAMPLE** (optional) - High-quality sinc-based sample rate conversion
   - Four quality presets: Fast, Balanced, High, Ultra
   - Supports common sample rates: 44.1kHz, 48kHz, 88.2kHz, 96kHz, 192kHz
5. **DITHER** (optional) - Professional dithering and noise shaping for bit depth reduction
   - Four dither modes: None, Rectangular, TPDF (Triangular), Gaussian
   - Four noise shaping algorithms: None, First Order, Second Order, Gesemann
   - Configurable target bit depth (8-24 bits)
6. **OUTPUT** - Streams to network device via DLNA/UPnP protocol, or outputs to local DAC

### Additional Features

- **Visual Pipeline Display** - Real-time status of all processing stages with clickable controls
- **Profile-Based Settings** - Each profile can have unique DSP settings (different headroom, resampling, dithering per profile)
- **Cross-Platform Now Playing** - Detects currently playing media on Linux (MPRIS), Windows (SMTC), and macOS (system-wide with `nowplayingctl`)
- **Automatic EQ Mapping** - Applies EQ presets based on detected track (same mapping logic as WiiM API mode)

The DSP mode works independently of WiiM devices - you can stream any audio source to any DLNA-compatible device with professional-quality processing.
