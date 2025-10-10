# AAEQ - Adaptive Audio Equalizer

[![Build Status](https://github.com/YOUR_USERNAME/AAEQ/workflows/Build/badge.svg)](https://github.com/YOUR_USERNAME/AAEQ/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

**Automatically apply per-song, album, or genre EQ presets to your network audio devices.**

AAEQ is a cross-platform desktop application that intelligently manages EQ settings on your WiiM (LinkPlay) devices based on what's currently playing. Set your favorite EQ preset once per song, album, or genre, and AAEQ will remember and apply it automatically.

![AAEQ Screenshot](docs/screenshot.png)

## ✨ Features

- 🎵 **Smart EQ Switching** - Automatically applies EQ based on song → album → genre → default priority
- 🎛️ **Manual Genre Editing** - Add genres to tracks that don't have metadata
- 🔌 **WiiM/LinkPlay Support** - Works with WiiM Mini, Pro, and other LinkPlay-based devices
- 💾 **Local-First** - All data stored locally in SQLite, no cloud required
- 🚀 **Fast & Lightweight** - Built in Rust with minimal resource usage
- 🖥️ **Cross-Platform** - Runs on Linux, macOS, and Windows

## 📥 Installation

### Download Pre-built Binaries

Download the latest release for your platform:

- **Linux**: `aaeq-linux-x64.tar.gz`
- **macOS**: `aaeq-macos-universal.dmg`
- **Windows**: `aaeq-windows-x64.zip`

[→ Latest Releases](https://github.com/YOUR_USERNAME/AAEQ/releases)

### Docker

```bash
docker pull ghcr.io/YOUR_USERNAME/aaeq:latest
docker run -d --network host ghcr.io/YOUR_USERNAME/aaeq:latest
```

### Build from Source

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/YOUR_USERNAME/AAEQ.git
cd AAEQ
cargo build --release

# Run
./target/release/aaeq
```

## 🚀 Quick Start

1. **Connect to your WiiM device**
   - Enter your device's IP address (e.g., `192.168.1.100`)
   - Click "Connect"

2. **Load presets from device**
   - Click "Refresh from Device" to see available EQ presets

3. **Create mappings**
   - Play a song on your WiiM device
   - Select an EQ preset from the list
   - Click "Apply Selected Preset"
   - Click "This Song", "This Album", or "This Genre" to save the mapping

4. **Enjoy automatic EQ switching!**
   - AAEQ will now automatically apply your saved presets when tracks change

## 📖 How It Works

AAEQ polls your WiiM device every second to check what's currently playing. When a track changes, it:

1. Checks for a **song-specific** mapping (`Artist - Title`)
2. Falls back to **album mapping** (`Artist - Album`)
3. Falls back to **genre mapping** (if genre is set)
4. Falls back to **default preset** (usually "Flat")

The resolved preset is only applied if it's different from the currently active one, preventing unnecessary device commands.

## 🎛️ Manual Genre Support

Since many streaming services don't provide genre metadata via the WiiM API, AAEQ includes a manual genre editor:

1. Click on the genre field in "Now Playing"
2. Type the genre (e.g., "Rock", "Jazz", "Classical")
3. The genre is automatically saved and will be used for preset resolution
4. Use the ↻ button to reset to device-provided genre (if available)

## 📁 Project Structure

```
AAEQ/
├── apps/
│   └── desktop/          # Main desktop application
├── crates/
│   ├── core/             # Core logic and models
│   ├── device-wiim/      # WiiM device integration
│   ├── persistence/      # SQLite database layer
│   └── ui-egui/          # egui-based UI
└── migrations/           # Database migrations
```

## 🛠️ Development

### Prerequisites

- Rust 1.75+ (stable)
- SQLite development libraries

### Running in Development

```bash
cargo run
```

### Running Tests

```bash
cargo test
```

### Code Style

```bash
cargo fmt
cargo clippy
```

## 🔧 Configuration

AAEQ stores its configuration and database in:

- **Linux**: `~/.local/share/aaeq/`
- **macOS**: `~/Library/Application Support/aaeq/`
- **Windows**: `%APPDATA%\aaeq\`

### Database Schema

- `device` - Connected devices
- `device_preset` - Cached presets from devices
- `mapping` - Song/album/genre → preset mappings
- `genre_override` - Manual genre assignments
- `last_applied` - Tracking state for debouncing

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 🐛 Known Limitations

- **WiiM API Constraints**:
  - Cannot create or save custom EQ presets (only load built-in presets)
  - Genre metadata often missing from streaming services
  - Metadata encoding issues with some sources (handled via hex decoding)

- **Device Support**:
  - Currently only supports WiiM/LinkPlay devices
  - Future: Sonos, HEOS, Bluesound support planned

## 📝 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🙏 Acknowledgments

- Built with [egui](https://github.com/emilk/egui) for the UI
- WiiM/LinkPlay API documentation
- Rust community for excellent crates and tools

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/YOUR_USERNAME/AAEQ/issues)
- **Discussions**: [GitHub Discussions](https://github.com/YOUR_USERNAME/AAEQ/discussions)

---

**Note**: Replace `YOUR_USERNAME` with your actual GitHub username throughout this README and in the workflows.
