# 🎧 AAEQ Node Protocol (ANP) — Draft v0.4 Specification

**High-fidelity, low-latency, bit-perfect network audio protocol optimized for AAEQ's DSP pipeline and streaming service integration.**

**Last Updated:** 2025-10-26  
**Version:** 0.4.0  
**Status:** Ready for Implementation

---

## 1. Protocol Version

```json
{ "v": "0.4" }
```

### 1.1 Changes from v0.3

**Major Additions:**
- Volume control commands with dB mapping and curve definitions
- Node capability negotiation with persistent UUID
- Enhanced error recovery with standardized error codes
- Detailed playback state reporting with state machine
- DSP acknowledgment protocol
- Latency breakdown telemetry
- RTP header extensions for gapless and CRC
- Micro-PLL parameter specification
- Buffer contract and start threshold

**Deferred to Future Versions:**
- DSD support (v0.5+)
- SRTP encryption (v0.5+)
- PTP clock synchronization (v0.5+)

**Backward Compatibility:** v0.4 maintains wire compatibility with v0.3 for core features (Micro-PLL, CRC, basic streaming). New features use optional flags.

### 1.2 Version Compatibility

ANP follows semantic versioning in development:
- **v0.x**: Breaking changes allowed (pre-1.0)
- **v1.0.0+**: Semantic versioning (major.minor.patch)

**Compatibility Rules:**
- Nodes MUST reject connections from incompatible server versions
- Servers SHOULD support at least 1 previous minor version
- Feature negotiation allows graceful degradation

**Example:**
- v0.4 node + v0.4 server: Full compatibility ✅
- v0.3 node + v0.4 server: Works with v0.3 features only ⚠️
- v0.4 node + v0.3 server: Node should detect and downgrade ⚠️

---

## 2. Core Features in v0.4

| Feature | Purpose | Negotiation Flag | Priority |
|---------|---------|------------------|----------|
| **Micro-PLL** | Clock drift correction via resampling | `micro_pll` | Core |
| **CRC Verify** | Bit-perfect verification | `crc_verify` | Core |
| **Volume Control** | Remote volume adjustment | `volume_control` | Core |
| **Gapless Playback** | Seamless track transitions | `gapless` | Core |
| **Node Capabilities** | DAC info, supported formats | `capabilities` | Core |
| **Health Telemetry** | Monitoring & diagnostics | (always on) | Core |
| **Latency Calibration** | Sample-accurate timing | `latency_cal` | Core |

**Optional Features (Negotiated):**
| Feature | Purpose | Negotiation Flag | Priority |
|---------|---------|------------------|----------|
| **DSP Transfer** | Server pushes DSP state to Node | `dsp_transfer` | Optional |
| **Convolution** | Room correction (IRs) | `convolution` | Optional |
| **RTCP SR** | Sender reports for QoS | `rtcp_sr` | Optional |

**Future Features (v0.5+):**
| Feature | Purpose | Negotiation Flag | Version |
|---------|---------|------------------|---------|
| DSD Support | Native DSD streaming | `dsd` | v0.5+ |
| SRTP Encryption | Secure streaming | `srtp_aes` | v0.5+ |
| PTP Clock Sync | Precision timing | `ptp_sync` | v0.5+ |
| Multi-room Sync | Synchronized playback | `multiroom` | v0.5+ |

---

## 3. Discovery (mDNS TXT)

Nodes advertise themselves via mDNS with the following TXT record:

```
_aaeq-anp._tcp.local.
```

### 3.1 TXT Record Format (Compact)

**Recommended key order** (uuid first for truncation resilience):

```
uuid=550e8400-e29b-41d4-a716-446655440000
v=0.4.0
sr=44100,48000,96000,192000
bd=S16,S24,F32
ch=2
ft=pll,crc,vol,gap,cap
opt=dsp,conv
ctrl=wss://10.0.0.10:7443
st=idle
vol=75
dac=HiFiBerry DAC+
hw=RPi4
```

### 3.2 Field Descriptions

| Field | Description | Example | Max Length |
|-------|-------------|---------|------------|
| `uuid` | Persistent node UUID (FIRST for resilience) | `550e8400-...` | 36 |
| `v` | Protocol version (semantic) | `0.4.0` | 8 |
| `sr` | Supported sample rates (Hz) | `44100,48000,96000` | 64 |
| `bd` | Supported formats (abbreviated) | `S16,S24,F32` | 32 |
| `ch` | Number of channels | `2` | 2 |
| `ft` | Core features (abbreviated) | `pll,crc,vol,gap` | 64 |
| `opt` | Optional features | `dsp,conv` | 64 |
| `ctrl` | WebSocket control URL | `wss://...` | 128 |
| `st` | Current state | `idle`, `play`, `buf`, `err` | 8 |
| `vol` | Current volume (0-100) | `75` | 3 |
| `dac` | DAC/device name | `HiFiBerry DAC+` | 32 |
| `hw` | Hardware platform (abbreviated) | `RPi4` | 16 |

**Feature Abbreviations:**
- `pll` = micro_pll
- `crc` = crc_verify
- `vol` = volume_control
- `gap` = gapless
- `cap` = capabilities
- `dsp` = dsp_transfer
- `conv` = convolution
- `rtcp` = rtcp_sr

**State Abbreviations:**
- `idle` = idle
- `play` = playing/streaming
- `buf` = buffering
- `err` = error

**Hardware Abbreviations:**
- `RPi4` = Raspberry Pi 4
- `RPi5` = Raspberry Pi 5
- `RPi3` = Raspberry Pi 3
- `x64` = x86_64 desktop

### 3.3 UUID Generation

**Node UUID MUST be:**
- Persistent across reboots
- Unique per physical device
- Generated once at first boot

**Recommended generation methods:**
1. Derive from MAC address (stable, hardware-based)
2. Generate random UUID and store in config file
3. Use hardware serial number if available

**Example (Rust):**
```rust
use uuid::Uuid;

fn get_or_create_node_uuid() -> Uuid {
    // Try to load from config
    if let Some(uuid) = load_uuid_from_config() {
        return uuid;
    }
    
    // Generate from MAC address (deterministic)
    if let Some(mac) = get_primary_mac_address() {
        return Uuid::new_v5(&Uuid::NAMESPACE_OID, mac.as_bytes());
    }
    
    // Last resort: generate random and save
    let uuid = Uuid::new_v4();
    save_uuid_to_config(&uuid);
    uuid
}
```

---

## 4. Extended Session Negotiation

### 4.1. session_init (Node → Server)

Node proposes desired features and reports capabilities.

**Message Type:** `session_init`

```json
{
  "session_init": {
    "protocol_version": "0.4",
    "node_uuid": "550e8400-e29b-41d4-a716-446655440000",
    "features": ["micro_pll", "crc_verify", "volume_control", "gapless", "capabilities"],
    "optional_features": ["dsp_transfer"],
    "latency_comp": true,
    
    "node_capabilities": {
      "hardware": "Raspberry Pi 4 Model B",
      "dac_name": "HiFiBerry DAC+ Pro",
      "dac_chip": "PCM5122",
      "max_sample_rate": 192000,
      "supported_formats": ["F32", "S24LE", "S16LE"],
      "native_format": "S24LE",
      "max_channels": 2,
      "buffer_range_ms": [50, 500],
      "has_hardware_volume": true,
      "volume_range": [0.0, 1.0],
      "volume_curve": "logarithmic",
      "cpu_info": {
        "arch": "ARMv8",
        "cores": 4,
        "freq_mhz": 1500
      },
      "dsp_capabilities": {
        "can_eq": false,
        "can_resample": false,
        "can_convolve": false
      }
    }
  }
}
```

**New Fields:**
- **`node_uuid`**: Persistent unique identifier for this node
- **`volume_range`**: Normalized control range [0.0, 1.0] regardless of hardware DAC capabilities
- **`volume_curve`**: Supported volume curve type(s)

**Note on `volume_range`:**  
For hardware DACs that report a dB range (e.g., -60 dB to 0 dB), the Node SHOULD normalize this to [0.0, 1.0] in the control plane but apply the real dB values internally. This ensures consistent volume control across different DAC types.

### 4.2. session_accept (Server → Node)

Server confirms active features and provides configuration.

**Message Type:** `session_accept`

```json
{
  "session_accept": {
    "protocol_version": "0.4",
    "session_id": "srv-1234567890",
    "active_features": ["micro_pll", "crc_verify", "volume_control", "gapless"],
    "optional_features": [],
    
    "rtp_config": {
      "ssrc": 305419896,
      "payload_type": 96,
      "timestamp_rate": 48000,
      "initial_sequence": 0,
      "initial_timestamp": 0
    },
    
    "rtp_extensions": {
      "gapless": {
        "enabled": true,
        "extension_id": 1
      },
      "crc32": {
        "enabled": true,
        "extension_id": 2,
        "window": 64
      }
    },
    
    "recommended_config": {
      "sample_rate": 48000,
      "format": "S24LE",
      "buffer_ms": 150,
      "reason": "Optimal for your hardware and network"
    },
    
    "latency": {
      "dac_ms": 1.34,
      "pipeline_ms": 0.62,
      "comp_mode": "exact"
    },
    
    "micro_pll": {
      "enabled": true,
      "ppm_limit": 150,
      "adjustment_interval_ms": 100,
      "slew_rate_ppm_per_sec": 10,
      "ema_window": 8
    },
    
    "volume": {
      "initial_level": 0.75,
      "mute": false,
      "control_mode": "software",
      "curve_type": "logarithmic"
    },
    
    "buffer": {
      "target_ms": 150,
      "min_ms": 50,
      "max_ms": 500,
      "start_threshold_ms": 100
    }
  }
}
```

**New in v0.4:**
- **`session_id`**: Unique session identifier for logging/debugging
- **`rtp_config`**: Complete RTP stream parameters
- **`rtp_extensions`**: Negotiated RTP extensions with IDs
- **`micro_pll`**: Detailed PLL parameters
- **`buffer`**: Buffer management parameters

**RTP Extension Negotiation:**  
Servers MUST only send RTP header extensions that have been negotiated in `session_accept.rtp_extensions`. Nodes MUST ignore or drop packets with unknown extension IDs to prevent interoperability issues.

---

## 5. Volume Control

Critical for streaming service use case. All volume commands are sent over WebSocket control channel.

### 5.1. Volume Curve Definition

ANP supports three standard volume curves:

#### Linear Curve
```
volume_linear = level
gain_db = 20 * log10(volume_linear)
```

**Range:** -∞ dB (0.0) to 0 dB (1.0)

#### Logarithmic Curve (Recommended)
```
if level == 0.0:
    gain_db = -∞ (mute)
else:
    gain_db = 40 * log10(level)
```

**Range:** -∞ dB (0.0) to 0 dB (1.0)  
**Characteristic:** More natural perceived volume, better control at low levels  
**Math Note:** The factor of 40 provides intuitive dB scaling: 0.1 → -40 dB, 0.5 → -12 dB, 1.0 → 0 dB

#### Exponential Curve
```
gain_db = 60 * (level - 1.0)
```

**Range:** -60 dB (0.0) to 0 dB (1.0)  
**Characteristic:** Linear dB change, constant perceived change

### 5.2. Volume-to-dB Mapping Table (Logarithmic)

| Level | Gain (dB) | Perceived | Use Case |
|-------|-----------|-----------|----------|
| 0.00 | -∞ | Silent | Mute |
| 0.01 | -80 dB | Barely audible | |
| 0.10 | -40 dB | Very quiet | Night listening |
| 0.25 | -24 dB | Quiet | Background |
| 0.50 | -12 dB | Moderate | Normal |
| 0.75 | -5.1 dB | Loud | Preferred |
| 0.90 | -1.9 dB | Very loud | Party |
| 1.00 | 0 dB | Maximum | Unity gain |

### 5.3. Volume Ramp Shapes

When `ramp_ms` is specified, volume changes follow one of these ramp shapes:

#### Linear Ramp (Default)
```
current_level(t) = start_level + (target_level - start_level) * (t / ramp_ms)
```

#### S-Curve Ramp (Smooth)
```
progress = t / ramp_ms
s_curve = 3*progress^2 - 2*progress^3  // Smoothstep
current_level(t) = start_level + (target_level - start_level) * s_curve
```

#### Exponential Ramp (Natural)
```
tau = ramp_ms / 5
current_level(t) = target_level + (start_level - target_level) * exp(-t / tau)
```

**Node MUST implement at least linear ramp.**  
**Node SHOULD implement S-curve for better UX.**

### 5.4. volume_set

Set volume level and mute state.

**Message Type:** `volume_set`

```json
{
  "volume_set": {
    "level": 0.75,
    "mute": false,
    "ramp_ms": 100,
    "ramp_shape": "s_curve"
  }
}
```

**Fields:**
- **`level`**: Volume level (0.0 = silence, 1.0 = unity gain)
- **`mute`**: Mute state (true/false)
- **`ramp_ms`**: Optional fade time in milliseconds (0 = instant)
- **`ramp_shape`**: Optional: `linear`, `s_curve`, `exponential` (default: `linear`)

**Response:**

**Message Type:** `volume_result`

```json
{
  "volume_result": {
    "status": "success",
    "level": 0.75,
    "mute": false,
    "gain_db": -5.1,
    "ramp_complete": false
  }
}
```

### 5.5. volume_get

Query current volume state.

**Message Type:** `volume_get`

```json
{
  "volume_get": {}
}
```

**Response:**

**Message Type:** `volume_result`

```json
{
  "volume_result": {
    "level": 0.75,
    "mute": false,
    "hardware_control": true,
    "dac_volume_db": -12.5,
    "gain_db": -5.1,
    "curve_type": "logarithmic"
  }
}
```

**Fields:**
- **`level`**: Current volume (0.0-1.0, normalized)
- **`mute`**: Current mute state
- **`hardware_control`**: True if using DAC hardware volume
- **`dac_volume_db`**: Actual DAC volume in dB (if hardware control)
- **`gain_db`**: Calculated gain based on curve and level
- **`curve_type`**: Active volume curve

### 5.6. Volume Control Modes

**Software Volume (Default):**
- Node applies volume in software (multiply samples by gain)
- No quality loss with F32/S24LE formats
- Works with all DACs

**Hardware Volume (Preferred):**
- Uses DAC's hardware volume control (ALSA mixer)
- Better SNR and dynamic range
- Requires DAC support (e.g., HiFiBerry, IQaudIO)
- Node normalizes hardware dB range to [0.0, 1.0] for control plane

**Auto Mode:**
- Node uses hardware volume if available
- Falls back to software volume

---

## 6. RTP Transport Specification

### 6.1. RTP Header Structure

Standard RTP header with ANP-specific payload types:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|V=2|P|X|  CC   |M|     PT      |       sequence number         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           timestamp                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           synchronization source (SSRC) identifier            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Field Values:**
- **V** (Version): 2
- **P** (Padding): 0 (no padding)
- **X** (Extension): 1 if using header extensions, 0 otherwise
- **CC** (CSRC Count): 0 (no contributing sources)
- **M** (Marker): 0 (reserved for future use)
- **PT** (Payload Type): See payload type table below
- **Sequence Number**: Starts at value from `session_accept.rtp_config.initial_sequence`, increments by 1 per packet
- **Timestamp**: Audio sample timestamp (see section 6.2)
- **SSRC**: Synchronization source identifier from `session_accept.rtp_config.ssrc`

### 6.2. Timestamp Rules

**Timestamp Rate:**
- MUST equal the sample rate (e.g., 48000 for 48kHz audio)
- Specified in `session_accept.rtp_config.timestamp_rate`

**Timestamp Base:**
- Starts at value from `session_accept.rtp_config.initial_timestamp` (typically 0)
- Increments by number of frames (not samples) in packet
- Example: For stereo packet with 480 frames → timestamp += 480

**Timestamp Calculation:**
```
frames_in_packet = payload_bytes / (channels * bytes_per_sample)
next_timestamp = current_timestamp + frames_in_packet
```

**Wraparound:**
- Timestamp is 32-bit unsigned, wraps at 2^32
- Node MUST handle wraparound correctly
- At 48kHz: wraps every ~24.8 hours

### 6.3. SSRC Rules

**SSRC Assignment:**
- Server generates SSRC (32-bit identifier)
- Sent in `session_accept.rtp_config.ssrc`
- MUST be unique per session
- SHOULD be derived from server instance + timestamp

**SSRC Generation (recommended):**
```rust
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

fn generate_ssrc(server_id: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    server_id.hash(&mut hasher);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .hash(&mut hasher);
    hasher.finish() as u32
}
```

**SSRC Conflict:**
- If Node detects SSRC conflict (multiple streams with same SSRC), it MUST report error E205
- Server SHOULD regenerate SSRC and restart session

### 6.4. Payload Types

| PT | Format | Description | Bits | Byte Order |
|----|--------|-------------|------|------------|
| 96 | L24 | 24-bit Linear PCM | 24 | Network (big-endian) |
| 97 | L16 | 16-bit Linear PCM | 16 | Network (big-endian) |
| 10 | L16 | 16-bit Linear PCM (standard) | 16 | Network (big-endian) |
| 11 | L16 | 16-bit Linear PCM Stereo (standard) | 16 | Network (big-endian) |

**Note:** PT 10/11 are standard RTP audio types for compatibility.  
**Recommended:** Use PT 96 (L24) for high-quality, PT 97 (L16) for compatibility.

**Critical Endianness Note:**  
For **S24LE payloads** (PT 96), each audio sample is transmitted in **network byte order (big-endian)** regardless of host endianness. Implementers on little-endian systems (e.g., Raspberry Pi, x86) MUST perform byte swapping when packing/unpacking samples to/from RTP payloads.

**Example (Rust):**
```rust
// Packing S24LE sample to network byte order
fn pack_s24le_sample(sample: i32) -> [u8; 3] {
    let clamped = sample.clamp(-8388608, 8388607); // 24-bit range
    let bytes = clamped.to_be_bytes(); // Big-endian (network order)
    [bytes[1], bytes[2], bytes[3]] // Take lower 3 bytes
}

// Unpacking S24LE sample from network byte order
fn unpack_s24le_sample(bytes: &[u8; 3]) -> i32 {
    let sign_extended = if bytes[0] & 0x80 != 0 { 0xFF } else { 0x00 };
    i32::from_be_bytes([sign_extended, bytes[0], bytes[1], bytes[2]])
}
```

### 6.5. RTP Header Extensions

ANP defines two header extensions for enhanced functionality, using **RFC 5285 one-byte header format**.

#### Extension Format (RFC 5285)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|      0xBE     |      0xDE     |           length              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  ID   | len   |     data      |  ID   | len   |     data      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Profile:** 0xBEDE (one-byte header extensions, RFC 5285)  
**Length Encoding:** Length field = (actual_data_bytes - 1), range 0-15 represents 1-16 bytes

#### Extension 1: Gapless Playback Markers (ID negotiated)

**Extension ID:** Negotiated in `session_accept.rtp_extensions.gapless.extension_id` (typically 1)

```
 0                   1
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  ID   | len=0 |T|S|  reserved |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Fields:**
- **ID**: From `session_accept.rtp_extensions.gapless.extension_id`
- **len**: 0 (indicates 1 byte of data, per RFC 5285)
- **T** (Track End): 1 = last packet of current track, 0 = normal packet
- **S** (Track Start): 1 = first packet of next track, 0 = normal packet
- **Reserved**: 6 bits, MUST be 0

**Usage:**
```
[Track 1, packet N-1] T=0, S=0
[Track 1, packet N]   T=1, S=0  ← Last packet of Track 1
[Track 2, packet 1]   T=0, S=1  ← First packet of Track 2
[Track 2, packet 2]   T=0, S=0
```

**Node Behavior:**
- When T=1 seen: Prepare for track transition, pre-buffer next track
- When S=1 seen: Begin playback of new track seamlessly
- No gap or click between tracks

#### Extension 2: CRC32 Verification (ID negotiated)

**Extension ID:** Negotiated in `session_accept.rtp_extensions.crc32.extension_id` (typically 2)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  ID   | len=3 |                  CRC32 value                  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Fields:**
- **ID**: From `session_accept.rtp_extensions.crc32.extension_id`
- **len**: 3 (indicates 4 bytes of data, per RFC 5285: len = actual_bytes - 1)
- **CRC32 value**: CRC32 of RTP payload (audio data only), 32-bit big-endian

**CRC32 Algorithm:**
- Polynomial: 0x04C11DB7 (IEEE 802.3)
- Initial value: 0xFFFFFFFF
- Final XOR: 0xFFFFFFFF
- Computed over RTP payload bytes only (not header)
- Transmitted in network byte order (big-endian)

**Example (Rust):**
```rust
use crc32fast::Hasher;

fn compute_payload_crc(payload: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(payload);
    hasher.finalize()
}
```

**Verification Frequency:**
- Server sends CRC32 every N packets (configured in `session_accept.rtp_extensions.crc32.window`)
- Default: every 64 packets
- Node verifies CRC and reports failures in health message

**Node Behavior on CRC Failure:**
- Log error with packet sequence number
- Increment `crc_fail` counter in health
- Continue playback (don't drop packet)
- If `crc_fail_rate > 1%`: report error E306

### 6.6. Packet Size Recommendations

**Recommended packet sizes:**
- 10ms @ 48kHz: 480 frames = 960 samples (stereo) = 2880 bytes (S24LE) + RTP header
- 20ms @ 48kHz: 960 frames = 1920 samples (stereo) = 5760 bytes (S24LE) + RTP header

**Considerations:**
- Smaller packets: Lower latency, higher overhead
- Larger packets: Higher latency, more efficient
- Typical MTU: 1500 bytes (Ethernet) - avoid fragmentation

**Rule of thumb:** Use 10-20ms packets for balance of latency and efficiency.

---

## 7. Micro-PLL Clock Synchronization

### 7.1. PLL Parameters

Micro-PLL corrects clock drift between server and node using adaptive resampling.

**Configuration (from session_accept):**
```json
{
  "micro_pll": {
    "enabled": true,
    "ppm_limit": 150,
    "adjustment_interval_ms": 100,
    "slew_rate_ppm_per_sec": 10,
    "ema_window": 8
  }
}
```

**Parameter Definitions:**

| Parameter | Description | Typical Value | Range |
|-----------|-------------|---------------|-------|
| `ppm_limit` | Maximum correction (parts per million) | 150 | 50-500 |
| `adjustment_interval_ms` | How often to adjust resampling | 100 | 50-500 |
| `slew_rate_ppm_per_sec` | Max rate of change (ppm/sec) | 10 | 1-50 |
| `ema_window` | Exponential moving average window | 8 | 4-16 |

### 7.2. Drift Measurement

**Node measures drift by comparing:**
- Expected buffer fill (based on sample rate)
- Actual buffer fill (measured in samples)

**Drift calculation:**
```
drift_ppm = (actual_buffer_samples - expected_buffer_samples) / expected_buffer_samples * 1e6
```

**Exponential Moving Average (EMA):**
```
alpha = 2.0 / (ema_window + 1)
smoothed_drift = alpha * measured_drift + (1 - alpha) * previous_smoothed_drift
```

**Purpose:** Smooth out jitter and transient variations.

### 7.3. Resampling Adjustment

**When to adjust:**
- Every `adjustment_interval_ms` milliseconds
- Only if `|smoothed_drift| > 1 ppm` (dead zone to prevent hunting)

**Adjustment calculation:**
```
if |smoothed_drift| > ppm_limit:
    // Clamp to prevent excessive correction
    clamped_drift = clamp(smoothed_drift, -ppm_limit, ppm_limit)
else:
    clamped_drift = smoothed_drift

// Apply slew rate limit
drift_delta = clamped_drift - current_adjustment
max_delta = slew_rate_ppm_per_sec * adjustment_interval_ms / 1000.0

if |drift_delta| > max_delta:
    new_adjustment = current_adjustment + sign(drift_delta) * max_delta
else:
    new_adjustment = clamped_drift

// Apply to resampler
resampling_ratio = 1.0 + (new_adjustment / 1e6)
```

**Example:**
```
smoothed_drift = +15 ppm (buffer filling)
slew_rate = 10 ppm/sec
adjustment_interval = 100 ms
max_delta = 10 * 0.1 = 1 ppm

If current_adjustment = +10 ppm:
    drift_delta = 15 - 10 = +5 ppm
    new_adjustment = 10 + 1 = +11 ppm  (slew limited)
    resampling_ratio = 1.000011  (speed up playback slightly)
```

### 7.4. PLL State Machine

```
       ┌─────────┐
       │ SEEKING │ ← Initial state, measuring drift
       └────┬────┘
            │ |drift| < 5 ppm for 5 seconds
            ▼
       ┌─────────┐
       │ LOCKED  │ ← Stable, normal operation
       └────┬────┘
            │ |drift| > 20 ppm for 2 seconds
            ▼
       ┌─────────┐
       │UNLOCKED │ ← Drift too high, unstable
       └────┬────┘
            │ Auto-reconnect or buffer adjust
            ▼
       ┌─────────┐
       │ SEEKING │
       └─────────┘
```

**PLL States (reported in health message):**
- **SEEKING**: Measuring and adapting to drift
- **LOCKED**: Drift stable, tracking well
- **UNLOCKED**: Drift exceeds limits, may need intervention

### 7.5. Performance Targets

**Steady-state drift:** ±5 ppm  
**Lock time:** <10 seconds  
**Stability:** No audible artifacts from resampling

---

## 8. Enhanced Health Telemetry

### 8.1. Counter Semantics

**All counters in health messages are LIFETIME totals** (since session start), NOT deltas.

**Example:**
```json
// First health message (t=1s)
{
  "packets_received": 4800,
  "crc_ok": 64,
  "crc_fail": 0,
  "xruns": 0
}

// Second health message (t=2s)
{
  "packets_received": 9600,    // Total, not +4800
  "crc_ok": 128,               // Total, not +64
  "crc_fail": 1,               // Total, not +1
  "xruns": 0
}
```

**Server calculates deltas:**
```rust
let delta_packets = current.packets_received - previous.packets_received;
let delta_crc_fail = current.crc_fail - previous.crc_fail;

if delta_crc_fail > 0 {
    warn!("CRC failures detected: {}", delta_crc_fail);
}
```

**Rationale:** Lifetime counters prevent loss of information if health messages are dropped.

### 8.2. health (Node → Server)

Sent every ~1 second via WebSocket control channel.

**Message Type:** `health`

```json
{
  "health": {
    "timestamp_us": 1234567890123456,
    
    "connection": {
      "state": "connected",
      "uptime_seconds": 3600,
      "packets_received": 172800,
      "packets_lost": 3,
      "bytes_received": 497664000
    },
    
    "playback": {
      "state": "playing",
      "buffer_ms": 140.1,
      "buffer_health": "good",
      "buffer_fill_percent": 93
    },
    
    "latency": {
      "network_ms": 5.2,
      "jitter_buffer_ms": 140.1,
      "dac_ms": 1.34,
      "pipeline_ms": 0.62,
      "total_ms": 147.26
    },
    
    "clock_sync": {
      "drift_ppm": 1.2,
      "phase_us": 3.8,
      "pll_state": "locked",
      "adjustment_ppm": 1.5
    },
    
    "integrity": {
      "crc_ok": 2700,
      "crc_fail": 0,
      "last_crc_fail_seq": null
    },
    
    "errors": {
      "xruns": 0,
      "buffer_underruns": 0,
      "buffer_overruns": 0,
      "last_xrun_timestamp_us": null
    },
    
    "volume": {
      "level": 0.75,
      "mute": false,
      "hardware_control": true,
      "gain_db": -5.1
    },
    
    "dsp": {
      "current_profile_hash": 12345678,
      "eq_active": true,
      "convolution_active": false
    }
  }
}
```

**All counters are LIFETIME totals (except where noted).**

### 8.3. Example Latency Breakdown

**Typical latency chain for music playback:**

| Component | Latency | Description |
|-----------|---------|-------------|
| Network | 5 ms | One-way transmission time (LAN) |
| Jitter Buffer | 100 ms | Packet reordering and smoothing |
| DSP Pipeline | 0.6 ms | EQ, resampling (if node-side) |
| DAC | 1.3 ms | DAC hardware latency |
| **Total** | **106.9 ms** | End-to-end latency |

**For low-latency mode:**

| Component | Latency | Description |
|-----------|---------|-------------|
| Network | 5 ms | One-way transmission time (LAN) |
| Jitter Buffer | 33 ms | Minimal buffering |
| DSP Pipeline | 0.5 ms | Lightweight processing |
| DAC | 1.3 ms | DAC hardware latency |
| **Total** | **39.8 ms** | Low-latency mode |

---

## 9. Buffer Management Contract

### 9.1. Buffer Lifecycle

**Buffer States:**
```
EMPTY → FILLING → BUFFERED → PLAYING → DRAINING → EMPTY
```

**State Definitions:**

| State | Description | Buffer Fill | Node Action |
|-------|-------------|-------------|-------------|
| EMPTY | No data in buffer | 0% | Wait for packets |
| FILLING | Receiving but not playing | 0-66% | Buffer packets |
| BUFFERED | Ready to play | 66-100% | Ready to start |
| PLAYING | Active playback | 30-100% | Playing audio |
| DRAINING | Stream ending | 0-30% | Play remaining |
| EMPTY | Playback complete | 0% | Idle |

### 9.2. Start Threshold

**Start threshold** is the buffer fill level required before beginning playback.

**Configuration (from session_accept):**
```json
{
  "buffer": {
    "target_ms": 150,
    "min_ms": 50,
    "max_ms": 500,
    "start_threshold_ms": 100
  }
}
```

**Start Threshold Rules:**
1. Node MUST buffer at least `start_threshold_ms` before starting playback
2. Typically `start_threshold_ms = target_ms * 0.66`
3. Prevents immediate underrun after play starts

**Example:**
```
target_ms = 150
start_threshold_ms = 100

Buffer fills:
  0ms → 50ms → 100ms ← START PLAYBACK
  100ms → 150ms (continue filling while playing)
  Steady state: oscillates around 150ms
```

### 9.3. Buffer Health Indicators

**Node reports `buffer_health` in health message:**

| Health | Buffer Fill | Description |
|--------|-------------|-------------|
| `critical` | <30% | Danger of underrun |
| `low` | 30-60% | Below target |
| `good` | 60-90% | Normal range |
| `high` | 90-100% | Risk of overrun |

**Server Actions:**
- `critical`: Consider increasing buffer size
- `low`: Monitor for underruns
- `good`: Normal operation
- `high`: Consider reducing latency

### 9.4. Adaptive Buffering

**Server monitors xrun rate and adjusts buffer size:**

```rust
fn adjust_buffer_size(stats: &HealthStats, config: &mut BufferConfig) {
    let xrun_rate = stats.xruns_per_minute();
    
    if xrun_rate > 5 {
        // Too many underruns, increase buffer
        config.target_ms += 50;
        config.start_threshold_ms = (config.target_ms as f32 * 0.66) as u32;
        info!("Increased buffer to {}ms due to underruns", config.target_ms);
    } else if xrun_rate == 0 && config.target_ms > 100 {
        // No underruns for a while, try reducing latency
        config.target_ms -= 25;
        config.start_threshold_ms = (config.target_ms as f32 * 0.66) as u32;
        info!("Decreased buffer to {}ms for lower latency", config.target_ms);
    }
    
    // Clamp to configured range
    config.target_ms = config.target_ms.clamp(config.min_ms, config.max_ms);
}
```

---

## 10. Error Codes and State Machine

### 10.1. Standard Error Codes

All error messages include a standardized error code.

| Code | Category | Description | Severity |
|------|----------|-------------|----------|
| **E1xx** | **Connection** | | |
| E101 | Connection | Network unreachable | Fatal |
| E102 | Connection | Connection timeout | Warning |
| E103 | Connection | Connection refused | Fatal |
| E104 | Connection | WebSocket error | Fatal |
| E105 | Connection | RTP port bind failed | Fatal |
| **E2xx** | **Protocol** | | |
| E201 | Protocol | Version mismatch | Fatal |
| E202 | Protocol | Invalid session_init | Fatal |
| E203 | Protocol | Invalid message format | Warning |
| E204 | Protocol | Unsupported feature | Warning |
| E205 | Protocol | SSRC conflict | Warning |
| **E3xx** | **Audio** | | |
| E301 | Audio | Unsupported sample rate | Fatal |
| E302 | Audio | Unsupported format | Fatal |
| E303 | Audio | DAC open failed | Fatal |
| E304 | Audio | Buffer underrun | Warning |
| E305 | Audio | Buffer overrun | Warning |
| E306 | Audio | CRC verification failed | Warning |
| **E4xx** | **Clock** | | |
| E401 | Clock | Drift too high | Warning |
| E402 | Clock | PLL unlock | Warning |
| E403 | Clock | Timestamp discontinuity | Warning |
| **E5xx** | **DSP** | | |
| E501 | DSP | EQ application failed | Warning |
| E502 | DSP | Convolution failed | Warning |
| E503 | DSP | Insufficient CPU | Warning |
| E504 | DSP | Profile hash mismatch | Info |
| **E6xx** | **Volume** | | |
| E601 | Volume | Hardware volume unavailable | Info |
| E602 | Volume | Volume out of range | Warning |

### 10.2. Error Message Format

**Message Type:** `error`

```json
{
  "error": {
    "code": "E304",
    "category": "audio",
    "severity": "warning",
    "message": "Buffer underrun detected",
    "details": {
      "buffer_ms": 5.2,
      "target_ms": 150.0,
      "timestamp_us": 1234567890123456
    },
    "recovery_action": "increase_buffer"
  }
}
```

**Severity Levels:**
- **Fatal**: Session cannot continue, must reconnect
- **Warning**: Degraded performance, monitoring required
- **Info**: FYI, no action needed

### 10.3. Node State Machine

```
                    ┌──────────────┐
                    │ DISCONNECTED │
                    └───────┬──────┘
                            │ mDNS advertise
                            ▼
                    ┌──────────────┐
             ┌──────│     IDLE     │◄─────┐
             │      └───────┬──────┘      │
             │              │ Server       │
             │              │ connects     │
             │              ▼              │
             │      ┌──────────────┐      │
             │      │ NEGOTIATING  │      │
             │      └───────┬──────┘      │
             │              │ session_accept│
             │              ▼              │
             │      ┌──────────────┐      │
             │      │  BUFFERING   │      │
             │      └───────┬──────┘      │
             │              │ Buffer       │
             │              │ threshold    │
             │              ▼              │
      STOP   │      ┌──────────────┐      │ Error
      ◄──────┼──────│   PLAYING    │──────┤
             │      └───────┬──────┘      │
             │              │ PAUSE        │
             │              ▼              │
             │      ┌──────────────┐      │
             └──────│    PAUSED    │      │
                    └──────────────┘      │
                                           │
                    ┌──────────────┐      │
                    │    ERROR     │◄─────┘
                    └───────┬──────┘
                            │ Retry
                            ▼
                    ┌──────────────┐
                    │     IDLE     │
                    └──────────────┘
```

**State Descriptions:**

| State | Playback | Buffer | Description |
|-------|----------|--------|-------------|
| DISCONNECTED | No | Empty | Node not connected to any server |
| IDLE | No | Empty | Connected, waiting for stream |
| NEGOTIATING | No | Empty | Exchanging capabilities |
| BUFFERING | No | Filling | Pre-buffering before playback |
| PLAYING | Yes | Active | Normal playback |
| PAUSED | No | Held | Playback paused, buffer held |
| ERROR | No | Varies | Error state, attempting recovery |

**State Transitions:**

| From | Event | To | Action |
|------|-------|----|----|
| DISCONNECTED | mDNS advertise | IDLE | Wait for connection |
| IDLE | Server connects | NEGOTIATING | Exchange session_init |
| NEGOTIATING | session_accept | BUFFERING | Start buffering |
| BUFFERING | Buffer >= threshold | PLAYING | Start playback |
| PLAYING | PAUSE command | PAUSED | Hold buffer |
| PAUSED | RESUME command | PLAYING | Resume playback |
| PLAYING | STOP command | IDLE | Flush buffer |
| * | Fatal error | ERROR | Attempt recovery |
| ERROR | Retry | IDLE | Reconnect |

---

## 11. RTCP Sender Reports 

### 11.1. RTCP SR Packet

**When `rtcp_sr` feature is negotiated:**

Server SHOULD send RTCP Sender Reports (SR) for QoS monitoring.

**RTCP SR Format (RFC 3550):**
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|V=2|P|    RC   |   PT=SR=200   |             length            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         SSRC of sender                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|              NTP timestamp, most significant word             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|              NTP timestamp, least significant word            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         RTP timestamp                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     sender's packet count                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      sender's octet count                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Transmission:**
- RECOMMENDED: Send every 5 seconds
- Use same SSRC as RTP stream
- Sent to RTCP port (RTP port + 1)

**Node Usage:**
- Calculate round-trip time (if Node sends RR)
- Verify sender packet/byte counts match received
- Detect packet loss rate

**Benefits:**
- Better network diagnostics
- Jitter and loss statistics
- Clock synchronization (NTP timestamp)

**Implementation Priority:** Low (optional feature for advanced QoS)

---

## 12. DSP Configuration Transport 

When `dsp_transfer` feature is negotiated, server can push DSP state to Node.

### 12.1. dsp_update Control Message

**Message Type:** `dsp_update`

```json
{
  "dsp_update": {
    "profile_id": 42,
    "profile_name": "Rock Preset - Living Room",
    "headroom_db": -6.0,
    "dithering": "tpdf_24bit",
    
    "equalizer": {
      "name": "Rock Preset",
      "enabled": true,
      "bands": [
        {"frequency": 62, "gain": 4.0, "q": 1.0, "type": "peak"},
        {"frequency": 1000, "gain": -2.0, "q": 1.0, "type": "peak"},
        {"frequency": 16000, "gain": 3.5, "q": 1.0, "type": "peak"}
      ]
    },
    
    "convolution": {
      "enabled": true,
      "filter_id": "living_room_1_2025_48k",
      "delay_samples": 256,
      "gain_db": -3.0
    }
  }
}
```

### 12.2. dsp_update_ack (Node → Server)

Node acknowledges DSP update and reports which features were applied.

**Message Type:** `dsp_update_ack`

```json
{
  "dsp_update_ack": {
    "profile_id": 42,
    "status": "success",
    "profile_hash": 12345678,
    
    "applied": {
      "equalizer": true,
      "headroom": true,
      "dithering": true,
      "convolution": false
    },
    
    "errors": [
      {
        "code": "E502",
        "message": "Convolution: insufficient CPU for FFT processing"
      }
    ],
    
    "fallback": {
      "convolution": "server_side"
    }
  }
}
```

**Fields:**
- **`status`**: `success`, `partial`, `failed`
- **`profile_hash`**: CRC32 of applied profile (for verification)
- **`applied`**: Which DSP features were successfully applied
- **`errors`**: Structured error messages with codes
- **`fallback`**: Suggested fallback modes

**Fallback Strategy:**
- If Node can't apply DSP, server continues to process
- Node reports capability limitations
- Server can choose to stream pre-processed audio

### 12.3. Convolution Filter Transfer

If convolution is enabled and filter not cached:

**Request Message Type:** `convolution_request`

```json
{
  "convolution_request": {
    "cmd": "get_ir_url",
    "filter_id": "living_room_1_2025_48k"
  }
}
```

**Response Message Type:** `convolution_response`

```json
{
  "convolution_response": {
    "ir_url": "http://10.0.0.1:8080/ir/living_room_1_2025_48k.flac",
    "format": "flac",
    "sample_rate": 48000,
    "length_samples": 131072,
    "checksum": "sha256:abcd1234..."
  }
}
```

**Node downloads IR file via HTTP, verifies checksum, applies convolution.**

---

## 13. Error Recovery Protocols

### 13.1. Network Interruption

**Detection:**
- Node doesn't receive RTP packets for 1 second
- health message reports: `"connection_state": "interrupted"`
- Error code: E102

**Recovery:**
- Node maintains buffer for up to 5 seconds
- Auto-reconnect: 1-second intervals, max 10 attempts
- On reconnect, server resends last 100ms to prevent gaps

**Example health During Interruption:**

**Message Type:** `health` with embedded error

```json
{
  "health": {
    "connection": {
      "state": "interrupted",
      "reconnect_attempts": 3,
      "buffer_remaining_ms": 2300
    }
  },
  "error": {
    "code": "E102",
    "message": "Connection timeout, attempting reconnect"
  }
}
```

### 13.2. Buffer Underrun

**Detection:**
- Playback buffer depleted
- Node reports: `"xruns": N` in health message
- Error code: E304

**Recovery:**
```
If xruns > 5 in 10 seconds:
  1. Server increases buffer_ms by 50ms
  2. Server logs warning
  3. Server notifies UI if persistent
  4. Node continues playback (may cause audible glitch)
```

**Adaptive Buffering:**
- Server tracks xrun frequency
- Automatically adjusts buffer size
- Target: zero xruns during steady state

### 13.3. Sample Rate Change

**Protocol:**

**Message Types:** `stream_pause`, `stream_paused`, `session_init`, `session_accept`, `stream_resume`

```
1. Server sends stream_pause
2. Node flushes buffer, reports "buffering" state via stream_paused
3. Server sends new session_init (with new sample rate)
4. Node reconfigures DAC, sends session_accept
5. Server sends stream_resume
6. Playback continues at new sample rate
```

**Example:**
```json
// Server
{ "stream_pause": {} }

// Node
{ "stream_paused": { "buffer_flushed": true } }

// Server (reconfigure)
{ "session_init": { "sample_rate": 96000, ... } }

// Node
{ "session_accept": { "status": "ready", ... } }

// Server
{ "stream_resume": {} }
```

### 13.4. CRC Failure

**Detection:**
- Node verifies CRC32 every N packets (configurable, default 64)
- Reports failures in health: `"crc_fail": N`
- Error code: E306

**Response:**
```
If crc_fail > 0:
  1. Server logs warning with packet details
  2. If crc_fail_rate > 1%:
     - Server reduces bitrate (if possible)
     - Server increases redundancy
     - Server notifies user of network issues
```

### 13.5. DSP Profile Mismatch

**Detection:**
- Node's `current_dsp_hash` doesn't match server expectation
- Error code: E504
- Indicates DSP state drift

**Recovery:**
```
1. Server detects mismatch in health message
2. Server resends dsp_update
3. Node applies and acknowledges via dsp_update_ack
4. Hash should match on next health message
```

---

## 14. Recommended Configurations

### 14.1. Low-Latency (Gaming, Live Audio)

**Use Case:** System audio capture for games, video calls, live streaming

**Configuration:**
```json
{
  "sample_rate": 48000,
  "format": "S24LE",
  "buffer_ms": 50,
  "start_threshold_ms": 33,
  "micro_pll": true,
  "crc_verify": false
}
```

**Expected Performance:**
- Total latency: 60-120ms
- CPU usage: 5-10%
- Network: ~2.3 Mbps

### 14.2. High-Quality Music (Audiophile)

**Use Case:** Spotify/Tidal lossless streaming with DSP

**Configuration:**
```json
{
  "sample_rate": 96000,
  "format": "S24LE",
  "buffer_ms": 200,
  "start_threshold_ms": 133,
  "micro_pll": true,
  "crc_verify": true,
  "dsp_transfer": true
}
```

**Expected Performance:**
- Total latency: 220-280ms
- CPU usage: 10-15%
- Network: ~4.6 Mbps

### 14.3. Multi-Room Sync (Future)

**Use Case:** Synchronized playback across multiple rooms

**Configuration:**
```json
{
  "sample_rate": 48000,
  "format": "S24LE",
  "buffer_ms": 300,
  "start_threshold_ms": 200,
  "micro_pll": true,
  "ptp_sync": true
}
```

**Expected Performance:**
- Total latency: 320-380ms
- Sync accuracy: ±10ms
- Network: ~2.3 Mbps per node

---

## 15. WebSocket Control Channel

All control messages use WebSocket over TLS (wss://).

### 15.1. Control Message Types

**Message Type Naming:** All control messages use **snake_case** for JSON keys.

**From Server to Node:**
- `session_accept` - Confirm session parameters
- `volume_set` - Set volume/mute
- `dsp_update` - Push DSP configuration
- `stream_pause` - Pause streaming
- `stream_resume` - Resume streaming
- `stream_stop` - Stop streaming
- `get_status` - Request status update

**From Node to Server:**
- `session_init` - Initiate session with capabilities
- `health` - Periodic health telemetry
- `volume_result` - Volume query result
- `dsp_update_ack` - DSP configuration acknowledgment
- `stream_paused` - Confirm pause
- `stream_stopped` - Confirm stop
- `error` - Error notification

### 15.2. Connection Lifecycle

```
1. Node starts, advertises via mDNS
2. Server discovers Node, connects to WebSocket
3. Node sends session_init (capabilities)
4. Server sends session_accept (configuration)
5. Server starts RTP stream
6. Node sends health messages (every 1s)
7. [Interactive control: volume, DSP updates, etc.]
8. Server sends stream_stop
9. Node sends stream_stopped
10. WebSocket closes gracefully
```

---

## 16. Security Considerations (Future)

**v0.4 Security:**
- WebSocket over TLS (wss://) recommended
- mDNS discovery on trusted network only
- No authentication required (LAN use case)

**Future Security (v0.5+):**
- SRTP with AES-GCM for RTP stream encryption
- Token-based authentication for WebSocket
- Certificate pinning for Node-Server trust
- Rate limiting for control messages

---

## 17. Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Latency (Low)** | <100ms | Network + jitter buffer + DAC |
| **Latency (Normal)** | 150-250ms | Normal music listening |
| **Jitter RMS** | <500µs | Clock stability |
| **Drift Error** | ±5 ppm | With Micro-PLL |
| **Packet Loss** | <0.1% | RTP packets |
| **CRC Error Rate** | 0% | Bit-perfect verification |
| **CPU (Server)** | <10% | On modern CPU |
| **CPU (Node)** | <20% | Raspberry Pi 4 |
| **Memory (Node)** | <50MB | RSS |

---

## 18. Implementation Checklist

### Core Protocol (Week 1-3)
- [ ] RTP audio transport (L16, L24) with correct endianness
- [ ] WebSocket control channel with snake_case messages
- [ ] mDNS discovery with compact TXT records (uuid first)
- [ ] Session negotiation with UUID and RTP extension negotiation
- [ ] Health telemetry with lifetime counters

### Clock Sync (Week 3-4)
- [ ] Micro-PLL drift correction with specified parameters
- [ ] Phase lock loop implementation
- [ ] Adaptive resampling with slew rate limiting
- [ ] EMA drift smoothing

### Volume Control (Week 4-5)
- [ ] Volume curves (linear, logarithmic, exponential)
- [ ] Volume-to-dB conversion (40 * log10 formula)
- [ ] Software volume control
- [ ] Hardware volume (ALSA mixer) with normalization
- [ ] Mute support
- [ ] Volume ramp with shapes (linear, s-curve, exponential)

### Quality (Week 5-6)
- [ ] CRC32 verification via RTP extension (correct length encoding)
- [ ] Bit-perfect mode
- [ ] Gapless playback via RTP extension
- [ ] S24LE network byte order handling

### Error Recovery (Week 6-7)
- [ ] Network interruption handling with error codes
- [ ] Buffer underrun recovery
- [ ] Adaptive buffering
- [ ] Sample rate change protocol
- [ ] State machine implementation

### Optional Features (Week 8-10)
- [ ] DSP offloading (basic EQ)
- [ ] Convolution IR transfer
- [ ] DSP acknowledgment protocol
- [ ] RTCP SR support

### Testing (Week 10-12)
- [ ] Unit tests (protocol, parsing, volume curves)
- [ ] Integration tests (Server + Node)
- [ ] Hardware tests (various DACs)
- [ ] Network tests (jitter, loss)
- [ ] Endianness tests (S24LE packing/unpacking)
- [ ] Long-term stability tests

---

## 19. Conformance Checklist

### MUST Implement (Required)
- ✅ RTP audio transport (L16 or L24)
- ✅ WebSocket control channel (wss:// recommended)
- ✅ mDNS service discovery
- ✅ session_init / session_accept negotiation
- ✅ health telemetry (every ~1s)
- ✅ Persistent node UUID
- ✅ Buffer start threshold
- ✅ Network byte order for audio samples
- ✅ Lifetime counters in health messages
- ✅ Standard error codes
- ✅ State machine compliance

### SHOULD Implement (Strongly Recommended)
- ✅ Micro-PLL clock synchronization
- ✅ Volume control (hardware or software)
- ✅ Logarithmic volume curve
- ✅ S-curve volume ramp
- ✅ CRC32 verification
- ✅ Gapless playback
- ✅ Adaptive buffering
- ✅ Error recovery protocols
- ✅ TLS for WebSocket (wss://)

### MAY Implement (Optional)
- ⚪ DSP transfer and offloading
- ⚪ Convolution filter support
- ⚪ RTCP Sender Reports
- ⚪ Hardware volume control
- ⚪ Exponential volume curve
- ⚪ PTP clock sync (v0.5+)
- ⚪ SRTP encryption (v0.5+)

---

## 20. Success Criteria

### Functional Requirements
- ✅ Bit-perfect audio delivery (verified via CRC)
- ✅ Gapless track transitions
- ✅ Remote volume control with curves
- ✅ Automatic Node discovery with UUID
- ✅ Stable 24+ hour operation
- ✅ <250ms total latency (music use case)
- ✅ <100ms total latency (low-latency use case)

### Quality Requirements
- ✅ Zero CRC errors in normal conditions
- ✅ <0.1% packet loss handling
- ✅ ±5 ppm clock drift correction
- ✅ Graceful error recovery with error codes
- ✅ Professional audio quality (subjective)

### Usability Requirements
- ✅ 15-minute Pi setup experience
- ✅ Automatic server discovery
- ✅ Zero-config for basic use case
- ✅ Clear error messages with codes
- ✅ Responsive volume control (<100ms)

---

## Conclusion

**ANP v0.4** is a complete, implementable specification focusing on essential features for streaming service integration:

✅ **Core audio transport** (RTP with extensions, correct endianness)  
✅ **Volume control** (curves with 40*log10 formula, hardware/software)  
✅ **Node capabilities** (UUID-first, intelligent config)  
✅ **Error recovery** (codes, state machine)  
✅ **Enhanced telemetry** (lifetime counters, latency breakdown)  
✅ **Gapless playback** (RTP extensions with correct length encoding)  
✅ **Clock sync** (Micro-PLL with detailed parameters)  
✅ **Buffer contract** (start threshold, health indicators)  
✅ **Consistent JSON** (snake_case for all control messages)

**Deferred to v0.5+:**
- DSD support (niche use case)
- SRTP encryption (LAN use is safe)
- PTP clock sync (Micro-PLL sufficient)

This specification is **ready for implementation** with clear protocols, error handling, performance targets, and conformance requirements for the core AAEQ+ANP use case.

**Last Updated:** 2025-10-26  
**Version:** 0.4.0  
**Status:** Ready for Implementation  
