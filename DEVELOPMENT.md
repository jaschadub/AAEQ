# AAEQ Development Guide

## Project Structure

```
adapt-eq/
├─ crates/
│  ├─ core/              # Core mapping engine, state, models
│  ├─ device-wiim/       # WiiM plugin (LinkPlay HTTP API)
│  ├─ ui-egui/           # UI front-end with egui
│  └─ persistence/       # SQLite database layer
├─ apps/
│  └─ desktop/           # Main desktop application binary
└─ Cargo.toml            # Workspace configuration
```

## Building

```bash
# Build the entire workspace
cargo build --workspace

# Build in release mode
cargo build --workspace --release

# Run the application
cargo run --bin aaeq

# Run tests
cargo test --workspace
```

## Running

```bash
# Development mode
cargo run --bin aaeq

# Or run the binary directly
./target/debug/aaeq
```

## Features Implemented (v0.1)

- ✅ Core data models (TrackMeta, Mapping, EqPreset)
- ✅ Hierarchical mapping resolver (Song → Album → Genre → Default)
- ✅ WiiM device controller with LinkPlay HTTP API
- ✅ SQLite persistence layer
- ✅ egui-based UI with:
  - Vertical sliders for 10-band EQ control
  - Now Playing view
  - Preset management
  - Mapping quick-save buttons
- ✅ Polling loop with debounce logic
- ✅ Cross-platform support (Linux, macOS, Windows)

## Key Components

### Core (`crates/core`)

- **TrackMeta**: Represents track metadata (artist, title, album, genre)
- **Scope**: Enum for mapping hierarchy (Song, Album, Genre, Default)
- **RulesIndex**: Fast lookup index for mapping resolution
- **resolve_preset()**: Main logic for determining which preset to apply

### Device WiiM (`crates/device-wiim`)

- **WiimController**: Implements DeviceController trait
- HTTP API calls to LinkPlay devices
- JSON/text parsing for player status and EQ commands

### Persistence (`crates/persistence`)

- SQLite database with tables:
  - `device`: Connected devices
  - `device_preset`: Cached preset lists
  - `mapping`: User-defined rules
  - `last_applied`: Debounce state
- Repository pattern for database operations

### UI egui (`crates/ui-egui`)

- **VerticalSlider**: Custom widget for EQ band control
- **NowPlayingView**: Shows current track and quick-save buttons
- **PresetsView**: List and apply presets from device
- **EqEditorView**: Create custom EQ curves with sliders
- **AaeqApp**: Main application state and update loop

## Configuration

The application stores its database at:
- **Linux**: `~/.config/aaeq/aaeq.db`
- **macOS**: `~/Library/Application Support/AAEQ/aaeq.db`
- **Windows**: `%APPDATA%\AAEQ\aaeq.db`

## WiiM API Integration

The WiiM plugin uses the LinkPlay HTTP API format:

```
http://{device_ip}/httpapi.asp?command={command}
```

Key commands:
- `getPlayerStatus` - Get current track metadata
- `EQGetList` - List available EQ presets
- `EQLoad:{preset}` - Apply a preset by name
- `EQSet:{freq}:{gain}:...` - Set custom EQ bands

**Note**: The exact API format should be verified against your WiiM API documentation.

## Testing

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p aaeq-core
cargo test -p aaeq-device-wiim

# Run with output
cargo test --workspace -- --nocapture
```

## Next Steps for Development

1. **Add WiiM API docs** to the project to refine device integration
2. **mDNS/SSDP discovery** for automatic device detection
3. **Preset import/export** (JSON/YAML)
4. **Rules management view** (edit/delete mappings)
5. **System tray integration** for background operation
6. **Cloud sync** (optional, Phase 2)

## Dependencies

Key dependencies:
- `tokio` - Async runtime
- `reqwest` - HTTP client for WiiM API
- `sqlx` - SQLite database access
- `eframe/egui` - UI framework
- `serde` - Serialization
- `anyhow` - Error handling
- `tracing` - Logging

## Troubleshooting

### Database Issues

If you encounter database errors, delete the database file to reset:
```bash
rm ~/.config/aaeq/aaeq.db  # Linux
rm ~/Library/Application\ Support/AAEQ/aaeq.db  # macOS
```

### Device Connection

Make sure your WiiM device is on the same network and reachable. You can test connectivity:
```bash
curl http://{device_ip}/httpapi.asp?command=getPlayerStatus
```

### Logging

Set the log level with the `RUST_LOG` environment variable:
```bash
RUST_LOG=debug cargo run --bin aaeq
RUST_LOG=aaeq=trace cargo run --bin aaeq
```
