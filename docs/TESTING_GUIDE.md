# AAEQ Stream Server - Testing Guide

Complete guide to testing the new DSP audio streaming features.

## Quick Start

### 1. Discover Available Devices

```bash
cd /home/jascha/Documents/repos/AAEQ
cargo run -p stream-server --example discover_devices
```

This will show you:
- Local audio output devices (speakers, headphones)
- AirPlay devices on your network

### 2. Test Local DAC (Easiest)

```bash
cargo run -p stream-server --example test_local_dac
```

**What to expect:**
- Lists available audio devices
- Plays a 1kHz sine wave for 3 seconds through your default device
- Shows progress and latency information

**Troubleshooting:**
- **No sound**: Check system volume, ensure audio device is not muted
- **Device not found**: Try running `aplay -L` (Linux) to verify audio devices exist
- **Permission denied**: On Linux, add your user to the `audio` group

---

## Test Scenarios

### Scenario 1: Local DAC Output (Direct Audio)

**Purpose**: Test direct audio output to your sound card/USB DAC

**Steps**:
1. Ensure your speakers/headphones are connected and working
2. Run the test:
   ```bash
   cargo run -p stream-server --example test_local_dac
   ```
3. You should hear a pure 1kHz tone

**What's being tested**:
- CPAL audio I/O
- F32 format conversion
- Ring buffer management
- Cross-platform audio output

**Success criteria**:
- ✓ Audio plays smoothly without clicks/pops
- ✓ No buffer underruns reported
- ✓ Latency < 100ms

---

### Scenario 2: DLNA/UPnP Streaming (Network Audio)

**Purpose**: Test HTTP audio streaming to network players with both pull and push modes

#### Scenario 2A: DLNA Pull Mode (Manual Configuration)

**Steps**:
1. Start the DLNA server:
   ```bash
   cargo run -p stream-server --example test_dlna
   ```

2. The server will display a URL like: `http://localhost:8090/stream.wav`

3. Test with one of these methods:

   **Option A: VLC Media Player**
   ```bash
   vlc http://localhost:8090/stream.wav
   ```

   **Option B: mpv**
   ```bash
   mpv http://localhost:8090/stream.wav
   ```

   **Option C: curl (silent test)**
   ```bash
   curl http://localhost:8090/stream.wav > /dev/null
   ```

   **Option D: Web Browser**
   - Open `http://localhost:8090/stream.wav` in your browser

   **Option E: Network Audio Device**
   - Configure your WiiM/Bluesound/HEOS device to pull from the URL

**What's being tested**:
- HTTP streaming server (Axum)
- WAV format with chunked transfer encoding
- S16LE PCM audio
- Network buffering
- Multiple concurrent clients

**Success criteria**:
- ✓ Stream starts within 1-2 seconds
- ✓ Audio plays continuously without stuttering
- ✓ Multiple clients can connect simultaneously
- ✓ `/status` endpoint shows correct information

**Troubleshooting**:
- **Connection refused**: Check if port 8090 is already in use (`lsof -i :8090`)
- **Firewall**: Allow incoming connections on port 8090
- **No audio**: Check client volume settings
- **Stuttering**: Increase `buffer_ms` in the code (default: 200ms)

#### Scenario 2B: DLNA Device Discovery

**Steps**:
1. Discover DLNA MediaRenderer devices on your network:
   ```bash
   cargo run -p stream-server --example discover_dlna_devices
   ```

2. Wait 15 seconds for discovery to complete

3. Review discovered devices:
   - Device names
   - Manufacturers and models
   - IP addresses
   - Available services (AVTransport, RenderingControl)

**What's being tested**:
- SSDP multicast discovery
- UPnP device description parsing
- Service enumeration
- Network multicast routing

**Success criteria**:
- ✓ Finds DLNA devices within 15 seconds
- ✓ Shows complete device information
- ✓ Lists available UPnP services

**Troubleshooting**:
- **No devices found**:
  - Ensure devices are powered on and connected to network
  - Check both computer and devices on same network/VLAN
  - Verify firewall allows multicast (UDP 239.255.255.250:1900)
  - Some devices may not advertise as MediaRenderer
- **Incomplete information**:
  - Some devices have minimal device descriptions
  - Try accessing device web interface for details

#### Scenario 2C: DLNA Push Mode (Automatic Control)

**Purpose**: Test AVTransport control where AAEQ automatically starts playback on device

**Prerequisites**:
- DLNA device discovered (run Scenario 2B first)
- Device must support AVTransport service
- Both devices on same network

**Steps**:
1. Stream to a specific device:
   ```bash
   cargo run -p stream-server --example test_dlna_push "Living Room"
   ```
   (Replace "Living Room" with your device name from discovery)

2. AAEQ will:
   - Start HTTP server
   - Set stream URL on device via AVTransport
   - Automatically start playback
   - Stream 15-second test tone

3. Audio should play on your DLNA device automatically

**What's being tested**:
- UPnP AVTransport SOAP control
- SetAVTransportURI action
- Play/Stop actions
- DIDL-Lite metadata generation
- End-to-end push mode workflow

**Success criteria**:
- ✓ Device accepts SetAVTransportURI command
- ✓ Playback starts automatically
- ✓ Audio streams without interruption
- ✓ Playback stops cleanly at end

**Troubleshooting**:
- **Device doesn't support AVTransport**:
  - Some DLNA devices are MediaServer only
  - Use pull mode (Scenario 2A) instead
- **SetAVTransportURI fails**:
  - Some devices require authentication (not implemented)
  - Try updating device firmware
  - Check device logs for errors
- **No audio plays**:
  - Verify device isn't muted
  - Check device selected correct input
  - Some devices may need manual play after SetURI

---

### Scenario 3: AirPlay Streaming (Wireless)

**Purpose**: Test AirPlay (RAOP) streaming to Apple devices or compatible receivers

**Prerequisites**:
- AirPlay-compatible device (HomePod, AirPort Express, Sonos, etc.)
- Device and computer on same network
- mDNS/Bonjour enabled (default on most systems)

**Steps**:
1. Discover available AirPlay devices:
   ```bash
   cargo run -p stream-server --example discover_devices
   ```

2. Stream to a device (replace "Living Room" with your device name):
   ```bash
   cargo run -p stream-server --example test_airplay "Living Room"
   ```

3. You should hear a 440Hz tone (A4 note) for 10 seconds

**What's being tested**:
- mDNS device discovery
- RTSP protocol implementation
- RTP audio streaming
- ALAC framing (simplified)
- UDP packet transmission
- RTCP feedback

**Success criteria**:
- ✓ Device discovered within 5 seconds
- ✓ RTSP connection established
- ✓ Audio plays on the AirPlay device
- ✓ No dropped packets reported

**Troubleshooting**:
- **No devices found**:
  - Ensure devices are powered on
  - Check both devices are on same network (not guest network)
  - Verify mDNS is working: `dns-sd -B _raop._tcp` (macOS) or `avahi-browse -r _raop._tcp` (Linux)
  - Check firewall allows UDP port 5353 (mDNS)

- **Connection fails**:
  - Some devices require authentication (not fully implemented)
  - Try older AirPlay 1 devices first (easier to connect)
  - Check device logs for connection attempts

- **No audio / distorted audio**:
  - Current implementation uses simplified ALAC encoding
  - Some devices may not accept non-standard ALAC frames
  - This is expected with the current stub encoder

---

## Advanced Testing

### Test Multiple Sinks Simultaneously

Create a custom test program:

```rust
use stream_server::*;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let mut manager = OutputManager::new();

    // Register all sinks
    manager.register_sink(Box::new(LocalDacSink::new(None)));
    manager.register_sink(Box::new(DlnaSink::new(
        "Test".to_string(),
        "0.0.0.0:8090".parse()?,
    )));

    // Select and test each
    let config = OutputConfig::default();

    manager.select_sink_by_name("local_dac", config.clone()).await?;
    // ... write audio ...

    manager.select_sink_by_name("dlna", config).await?;
    // ... write audio ...

    Ok(())
}
```

### Test Format Conversions

Test different audio formats:

```rust
// Test F32 (native float)
let config = OutputConfig {
    format: SampleFormat::F32,
    ..Default::default()
};

// Test S24LE (24-bit PCM with dithering)
let config = OutputConfig {
    format: SampleFormat::S24LE,
    ..Default::default()
};

// Test S16LE (16-bit PCM)
let config = OutputConfig {
    format: SampleFormat::S16LE,
    ..Default::default()
};
```

### Test Different Sample Rates

```rust
// CD quality
let config = OutputConfig {
    sample_rate: 44100,
    ..Default::default()
};

// High resolution
let config = OutputConfig {
    sample_rate: 96000,
    ..Default::default()
};
```

### Stress Test - Long Duration

Modify the examples to run for longer periods:

```rust
let duration_secs = 300.0; // 5 minutes
```

Monitor for:
- Memory leaks
- Buffer overflows
- Connection stability
- CPU usage (`top` or `htop`)

---

## Performance Testing

### Measure Latency

The examples show latency at the end. Expected values:

- **Local DAC**: 20-50ms
- **DLNA**: 150-300ms
- **AirPlay**: 2000ms+ (intentional for sync)

### Check CPU Usage

While audio is playing:

```bash
# In another terminal
top -p $(pgrep -f test_local_dac)
```

Expected CPU usage:
- Local DAC: 1-3%
- DLNA: 2-5%
- AirPlay: 5-10%

### Monitor Network Traffic (DLNA/AirPlay)

```bash
# Monitor network traffic
sudo iftop -i wlan0

# Or use tcpdump
sudo tcpdump -i wlan0 port 8090  # DLNA
sudo tcpdump -i wlan0 port 6000  # AirPlay RTP
```

---

## Automated Testing

### Run Unit Tests

```bash
cargo test -p stream-server
```

**Expected**: All tests pass

### Run Integration Tests

```bash
cargo test -p stream-server --test integration_test
```

**Expected**: All integration tests pass

### Run All Tests

```bash
cargo test --all
```

---

## Platform-Specific Notes

### Linux

**Audio Backend**: ALSA

**Setup**:
```bash
# Install ALSA development files
sudo apt-get install libasound2-dev

# Check audio devices
aplay -L

# Test audio output
speaker-test -t wav -c 2
```

**Permissions**:
```bash
# Add user to audio group
sudo usermod -a -G audio $USER
# Log out and back in
```

### macOS

**Audio Backend**: CoreAudio

**Setup**:
- No special setup needed
- Check System Preferences → Sound for devices

**Test**:
```bash
# List audio devices
system_profiler SPAudioDataType
```

### Windows

**Audio Backend**: WASAPI

**Setup**:
- Check Sound Settings for output devices
- Ensure correct default device is selected

---

## Common Issues

### Issue: "No audio device available"

**Solutions**:
1. Check audio device is connected
2. Verify device appears in system sound settings
3. Try selecting a specific device by name
4. Check audio service is running (Linux: `pulseaudio`, Windows: "Windows Audio")

### Issue: "Buffer underrun" messages

**Solutions**:
1. Increase `buffer_ms` in OutputConfig
2. Close other audio applications
3. Reduce system load
4. Check for CPU throttling

### Issue: "Connection refused" (DLNA)

**Solutions**:
1. Check port is not already in use: `lsof -i :8090`
2. Try a different port
3. Check firewall settings
4. Ensure binding to correct network interface

### Issue: AirPlay device not discovered

**Solutions**:
1. Verify device is AirPlay-compatible
2. Check both devices on same network
3. Test mDNS: `avahi-browse -a` (Linux) or `dns-sd -B _raop._tcp` (macOS)
4. Restart device and try again
5. Check firewall allows UDP 5353

### Issue: Audio quality problems

**Possible causes**:
- Format mismatch (try different SampleFormat)
- Sample rate mismatch
- Network issues (DLNA/AirPlay)
- Insufficient buffer size

**Solutions**:
1. Match sample rate to device native rate
2. Use higher bit depth (S24LE instead of S16LE)
3. Increase network buffer
4. Use wired connection for DLNA

---

## Debugging

### Enable Trace Logging

```bash
RUST_LOG=debug cargo run -p stream-server --example test_local_dac
```

Log levels:
- `error`: Only errors
- `warn`: Warnings and errors
- `info`: Normal operation info
- `debug`: Detailed debugging info
- `trace`: Very verbose

### Module-Specific Logging

```bash
RUST_LOG=stream_server::sinks=debug cargo run ...
```

### Network Debugging

**DLNA**:
```bash
# Monitor HTTP requests
RUST_LOG=axum=debug cargo run --example test_dlna
```

**AirPlay**:
```bash
# See RTSP/RTP details
RUST_LOG=stream_server::sinks::airplay=debug cargo run --example test_airplay "Device"
```

---

## Next Steps

After successful testing:

1. **Integration**: Use the sinks in your main application
2. **Configuration**: Add user-facing sink selection UI
3. **Persistence**: Save preferred devices/settings
4. **Monitoring**: Add real-time stats display
5. **Error Handling**: Add user-friendly error messages

---

## Getting Help

If you encounter issues:

1. Check this guide's troubleshooting sections
2. Review logs with `RUST_LOG=debug`
3. Test with simpler examples first (local DAC before AirPlay)
4. Verify your audio setup with system tools
5. Check GitHub issues for known problems

---

## Test Checklist

Use this checklist to verify everything works:

**Local DAC (M1):**
- [ ] Device discovery finds local DACs
- [ ] Local DAC plays test tone successfully
- [ ] No audio dropouts or clicks
- [ ] Latency < 100ms

**DLNA/UPnP (M2):**
- [ ] DLNA device discovery finds network devices
- [ ] DLNA server starts and accepts connections (pull mode)
- [ ] VLC/mpv can play DLNA stream
- [ ] Push mode successfully controls DLNA device
- [ ] AVTransport commands work (SetURI, Play, Stop)

**AirPlay (M3):**
- [ ] AirPlay device discovery works (if applicable)
- [ ] Can connect and stream to AirPlay device
- [ ] Audio plays on AirPlay receiver

**General:**
- [ ] All tests pass (`cargo test -p stream-server`)
- [ ] No memory leaks during long playback
- [ ] CPU usage is reasonable (< 10%)
- [ ] Audio quality is acceptable
- [ ] Latency is within expected ranges for each sink type
