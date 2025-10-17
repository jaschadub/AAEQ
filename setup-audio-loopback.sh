#!/bin/bash
# Setup audio loopback for capturing system audio in AAEQ
# This creates a virtual sink that applications can play to
# AAEQ will handle the actual playback, so we don't create a loopback by default

WITH_LOOPBACK=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --with-loopback)
            WITH_LOOPBACK=true
            shift
            ;;
        *)
            echo "Usage: $0 [--with-loopback]"
            echo "  --with-loopback: Also create a loopback to default speakers (not recommended when using AAEQ)"
            exit 1
            ;;
    esac
done

echo "Setting up audio capture for AAEQ..."

# Check if PulseAudio/PipeWire is running
if ! command -v pactl &> /dev/null; then
    echo "Error: pactl not found. Please install PulseAudio or PipeWire."
    exit 1
fi

# Unload existing modules if they exist (cleanup)
echo "Cleaning up any existing modules..."
pactl unload-module module-null-sink 2>/dev/null || true
pactl unload-module module-loopback 2>/dev/null || true

# Create a virtual sink (null sink) for applications to output to
echo "Creating virtual sink 'AAEQ_Capture'..."
pactl load-module module-null-sink \
    sink_name=aaeq_capture \
    sink_properties=device.description="AAEQ_Capture" \
    rate=48000 \
    channels=2

# Optionally create a loopback (only for testing without AAEQ)
if [ "$WITH_LOOPBACK" = true ]; then
    echo "Creating loopback to your speakers (for testing without AAEQ)..."
    pactl load-module module-loopback \
        source=aaeq_capture.monitor \
        latency_msec=1
    echo "⚠ Warning: Disable AAEQ streaming or you'll hear double audio!"
fi

echo ""
echo "✓ Audio capture setup complete!"
echo ""
echo "To use with AAEQ:"
echo "1. Set your music player (Spotify, etc.) to output to 'AAEQ_Capture'"
echo "   - In PulseAudio Volume Control (pavucontrol): Playback tab → Select 'AAEQ_Capture' for your app"
echo "   - OR in your app's audio settings, select 'AAEQ_Capture' as output device"
echo ""
echo "2. In AAEQ DSP Server tab:"
echo "   - Uncheck 'Use Test Tone'"
echo "   - Click 🔍 next to 'Input Device' to discover"
echo "   - Select 'aaeq_monitor' or 'aaeq_capture.monitor'"
echo "   - Select your headphones/speakers as 'Output Device'"
echo "   - Click '▶ Start Streaming'"
echo ""
echo "3. AAEQ will capture from the virtual sink and play with EQ applied!"
echo ""
if [ "$WITH_LOOPBACK" = false ]; then
    echo "Note: No loopback was created. AAEQ handles playback."
    echo "If you want to test audio routing without AAEQ, run:"
    echo "  $0 --with-loopback"
fi
echo ""
echo "To remove this setup later, run:"
echo "  pactl unload-module module-null-sink"
echo ""
