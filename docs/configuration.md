# Configuration

## Data Directory

AAEQ stores its configuration and database in:

- **Linux**: `~/.local/share/aaeq/`
- **macOS**: `~/Library/Application Support/aaeq/`
- **Windows**: `%APPDATA%\aaeq\`

## Database Schema

- `device` - Connected devices
- `device_preset` - Cached presets from devices
- `profile` - User-created EQ profiles (e.g., "Headphones", "Speakers")
- `mapping` - Song/album/genre → preset mappings (scoped per profile)
- `genre_override` - Manual genre assignments
- `last_applied` - Tracking state for debouncing
- `app_settings` - Application settings (last connected device, last input/output devices, active profile, theme)
- `custom_eq_preset` - User-created custom EQ presets
- `custom_eq_band` - EQ band definitions for custom presets
- `dsp_profile_settings` - DSP configuration per profile (sample rate, buffer size, headroom, dithering, resampling)
