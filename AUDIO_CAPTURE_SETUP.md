# Audio Capture Setup Guide

This guide explains how to capture system audio (music from Spotify, web browsers, etc.) and stream it to your DLNA/AirPlay devices with AAEQ's DSP engine.

## Overview

To stream audio from your computer to network devices (WiiM, AirPlay speakers, etc.), you need to:
1. Create a **loopback/monitor device** that captures system audio
2. Configure your music apps to output to this device
3. Select the monitor device as input in AAEQ

---

## Linux (PulseAudio/PipeWire)

### Automatic Setup (Recommended)

Run the provided setup script:

```bash
./setup-audio-loopback.sh
```

This creates:
- A virtual sink called **"AAEQ_Capture"** for apps to output to
- A loopback that sends audio to both your speakers AND AAEQ

### Manual Setup

```bash
# Create virtual sink
pactl load-module module-null-sink \
    sink_name=aaeq_capture \
    sink_properties=device.description="AAEQ_Capture" \
    rate=48000 \
    channels=2

# Create loopback to your speakers
pactl load-module module-loopback \
    source=aaeq_capture.monitor \
    latency_msec=1
```

### Using the Capture Device

1. **Redirect your music player:**
   - Install PulseAudio Volume Control: `sudo apt-get install pavucontrol`
   - Run `pavucontrol`
   - Go to **Playback** tab
   - While music is playing, change the output to **"AAEQ_Capture"**

2. **In AAEQ:**
   - Click **"🔍 Discover"** next to Input Device
   - Select **"🔊 aaeq_monitor (AAEQ Capture - System Audio)"**
   - Uncheck **"Use Test Tone"**
   - Click **"Start Streaming"**

### Cleanup

To remove the loopback:
```bash
pactl unload-module module-null-sink
pactl unload-module module-loopback
```

---

## Windows

### Method 1: Stereo Mix (Built-in, if available)

1. **Enable Stereo Mix:**
   - Right-click the speaker icon in system tray → **Sounds**
   - Go to **Recording** tab
   - Right-click in empty space → **Show Disabled Devices**
   - Find **"Stereo Mix"**, right-click → **Enable**
   - Right-click **Stereo Mix** → **Set as Default Device**

2. **In AAEQ:**
   - Click **"🔍 Discover"** next to Input Device
   - Select **"Stereo Mix"**
   - Uncheck **"Use Test Tone"**
   - Click **"Start Streaming"**

**Note:** Not all audio drivers include Stereo Mix. If you don't see it, use Method 2.

### Method 2: VB-Audio Virtual Cable (Free)

1. **Download and install VB-Audio Virtual Cable:**
   - Download from: https://vb-audio.com/Cable/
   - Run the installer (requires admin)
   - Restart your computer

2. **Configure Windows Audio:**
   - Right-click speaker icon → **Sounds**
   - **Playback** tab: Set **"CABLE Input"** as default device
   - **Recording** tab: **"CABLE Output"** will appear automatically

3. **Monitor audio (so you can hear it):**
   - In **Recording** tab, select **"CABLE Output"**
   - Click **Properties** → **Listen** tab
   - Check **"Listen to this device"**
   - Select your speakers/headphones in the dropdown
   - Click **OK**

4. **In AAEQ:**
   - Click **"🔍 Discover"** next to Input Device
   - Select **"CABLE Output"**
   - Uncheck **"Use Test Tone"**
   - Click **"Start Streaming"**

### Method 3: VoiceMeeter (Advanced, Free)

VoiceMeeter is a virtual audio mixer that provides more control:
- Download from: https://vb-audio.com/Voicemeeter/
- Follow VoiceMeeter's documentation for setup
- Use VoiceMeeter's virtual output as AAEQ input

---

## macOS

### Using BlackHole (Recommended, Free)

1. **Install BlackHole:**
   ```bash
   brew install blackhole-2ch
   ```

   Or download from: https://existential.audio/blackhole/

2. **Create Multi-Output Device:**
   - Open **Audio MIDI Setup** (Applications → Utilities)
   - Click **"+"** at bottom left → **Create Multi-Output Device**
   - Check both:
     - Your speakers/headphones
     - **BlackHole 2ch**
   - Right-click the Multi-Output Device → **Use This Device For Sound Output**

3. **In AAEQ:**
   - Click **"🔍 Discover"** next to Input Device
   - Select **"BlackHole 2ch"**
   - Uncheck **"Use Test Tone"**
   - Click **"Start Streaming"**

### Alternative: Loopback by Rogue Amoeba (Paid)

- More user-friendly but costs $99
- Download from: https://rogueamoeba.com/loopback/
- Creates virtual audio devices with GUI

---

## Troubleshooting

### No Audio in Stream

1. **Verify audio is flowing to capture device:**
   - **Linux:** `pavucontrol` → Recording tab → Check AAEQ is capturing
   - **Windows:** Sound settings → Recording → Check input levels are moving
   - **macOS:** Audio MIDI Setup → Check device is receiving input

2. **Check AAEQ settings:**
   - Make sure **"Use Test Tone"** is **unchecked**
   - Try clicking **"🔍 Discover"** again to refresh devices
   - Look for devices marked with **🔊** (system audio indicator)

3. **Verify music player is using correct output:**
   - Music should play to your loopback device, NOT directly to speakers
   - Most players let you select audio output in settings

### Latency/Delay

- **Linux:** Adjust `latency_msec` in loopback module (lower = less delay)
- **Windows:** In VB-Cable properties, adjust buffer size
- **macOS:** In BlackHole, try different buffer sizes
- **AAEQ:** Adjust "Buffer" slider in DSP settings (lower = less latency but more risk of dropouts)

### Audio Quality Issues

- Match sample rates: Set loopback device and AAEQ to same rate (48000 Hz recommended)
- Increase buffer size in AAEQ if you hear crackling
- For best quality, use 24-bit PCM format (S24LE) instead of 16-bit

---

## Platform-Specific Notes

### Linux
- PipeWire users: The above commands work with PipeWire too (it has PulseAudio compatibility)
- JACK users: Use JACK's connection graph to route audio
- The `.asoundrc` file is already configured for ALSA access

### Windows
- Some audio drivers don't support Stereo Mix anymore (especially laptops)
- VB-Cable is the most reliable free solution
- Windows 11 may require additional permissions for virtual audio devices

### macOS
- BlackHole is open-source and works great
- Multi-Output Device lets you hear audio while capturing
- Some apps (like Safari) may need permission to access audio devices

---

## Performance Tips

1. **Use wired Ethernet** for DLNA/network streaming (more stable than WiFi)
2. **Close unnecessary apps** to reduce audio processing load
3. **Match sample rates** across the entire audio chain
4. **Start with higher buffer sizes** (200-300ms), reduce if latency is acceptable
5. **Use S24LE format** for best quality with DLNA devices

---

## Need Help?

If you're still having issues:
1. Check the logs for error messages
2. Verify your loopback device works outside AAEQ (try recording with Audacity)
3. Make sure your network devices support the audio format you're using
4. Try the test tone first to verify DLNA streaming works
