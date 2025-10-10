# AAEQ Quick Start Guide

## What is AAEQ?

**AAEQ (Adaptive Audio Equalizer)** automatically applies the right EQ preset to each song based on your preferences. It learns your per-song, per-album, or per-genre EQ choices and automatically switches them during playback.

## System Requirements

- **Linux, macOS, or Windows**
- **Rust toolchain** (for building from source)
- **WiiM device** on your local network (Mini, Pro, Plus, etc.)
- **Network access** to your WiiM device

## Installation

### Build from Source

```bash
# Clone the repository
cd /path/to/AAEQ

# Build the project
cargo build --release

# Run the application
./target/release/aaeq
```

The binary will be located at `./target/release/aaeq`

## First Run

1. **Launch AAEQ**
   ```bash
   cargo run --release
   ```

2. **Enter your WiiM device IP address**
   - Find your WiiM's IP in the WiiM Home app or router
   - Enter it in the "Device IP" field (e.g., `192.168.1.100`)
   - Click **Connect**

3. **Refresh Presets**
   - Click **Refresh from Device** in the Presets panel
   - This loads all available EQ presets from your WiiM device

4. **Start Playing Music**
   - Use Spotify Connect, TIDAL Connect, or any other source
   - AAEQ will display track information in the "Now Playing" section

## Basic Usage

### Manual EQ Selection

1. **Select a Preset** from the Presets list
2. **Click "Apply Selected Preset"**
3. Your WiiM will immediately switch to that EQ

### Save Mappings

After applying a preset you like:

1. Click one of the **Save** buttons:
   - **This Song** - Apply this preset every time this specific track plays
   - **This Album** - Apply to all tracks from this album
   - **This Genre** - Apply to all tracks of this genre (requires manual genre entry)
   - **Default** - Use as the fallback for unmapped tracks

### Automatic Switching

Once you've saved mappings, AAEQ will:
- Monitor your WiiM for track changes
- Look up the appropriate preset (Song → Album → Genre → Default)
- Automatically apply it when a new track starts

## Example Workflow

**Scenario:** You want "Bass Booster" for hip-hop albums but "Flat" for jazz

1. **Play a hip-hop album** (e.g., Kendrick Lamar - DAMN.)
2. **Select "Bass Booster"** and click Apply
3. **Click "This Album"** to save the mapping
4. **Play a jazz album** (e.g., Miles Davis - Kind of Blue)
5. **Select "Flat"** and click Apply
6. **Click "This Album"** to save
7. **Done!** AAEQ will now remember these preferences

## Understanding Mapping Hierarchy

AAEQ uses a **priority system** to determine which preset to apply:

1. **Song-specific** (highest priority)
   - `"Pink Floyd - Time"` → `"Bass Boost"`
2. **Album-specific**
   - `"Pink Floyd - The Dark Side of the Moon"` → `"Rock"`
3. **Genre-specific**
   - `"Rock"` → `"Loudness"`
4. **Default** (lowest priority)
   - Everything else → `"Flat"`

**Example:** If you have:
- Album mapping: `"The Dark Side of the Moon"` → `"Rock"`
- Song mapping: `"Time"` → `"Bass Boost"`

When "Time" plays, AAEQ will use `"Bass Boost"` (song beats album).

## Available EQ Presets (WiiM Standard)

Your WiiM device comes with these built-in presets:

- Flat
- Acoustic
- Bass Booster
- Bass Reducer
- Classical
- Dance
- Deep
- Electronic
- Hip-Hop
- Jazz
- Latin
- Loudness
- Lounge
- Piano
- Pop
- R&B
- Rock
- Small Speakers
- Spoken Word
- Treble Booster
- Treble Reducer
- Vocal Booster

## Important Notes

### Custom EQ Limitation

The WiiM HTTP API **does not support uploading custom EQ curves**. The vertical sliders in the EQ editor are for future device support or reference only. For WiiM devices, you can only use the presets listed above.

### Genre Metadata

WiiM devices **do not provide genre information** via the API. If you want to use genre-based mappings, you'll need to:
- Manually add genre info to tracks in your music library
- Or use a music player that provides richer metadata

### Metadata Availability

Track metadata (artist, title, album) availability depends on your playback source:
- ✅ **Spotify Connect, TIDAL Connect**: Full metadata
- ✅ **Local files**: Depends on file tags
- ⚠️ **Bluetooth, AUX-In**: May not provide metadata

## Troubleshooting

### Can't Connect to Device

**Problem:** "Device offline" message

**Solutions:**
1. Verify WiiM IP address in WiiM Home app
2. Ensure AAEQ and WiiM are on the same network
3. Try both HTTP and HTTPS (AAEQ tries both automatically)
4. Check firewall settings
5. Ping the device: `ping 192.168.1.100`

Test API manually:
```bash
curl "http://192.168.1.100/httpapi.asp?command=getPlayerStatus"
```

### No Track Metadata

**Problem:** Track shows as "No track" or "Track 1 of 10"

**Cause:** Playback source doesn't provide metadata (AUX/BT) or track is stopped

**Solution:** Use streaming services (Spotify, TIDAL) for best metadata

### EQ Not Switching

**Problem:** Preset doesn't change when tracks change

**Check:**
1. Is EQ enabled on device? (Check WiiM Home app)
2. Is mapping saved? (Mappings are stored in database)
3. Is poll interval too slow? (Default: 1 second)
4. Are track metadata fields matching? (Check normalization)

**Debug:**
```bash
# Run with debug logging
RUST_LOG=debug cargo run --release
```

### Wrong Preset Applied

**Problem:** AAEQ applies unexpected preset

**Cause:** Multiple mappings with different priorities

**Solution:** Check mapping hierarchy - song-specific beats album-specific.

## Advanced Usage

### Database Location

AAEQ stores its database at:
- **Linux:** `~/.config/aaeq/aaeq.db`
- **macOS:** `~/Library/Application Support/AAEQ/aaeq.db`
- **Windows:** `%APPDATA%\AAEQ\aaeq.db`

### Reset Database

To start fresh:
```bash
# Linux/macOS
rm ~/.config/aaeq/aaeq.db

# Windows
del %APPDATA%\AAEQ\aaeq.db
```

### Debug Logging

Enable detailed logging:
```bash
RUST_LOG=debug cargo run --release
RUST_LOG=aaeq=trace cargo run --release  # Very detailed
```

### Manual Mapping Management

Mappings are stored in SQLite. You can use `sqlite3` to inspect or modify them:

```bash
sqlite3 ~/.config/aaeq/aaeq.db

# List all mappings
SELECT scope, key_normalized, preset_name FROM mapping;

# Delete a specific mapping
DELETE FROM mapping WHERE id = 1;

# Clear all mappings
DELETE FROM mapping;
```

## Tips & Best Practices

1. **Start with Albums** - Album mappings are more flexible than song-specific
2. **Use Default Wisely** - Set a sensible default (like "Flat") for unmapped content
3. **Test First** - Try presets manually before saving mappings
4. **Leverage Spotify/TIDAL** - These provide the best metadata for automatic matching
5. **Be Patient** - Track changes are detected every second, not instantly

## Getting Help

- **Documentation:** See `README.md`, `DEVELOPMENT.md`, `WIIM_API_REFERENCE.md`
- **Issues:** Report bugs at [GitHub Issues](https://github.com/anthropics/claude-code/issues)
- **API Reference:** See `WIIM_API_REFERENCE.md` for WiiM HTTP API details

## What's Next?

**Future Features:**
- Device auto-discovery (mDNS/SSDP)
- Rules management UI
- Preset import/export
- System tray integration
- Cloud sync (optional)
- Support for additional devices (Sonos, HEOS, etc.)

---

**Enjoy your adaptive audio experience!** 🎵
