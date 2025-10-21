# AAEQ Media Session

Cross-platform media session detection for AAEQ.

## Overview

This crate provides a unified interface for detecting currently playing media across different platforms:

- **Linux**: MPRIS via D-Bus (✅ works with all MPRIS-compatible players)
- **Windows**: System Media Transport Controls (✅ works with all SMTC apps)
- **macOS**: System Now Playing + AppleScript (✅ works with most apps)

## Platform Support

### Linux (MPRIS)

**Supported Apps**: Any app that implements MPRIS2
- Spotify
- Strawberry Music Player
- VLC
- Firefox/Chrome (for web players)
- Clementine
- Rhythmbox
- And many more...

**Requirements**: D-Bus (standard on all modern Linux distros)

### Windows (SMTC)

**Supported Apps**: Any app that implements System Media Transport Controls
- Spotify
- iTunes
- Apple Music
- Tidal
- YouTube Music (browser)
- Amazon Music
- Deezer
- And many more...

**Requirements**: Windows 10 version 1803 or later

### macOS

**Supported Apps**:

#### Method 1: System Now Playing (Recommended)
Works with **all** apps that publish to macOS media center:
- Spotify ✅
- Tidal ✅
- YouTube Music ✅
- Amazon Music ✅
- Deezer ✅
- SoundCloud ✅
- Apple Music ✅
- And many more...

**Requirements**: Install `nowplayingctl` for best compatibility:

```bash
brew install nowplayingctl
```

#### Method 2: AppleScript (Fallback)
Works only with apps that have AppleScript support:
- Apple Music ✅
- Spotify ✅
- iTunes ✅

**No additional requirements** - works out of the box, but limited app support.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aaeq-media-session = { path = "crates/media-session" }
```

## Usage

```rust
use aaeq_media_session::{MediaSession, create_media_session};

// Create a platform-specific session
let session = create_media_session();

// Get currently playing track
match session.get_current_track()? {
    Some(metadata) => {
        println!("Now playing: {} by {}", metadata.title, metadata.artist);
    }
    None => {
        println!("No track currently playing");
    }
}

// Check if any player is playing
if session.is_playing() {
    println!("Music is playing!");
}

// List active players
let players = session.list_active_players();
println!("Active players: {:?}", players);
```

## macOS Setup for Maximum Compatibility

To support **all streaming services** (Tidal, YouTube Music, etc.) on macOS, install `nowplayingctl`:

### Option 1: Homebrew (Recommended)

```bash
brew install nowplayingctl
```

### Option 2: Manual Installation

1. Download from: https://github.com/davidwernhart/nowplayingctl
2. Build and install following their instructions

### Without nowplayingctl

Without `nowplayingctl`, only **Music.app** and **Spotify** will be detected (via AppleScript). This still works but is more limited.

## Testing

Run the tests to verify media detection on your platform:

```bash
# Test on current platform
cargo test

# Test with logging
RUST_LOG=debug cargo test -- --nocapture
```

Start playing music in any supported app and run:

```bash
cargo test test_get_current_track -- --nocapture
```

## Architecture

The crate uses a trait-based design for cross-platform compatibility:

```
┌─────────────────────────┐
│   MediaSession trait    │  ← Common interface
└─────────────────────────┘
            ▲
            │
    ┌───────┼───────┐
    │       │       │
┌───┴───┐ ┌─┴─┐ ┌──┴───┐
│ Linux │ │Win│ │ macOS│  ← Platform implementations
└───────┘ └───┘ └──────┘
  MPRIS   SMTC   Multi-method
```

## Known Limitations

### Linux
- Requires D-Bus (standard on all modern distros)
- Apps must implement MPRIS (most music players do)
- Browser players may not always expose metadata correctly

### Windows
- Requires Windows 10 version 1803+ or Windows 11
- Apps must implement SMTC (most modern apps do)
- Some older or niche apps may not support SMTC

### macOS
- **With `nowplayingctl`**: Works with all apps ✅
- **Without `nowplayingctl`**: Only Music.app and Spotify ⚠️
- Some apps don't set Now Playing info correctly (rare)

## Contributing

Contributions are welcome! Areas for improvement:

1. **macOS**: Bundle `nowplayingctl` or implement MediaRemote.framework (private API)
2. **All platforms**: Better error handling and retry logic
3. **Testing**: More comprehensive integration tests
4. **Documentation**: Platform-specific troubleshooting guides

## License

MIT License - see LICENSE file for details
