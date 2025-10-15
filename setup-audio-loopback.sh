#!/bin/bash
# Setup audio loopback for capturing system audio in AAEQ
# This creates a virtual sink that applications can play to, and a loopback that captures it

echo "Setting up audio loopback for AAEQ..."

# Check if PulseAudio/PipeWire is running
if ! command -v pactl &> /dev/null; then
    echo "Error: pactl not found. Please install PulseAudio or PipeWire."
    exit 1
fi

# Unload existing modules if they exist (cleanup)
echo "Cleaning up any existing loopback modules..."
pactl unload-module module-null-sink 2>/dev/null || true
pactl unload-module module-loopback 2>/dev/null || true

# Create a virtual sink (null sink) for applications to output to
echo "Creating virtual sink 'AAEQ_Capture'..."
pactl load-module module-null-sink \
    sink_name=aaeq_capture \
    sink_properties=device.description="AAEQ_Capture" \
    rate=48000 \
    channels=2

# Create a loopback from the virtual sink's monitor to the default output
# This lets you hear what's playing while AAEQ captures it
echo "Creating loopback to your speakers..."
pactl load-module module-loopback \
    source=aaeq_capture.monitor \
    latency_msec=1

echo ""
echo "✓ Audio loopback setup complete!"
echo ""
echo "To use this:"
echo "1. Set your music player (Spotify, Strawberry, etc.) to output to 'AAEQ_Capture'"
echo "   - In PulseAudio Volume Control (pavucontrol): Playback tab → Select 'AAEQ_Capture' for your app"
echo "   - OR in your app's audio settings, select 'AAEQ_Capture' as output device"
echo "2. In AAEQ, click 'Discover' next to Input Device"
echo "3. Select the device containing 'aaeq_capture.monitor' or 'AAEQ_Capture'"
echo "4. Start streaming!"
echo ""
echo "To remove this setup later, run:"
echo "  pactl unload-module module-null-sink"
echo "  pactl unload-module module-loopback"
echo ""
