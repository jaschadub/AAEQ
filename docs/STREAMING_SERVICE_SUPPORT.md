# Streaming Service Support

AAEQ can detect "Now Playing" information from virtually any music streaming service on Linux, Windows, and macOS. This guide explains how to ensure your favorite service is supported.

## Quick Reference

| Platform | Services Supported | Setup Required |
|----------|-------------------|----------------|
| **Linux** | All MPRIS-compatible apps | ✅ None - works out of the box |
| **Windows** | All SMTC-compatible apps | ✅ None - works out of the box |
| **macOS** | All apps (with tool installed) | ⚠️ Install `nowplayingctl` recommended |

## Supported Streaming Services

### ✅ Fully Supported (All Platforms)

These services work on **all platforms** without any additional setup:

- **Spotify** - Desktop app
- **Apple Music / iTunes** - Native apps
- **YouTube Music** - Browser or desktop app
- **Amazon Music** - Desktop app
- **Deezer** - Desktop app
- **SoundCloud** - Browser or desktop app
- **Pandora** - Browser or desktop app
- **And many more...**

### Platform-Specific Notes

#### 🐧 Linux

**Works with**: Any app that implements MPRIS2 (Media Player Remote Interfacing Specification)

**Commonly supported apps**:
- Spotify ✅
- Strawberry Music Player ✅
- VLC ✅
- Clementine ✅
- Rhythmbox ✅
- Firefox/Chrome web players ✅
- And most other Linux music players

**Setup**: None required - if your app shows up in media controls, it will work!

#### 🪟 Windows

**Works with**: Any app that implements System Media Transport Controls (SMTC)

**Commonly supported apps**:
- Spotify ✅
- iTunes ✅
- Apple Music ✅
- Tidal ✅
- YouTube Music (browser) ✅
- Amazon Music ✅
- Deezer ✅
- And most modern Windows apps

**Requirements**: Windows 10 version 1803 or later (most users have this)

**Setup**: None required - if your app shows up in Windows media controls (volume overlay), it will work!

#### 🍎 macOS

**Two detection methods**:

##### Method 1: System-Wide (Recommended) ✅

**Works with**: **ALL** streaming services including:
- Spotify ✅
- Tidal ✅
- YouTube Music ✅
- Amazon Music ✅
- Deezer ✅
- SoundCloud ✅
- Apple Music ✅
- And virtually any other music app

**Setup Required**: Install `nowplayingctl`:

```bash
brew install nowplayingctl
```

That's it! AAEQ will automatically detect and use it.

##### Method 2: AppleScript (Fallback) ⚠️

**Works with**: Only Music.app and Spotify

**Setup**: None - works out of the box, but **only** supports Music.app and Spotify

### Why is macOS different?

macOS has native system-wide media detection, but accessing it requires an additional tool (`nowplayingctl`). Without it, AAEQ falls back to AppleScript which only works with Music.app and Spotify.

**Bottom line**: Install `nowplayingctl` on macOS for the best experience!

## Troubleshooting

### "Now Playing" not detected

1. **Check if the app is playing**
   - Make sure music is actually playing (not paused)
   - Some apps only publish metadata when playing, not when paused

2. **Linux**: Check MPRIS support
   ```bash
   # List available MPRIS players
   dbus-send --session --print-reply --dest=org.freedesktop.DBus \
     /org/freedesktop/DBus org.freedesktop.DBus.ListNames | grep mpris
   ```

   If your app doesn't show up, it may not support MPRIS.

3. **Windows**: Check media controls
   - Press the volume keys and see if the media overlay shows your app
   - If it doesn't appear, the app may not support SMTC
   - Try restarting the app

4. **macOS**: Install nowplayingctl
   ```bash
   # Check if nowplayingctl is installed
   which nowplayingctl

   # If not found, install it
   brew install nowplayingctl

   # Test it
   nowplayingctl get title artist album
   ```

### App shows up but metadata is wrong

Some apps don't properly implement media controls:
- **Browser players**: Sometimes show the webpage title instead of song title
- **Solution**: Use the dedicated desktop app when available
- **Example**: Use Spotify desktop app instead of Spotify Web Player

### Tidal not working on macOS

**Solution**: Install `nowplayingctl`:
```bash
brew install nowplayingctl
```

Without this tool, only Music.app and Spotify are supported on macOS.

## Testing Your Setup

### Linux
```bash
# Start playing music, then:
dbus-send --session --print-reply --dest=org.mpris.MediaPlayer2.* \
  /org/mpris/MediaPlayer2 org.freedesktop.DBus.Properties.Get \
  string:org.mpris.MediaPlayer2.Player string:Metadata
```

### Windows
Start playing music and check if the media overlay (volume keys) shows your app.

### macOS
```bash
# With nowplayingctl installed:
nowplayingctl get title artist album

# Should show currently playing track
```

## Developer Information

For developers interested in how this works, see:
- [CROSS_PLATFORM_MEDIA_DETECTION.md](./CROSS_PLATFORM_MEDIA_DETECTION.md) - Technical documentation
- `crates/media-session/` - Source code
- `crates/media-session/README.md` - API documentation

## Contributing

Found a streaming service that doesn't work? [Open an issue](https://github.com/jaschadub/AAEQ/issues) with:
- Platform (Linux/Windows/macOS)
- App name and version
- Debug logs (`RUST_LOG=debug aaeq`)

We'll investigate and add support if possible!
