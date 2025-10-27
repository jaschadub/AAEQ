# Configuration

## Data Directory

AAEQ stores its configuration and database in:

- **Linux**: `~/.local/share/aaeq/`
- **macOS**: `~/Library/Application Support/aaeq/`
- **Windows**: `%APPDATA%\aaeq\`

## Database Schema

- `device` - Connected WiiM/LinkPlay devices
- `device_preset` - Cached EQ presets from connected devices
- `profile` - User-created profiles for different listening scenarios (e.g., "Headphones", "Speakers")
- `mapping` - Song/album/genre → preset mappings (scoped per profile)
- `genre_override` - Manual genre assignments for tracks without metadata
- `last_applied` - Tracking state for debouncing EQ changes
- `app_settings` - Application settings (connected device, input/output devices, active profile, theme, hotkey, debug logging)
- `custom_eq_preset` - User-created custom EQ presets (for DSP mode)
- `custom_eq_band` - EQ band definitions (frequency, gain, Q) for custom presets
- `dsp_profile_settings` - DSP configuration per profile (sample rate, buffer size, headroom, dithering, resampling, sink settings)
- `managed_devices` - Discovered and configured DLNA/AirPlay devices for streaming
