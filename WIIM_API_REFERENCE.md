# WiiM HTTP API Reference

Based on "HTTP API for WiiM Mini.pdf" and "Linkplay Upnp API External.pdf"

## API Format

**Base URL:** `https://{device_ip}/httpapi.asp?command={command}` or `http://{device_ip}/httpapi.asp?command={command}`

The WiiM devices support HTTPS-based API with most responses in JSON format.

---

## Implemented Commands

### 1. Get Player Status

**Command:** `getPlayerStatus`

**URL:** `https://192.168.1.100/httpapi.asp?command=getPlayerStatus`

**Response Format:** JSON

```json
{
  "type": "0",
  "ch": "2",
  "mode": "10",
  "loop": "4",
  "eq": "0",
  "status": "play",
  "curpos": "184919",
  "offset_pts": "184919",
  "totlen": "0",
  "vol": "39",
  "mute": "0",
  "title": "Time",
  "artist": "Pink Floyd",
  "album": "The Dark Side of the Moon"
}
```

**Key Fields:**
- `status`: `"stop"`, `"play"`, `"loading"`, or `"pause"`
- `title`, `artist`, `album`: Track metadata (may be empty depending on source)
- `mode`: Playback mode
  - `10-19`: Wiimu playlist
  - `31`: Spotify Connect
  - `32`: TIDAL Connect
  - `40`: AUX-In
  - `41`: Bluetooth
- `eq`: Current EQ preset number
- `vol`: Volume (0-100)
- `mute`: `"0"` = unmuted, `"1"` = muted

### 2. List EQ Presets

**Command:** `EQGetList`

**URL:** `https://192.168.1.100/httpapi.asp?command=EQGetList`

**Response Format:** JSON Array

```json
["Flat", "Acoustic", "Bass Booster", "Bass Reducer", "Classical", "Dance", "Deep", "Electronic", "Hip-Hop", "Jazz", "Latin", "Loudness", "Lounge", "Piano", "Pop", "R&B", "Rock", "Small Speakers", "Spoken Word", "Treble Booster", "Treble Reducer", "Vocal Booster"]
```

### 3. Load EQ Preset

**Command:** `EQLoad:{preset_name}`

**URL:** `https://192.168.1.100/httpapi.asp?command=EQLoad:Rock`

**Response Format:** JSON

```json
{"status":"OK"}
```

or

```json
{"status":"Failed"}
```

**Note:** `preset_name` must be one of the names returned by `EQGetList`

### 4. Turn EQ On

**Command:** `EQOn`

**URL:** `https://192.168.1.100/httpapi.asp?command=EQOn`

**Response Format:** JSON

```json
{"status":"OK"}
```

### 5. Turn EQ Off

**Command:** `EQOff`

**URL:** `https://192.168.1.100/httpapi.asp?command=EQOff`

**Response Format:** JSON

```json
{"status":"OK"}
```

### 6. Check EQ Status

**Command:** `EQGetStat`

**URL:** `https://192.168.1.100/httpapi.asp?command=EQGetStat`

**Response Format:** JSON

```json
{"EQStat":"On"}
```

or

```json
{"EQStat":"Off"}
```

### 7. Get Device Information

**Command:** `getStatusEx`

**URL:** `https://192.168.1.100/httpapi.asp?command=getStatusEx`

**Response Format:** JSON (extensive, contains device name, firmware, UUID, network info, etc.)

```json
{
  "ssid": "WiiM Mini-8FA2",
  "firmware": "Linkplay.4.6.425351",
  "uuid": "FF970016A6FE22C1660AB4D8",
  "DeviceName": "WiiM Mini-8FA2",
  "MAC": "08:E9:F6:8F:8F:A2",
  ...
}
```

---

## Additional Playback Commands

### Volume Control

**Command:** `setPlayerCmd:vol:{value}`

**URL:** `https://192.168.1.100/httpapi.asp?command=setPlayerCmd:vol:50`

**Value:** 0-100

### Mute Control

**Command:** `setPlayerCmd:mute:{n}`

**URL:** `https://192.168.1.100/httpapi.asp?command=setPlayerCmd:mute:1`

**Values:**
- `n=1`: Mute
- `n=0`: Unmute

### Playback Control

- **Pause:** `setPlayerCmd:pause`
- **Resume:** `setPlayerCmd:resume`
- **Toggle Pause/Play:** `setPlayerCmd:onepause`
- **Previous:** `setPlayerCmd:prev`
- **Next:** `setPlayerCmd:next`
- **Stop:** `setPlayerCmd:stop`

---

## Important Notes

### Metadata Availability

The track metadata (title, artist, album) in `getPlayerStatus` depends on the playback source (`mode`):
- **Streaming services** (Spotify, TIDAL): Usually provides full metadata
- **AUX-In/Bluetooth**: May not provide metadata
- **Local files**: Depends on file tags

### EQ Presets vs Custom EQ

The WiiM HTTP API **only supports loading predefined EQ presets**. There is no documented command to:
- Set custom EQ band values
- Read the actual dB values of EQ bands
- Create or save custom presets

The API provides:
- ✅ List available presets (`EQGetList`)
- ✅ Load a preset by name (`EQLoad:{name}`)
- ✅ Enable/disable EQ (`EQOn`/`EQOff`)
- ❌ Set custom band gains (not supported via HTTP API)

### Genre Metadata

The WiiM `getPlayerStatus` response **does not include a `genre` field**. Genre information is not available through the HTTP API.

### HTTPS vs HTTP

WiiM devices may use self-signed certificates for HTTPS. Applications should either:
1. Accept invalid certificates when connecting to WiiM devices
2. Fall back to HTTP if HTTPS fails

---

## AAEQ Implementation Notes

### Current Implementation

The AAEQ WiiM controller (`crates/device-wiim/src/wiim.rs`) implements:

1. **Track Metadata Extraction** - `get_now_playing()`
   - Calls `getPlayerStatus`
   - Extracts artist, title, album from JSON
   - Provides fallback text when metadata is unavailable

2. **Preset Management** - `list_presets()` and `apply_preset()`
   - Calls `EQGetList` to get available presets
   - Calls `EQLoad:{name}` to apply a preset

3. **Device Health Check** - `is_online()`
   - Calls `getPlayerStatus` to verify connectivity

4. **Helper Methods**
   - `eq_on()` / `eq_off()` - Toggle EQ
   - `set_volume()` - Set volume 0-100
   - `set_mute()` - Mute/unmute
   - `get_device_info()` - Get device details

### Limitations

1. **No Custom EQ Creation**: The UI EQ editor with vertical sliders can create custom EQ curves, but these **cannot be uploaded to WiiM devices** via the HTTP API. Custom EQ functionality is disabled for WiiM.

2. **No Genre Support**: Since WiiM doesn't provide genre metadata, genre-based mapping will not work unless you manually add genre information to your mappings database.

3. **Metadata Depends on Source**: When using AUX-In or Bluetooth, track metadata may not be available.

### Recommended Workflow

1. **Connect to Device** - Enter WiiM IP address
2. **Refresh Presets** - Load available EQ presets from device
3. **Play Music** - Use Spotify Connect, TIDAL, or local files
4. **Apply Presets** - Select and apply EQ presets manually
5. **Save Mappings** - Save preset for current song/album
6. **Automatic Switching** - AAEQ will automatically apply saved presets when tracks change

---

## Testing Commands

You can test WiiM API calls using `curl`:

```bash
# Get player status
curl "http://192.168.1.100/httpapi.asp?command=getPlayerStatus"

# List EQ presets
curl "http://192.168.1.100/httpapi.asp?command=EQGetList"

# Load Rock preset
curl "http://192.168.1.100/httpapi.asp?command=EQLoad:Rock"

# Check EQ status
curl "http://192.168.1.100/httpapi.asp?command=EQGetStat"

# Set volume to 50
curl "http://192.168.1.100/httpapi.asp?command=setPlayerCmd:vol:50"
```

---

## Future Enhancements

To improve AAEQ functionality with WiiM devices, consider:

1. **UPnP/DLNA Integration** - Use the LinkPlay UPnP API (see `Linkplay Upnp API External.pdf`) for additional metadata
2. **Device Discovery** - Implement mDNS/SSDP to auto-discover WiiM devices on the network
3. **Preset Synchronization** - Periodically sync preset list in case user adds custom presets via WiiM app
4. **Mode Detection** - Adjust polling frequency based on playback mode (higher frequency for streaming, lower for AUX-In)
5. **Manual Genre Entry** - Allow users to manually specify genre for tracks/albums since it's not provided by API

---

## References

- **HTTP API for WiiM Mini.pdf** - Official HTTP API documentation
- **Linkplay Upnp API External.pdf** - UPnP/DLNA API documentation (not yet implemented)
- **WiiM Community Forums** - [https://forum.wiimhome.com/](https://forum.wiimhome.com/)
