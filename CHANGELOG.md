# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.4] - 2025-10-30

### Added

#### DLNA Device Discovery 🔍
- **SSDP Server**: New DLNA device discovery server for improved network device detection
  - UPnP/SSDP protocol implementation for device advertisement
  - Device description XML generation for DLNA compatibility
  - Enhanced DLNA sink with better device discovery capabilities
  - Supports standard DLNA MediaRenderer device detection

#### Single Instance & Global Hotkeys ⌨️
- **Single Instance Support**: Prevents multiple app instances from running simultaneously
  - Automatic focus on existing instance when launching second time
  - Platform-specific implementation for proper window management
  - Clean shutdown handling and instance detection
- **Global Hotkey System**: Control AAEQ with keyboard shortcuts even when minimized
  - Database persistence for custom hotkey configurations
  - New migration: `016_global_hotkey_settings.sql`
  - Extensible framework for future hotkey bindings

### Fixed

#### macOS Platform Fixes 🍎
- **DMG App Icon**: Fixed missing application icon in macOS DMG installer
  - Proper icon bundling in GitHub Actions build workflow
  - Enhanced macOS build process with correct asset paths
- **Virtual Audio Device Detection**: Improved detection of virtual audio devices on macOS
  - Better compatibility with BlackHole, Loopback, and other virtual audio tools
  - Fixed input device enumeration to include all available virtual devices

#### Build & Release 🔧
- **Default Log Level**: Changed default log level from debug to info for releases
  - Reduces log noise in production builds
  - Debug logging still available via environment variable or settings
  - Cleaner console output for end users

### Changed

#### Documentation Improvements 📚
- **Reorganized Documentation**: Moved detailed content from README to dedicated doc files
  - New `docs/configuration.md`: Comprehensive configuration guide
  - New `docs/development.md`: Development setup and contribution guidelines
  - New `docs/how-it-works.md`: Architecture and design documentation
  - README now focused on quick start and essential information
- **Removed Stale Documentation**: Cleaned up outdated and redundant documentation
  - Removed obsolete `BUILD_SUMMARY.md`, `DEVELOPMENT.md`, `DOCKER.md`
  - Removed outdated implementation docs (M2, M4, SINK_ADAPTERS, STREAM_SERVER, v2_ROADMAP)
  - Updated `TESTING_GUIDE.md` with current best practices
  - Streamlined `CROSS_PLATFORM_MEDIA_DETECTION.md`

#### Technical Improvements
- Enhanced DLNA sink architecture with device discovery capabilities
- Improved application lifecycle management with single instance control
- Better macOS build integration in CI/CD pipeline
- Cleaner codebase with removal of stale documentation artifacts

## [0.6.3] - 2025-10-24

### Added

#### Custom EQ Editor Enhancements 🎨
- **Bezier Curve EQ Editor**: New graphical editing mode for creating smooth, flowing EQ curves
  - Toggle between traditional Bands and new Curve editing modes
  - Interactive 4-point cubic Bezier curve with draggable control points
  - Logarithmic frequency axis (20Hz-20kHz) with grid and labels
  - Real-time visualization of both target curve (orange) and realized frequency response (green)
  - Fit error metric with warning when complex shapes exceed curve capability
  - Bidirectional conversion between curve and band representations
  - Full backward compatibility with existing presets
  - Best for smooth, broad EQ adjustments (gentle bass boost, treble roll-off, etc.)
  - Traditional Bands mode recommended for complex multi-peak/valley shapes

#### UI/UX Improvements 📐
- **Edit Button for Custom EQs**: Added explicit edit button (✏) alongside existing double-click functionality in DSP Server mode
  - Matches the UI pattern used for profile editing
  - Provides clearer, more discoverable editing workflow

### Fixed

#### Genre Override System 🐛
- **Genre Reset Bug**: Fixed genre reverting to old value when clicking refresh icon
  - Changed genre override lookup to use `song_key` (artist-title) instead of `track_key` which included genre
  - Prevents cascading key issues where resetting genre created duplicate database entries
  - Genre refresh now properly resets to device's original genre value

#### Album Art Performance 🚀
- **Reduced API Throttling Risk**: Optimized album art fetching to prevent unnecessary lookups
  - Only updates Now Playing view's track when track or genre actually changes
  - Prevents repeated album art processing on every 1-2 second poll cycle
  - Significantly reduces iTunes API calls for the same playing track

### Changed

#### Technical Improvements 🔧
- Clean warning-free build with all clippy checks passing
- Improved curve fitting algorithm uses strategic frequency points (62Hz, 250Hz, 2kHz, 8kHz)
- Helper tooltip explains curve editor limitations for complex EQ shapes

## [0.6.2] - 2025-10-22

### Fixed

#### DSP Audio Quality 🎵
- **Resampling Audio Issues**: Fixed garbled/slowed audio and incorrect volume when resampling is enabled
  - Output sink now correctly configured with target sample rate instead of input rate
  - Audio plays at proper speed and volume when resampling from 48kHz to 96kHz
  - Auto-restart stream when resampling settings change output rate (toggle on/off, change target rate)
  - Quality changes (Fast/Balanced/High/Ultra) update live without restart

- **Dithering Hiss**: Removed profound hiss when using dithering/noise shaping
  - Disabled DSP dithering in float domain which caused audible artifacts
  - Dithering now only happens during format conversion (S16LE/S24LE) where appropriate
  - Proper TPDF dither applied automatically during PCM conversion

#### DLNA Streaming Latency 🚀
- **EQ Change Responsiveness**: Reduced latency from 10+ seconds to <1 second
  - Changed from buffering entire audio stream to streaming in 200ms chunks
  - Reduced polling interval from 50ms to 10ms for faster response
  - Limited buffer to 1 second of audio (down from 10MB/~30 seconds)
  - EQ changes now audible almost immediately

### Changed

#### User Interface Improvements 📐
- **DSP Server Tab**: Reorganized layout for better ergonomics
  - Moved EQ Status next to Start/Stop Streaming button with separator (always visible)
  - Arranged Dithering and Resampling sections side-by-side to reduce vertical scrolling
  - More compact UI with better use of horizontal space

- **Settings Tab Cleanup**: Removed duplicate DSP settings section
  - DSP settings already available (with more options) in DSP Server tab
  - All DSP configuration now centralized in one location
  - Settings tab now focused on app-wide settings (theme, debug logging, database)

#### Technical Improvements 🔧
- Settings auto-save immediately when changed in DSP Server tab
- Smart restart logic: only restarts stream when output rate changes
- Quality adjustments apply live without interruption
- Improved buffer management for lower latency across all sinks

## [0.6.1] - 2025-10-22

### Fixed

#### Build & Release 🔧
- **Windows MSI Installer**: Fixed WiX linker error preventing Windows installer builds
  - Updated license file path to use cargo-wix Mustache template variable `{{eula}}`
  - Corrected icon file path reference in WiX configuration
  - Removed obsolete root `wix/` directory
  - Windows installer builds now succeed in CI/CD pipeline

### Changed

#### Code Quality Improvements 🧹
- **Implemented FromStr trait** for Scope enum with proper error handling
- **Improved iterator usage** by replacing needless range loops with iterator-based approaches (8 instances)
- **Fixed documentation formatting** by removing empty lines after doc comments (15+ instances)
- **Modernized numeric constants** by replacing legacy `std::f64::INFINITY` with `f64::INFINITY`
- **Simplified default implementations** by using `#[derive(Default)]` with `#[default]` attribute for enums
- **Improved struct initialization** patterns using struct update syntax

#### Technical Details
- All changes pass `cargo clippy --all-features --all-targets -- -D warnings` for core libraries
- Fixed compilation issues in core, media-session, persistence, and stream-server crates
- Improved type safety and error handling across repository interfaces

## [0.6.0] - 2025-10-21

### Added

#### Cross-Platform Media Detection 🌍
- **Universal Streaming Service Support**: Now Playing detection works with ALL major streaming services across all platforms
  - **Spotify**: ✅ All platforms
  - **Tidal**: ✅ All platforms
  - **YouTube Music**: ✅ All platforms
  - **Amazon Music**: ✅ All platforms
  - **Deezer**: ✅ All platforms
  - **Apple Music/iTunes**: ✅ All platforms
  - And many more...

- **New Media-Session Crate**: Trait-based abstraction for cross-platform media detection
  - `MediaSession` trait with unified interface
  - `MediaMetadata` type for track information
  - Platform-specific implementations via conditional compilation

- **Linux (MPRIS)**: D-Bus integration for all MPRIS-compatible players
  - Works with Spotify, Strawberry, VLC, browsers, and more
  - Prioritizes dedicated music players over browsers
  - Full metadata extraction (title, artist, album, genre, artwork)

- **Windows (SMTC)**: System Media Transport Controls integration
  - Universal support for all modern Windows apps
  - Works with Spotify, Tidal, iTunes, YouTube Music (browser), Amazon Music
  - Native Windows Runtime API via `windows` crate
  - Automatic async operation handling

- **macOS (System-Wide + AppleScript)**: Dual detection methods
  - System-wide support via `nowplayingctl` for ALL streaming services
  - AppleScript fallback for Music.app and Spotify
  - Setup instructions: `brew install nowplayingctl`

#### DSP Pipeline Enhancements 🎛️
- **High-Quality Audio Resampling**: Professional sinc-based sample rate conversion
  - Four quality presets: Fast, Balanced, High, Ultra
  - Support for 44.1kHz, 48kHz, 88.2kHz, 96kHz, 192kHz
  - Powered by `rubato` library with sinc interpolation

- **Dithering & Noise Shaping**: Industry-standard bit depth reduction
  - Four dither modes: None, Rectangular, TPDF (Triangular), Gaussian
  - Four noise shaping algorithms: None, First Order, Second Order, Gesemann
  - Configurable target bit depth (8-24 bits)
  - Professional audio quality for lossy format conversion

- **Headroom Control**: Prevent clipping in DSP pipeline
  - Configurable headroom reduction (e.g., -3 dB)
  - Clip detection and logging
  - Applied before EQ to prevent distortion

- **Pipeline Visualization**: Real-time DSP chain display
  - Visual representation: Input → Headroom → EQ → Resample → Dither → Output
  - Clickable stage controls for quick configuration
  - Status indicators for each processing stage

- **Profile-Based DSP Settings**: Per-profile DSP configuration
  - Each profile can have unique sample rate, format, headroom, resampling, dithering
  - Settings persist per profile in database
  - Perfect for different listening scenarios (Headphones vs Speakers)

#### Documentation 📚
- **Streaming Service Support Guide**: User-friendly compatibility documentation
  - Platform-specific setup instructions
  - Troubleshooting section for common issues
  - Detailed service compatibility matrix

- **Cross-Platform Media Detection**: Technical implementation guide
  - Architecture overview and design decisions
  - Platform-specific API documentation
  - Testing strategies and CI/CD setup

- **Updated README**: Comprehensive streaming service information
  - Compatibility table for all major services
  - macOS setup instructions for `nowplayingctl`
  - Links to detailed documentation

### Fixed

#### UI/UX Improvements 🐛
- **Device Cache Warning**: Visual notification when DLNA/AirPlay device cache is empty
  - Orange warning appears when device is selected but cache is empty (e.g., after restart)
  - Clear instruction: "Click '🔍 Discover' to find devices on your network"
  - Prevents confusion when trying to stream without discovery

- **Improved Error Messages**: Better error feedback for streaming failures
  - Clear actionable messages: "Device 'X' not found. Click '🔍 Discover'..."
  - Removed technical jargon from user-facing errors

### Changed

#### Architecture Improvements
- **Media Detection Abstraction**: Replaced platform-specific calls with unified API
  - `crate::mpris::get_now_playing_mpris()` → `crate::media::get_now_playing()`
  - Single interface works across Linux, Windows, and macOS
  - Clean separation between platform detection and UI integration

- **Module Organization**: New media module for cross-platform integration
  - `crates/ui-egui/src/media.rs`: Bridge between media-session and AAEQ
  - Provides compatibility layer for `TrackMeta` conversion

#### CI/CD Enhancements
- **Cross-Platform Build Checks**: Automated compilation verification
  - Matrix build strategy: `[ubuntu-latest, windows-latest, macos-latest]`
  - Runs on every push/PR to catch platform-specific issues early
  - `fail-fast: false` ensures all platforms are tested

### Technical Details

#### New Crates
- `crates/media-session/`: Cross-platform media detection
  - `lib.rs`: Core trait definitions and factory function
  - `linux.rs`: MPRIS implementation (311 lines)
  - `windows.rs`: SMTC implementation (225 lines)
  - `macos.rs`: System-wide + AppleScript (267 lines)

#### New Modules
- `crates/ui-egui/src/media.rs`: Media session integration layer
- `crates/ui-egui/src/pipeline_view.rs`: DSP pipeline visualization
- `crates/stream-server/src/dsp/`: Organized DSP modules
  - `mod.rs`: Module organization
  - `eq.rs`: Parametric EQ (moved from dsp.rs)
  - `headroom.rs`: Headroom control with clip detection
  - `resampler.rs`: High-quality sample rate conversion
  - `dither.rs`: Dithering and noise shaping algorithms

#### Database Schema
- `010_dsp_profile_settings.sql`: Profile-based DSP configuration
- `011_dsp_dithering_settings.sql`: Dithering parameters storage

#### Dependencies
- `windows = "0.58"`: Windows Runtime bindings for SMTC
- `rubato`: High-quality audio resampling library

#### Performance
- Efficient media detection with minimal overhead
- Cached media session instances (OnceLock pattern)
- Platform-native implementations for best performance

## [0.5.1] - 2025-10-19

### Fixed

#### DLNA Visualization Sync 🎵
- **Automatic Delay Detection**: Visualization now automatically syncs with DLNA/UPnP device playback
  - Auto-detects ~4 second device buffer latency
  - Immediate status update on stream start for faster sync
  - Visualization stays in sync when songs change
- **Improved Latency Reporting**: DLNA sink now correctly reports end-to-end latency including device buffering
  - Base DLNA latency increased from 150ms to 4000ms to match real-world behavior
  - Accounts for network streaming and device buffer delays
- **Extended Delay Controls**: Visualization delay slider extended to 0-5000ms range
  - Manual adjustment available for fine-tuning sync
  - Auto-detected delay can be overridden if needed
- **Larger Visualization Buffers**: Increased buffer capacity to support longer delays
  - Sample buffer: 5s → 10s capacity (120 → 240 items)
  - Metrics buffer: doubled to handle extended delays (500 → 1000 items)
  - Prevents buffer overflow causing flat visualization
- **Debug Logging**: Added comprehensive logging for visualization sync troubleshooting
  - Buffer status and processing metrics
  - Auto-detection trigger events
  - Delay adjustment tracking

## [0.5.0] - 2025-10-19

### Added

#### UI/UX Improvements 🎨
- **Spectrum Analyzer**: New real-time spectrum analyzer visualization mode
  - FFT-based frequency analysis with customizable bar display
  - Toggle between Waveform and Spectrum visualization modes
  - Theme-aware colors for bars and frequency labels
- **Theme System**: Multiple color themes for the entire application
  - Dark, Light, WinAmp, Vintage, and Studio themes
  - Theme selection persisted in database
  - Coordinated colors for meters, spectrum analyzer, and UI elements
- **Settings Tab**: New Settings tab with enhanced functionality
  - Theme selector with preview
  - Database backup/restore management
  - About section with version, author (with clickable links), license, and project URL
- **Audio Level Meters**: Pre/Post-EQ audio meters with ballistics
  - MC-style meters with peak hold and smooth decay
  - Toggle visibility to save screen space
  - Theme-aware color gradients

#### Audio Processing
- **Improved Audio Pipeline**: Enhanced DSP processing with better metrics
  - Separate pre-EQ and post-EQ audio metering
  - Better visualization of audio processing chain

### Fixed

#### Audio Quality 🐛
- **Local DAC Hissing**: Fixed audio quality issues with local DAC output
  - Resolved noise/hissing artifacts during playback
- **AirPlay Compatibility**: Improved AirPlay device compatibility and stability

### Changed

#### UI Enhancements
- **Collapsible Audio Output**: DSP Server audio controls can be collapsed
  - Reduces clutter when not actively configuring
  - Window automatically resizes based on visible elements
- **Improved Spectrum Display**: Better frequency labeling and bar spacing
- **Enhanced Visual Feedback**: Better loading states and status indicators

### Technical Details

#### New Modules
- `crates/ui-egui/src/theme.rs`: Theme system with multiple color schemes
- `crates/ui-egui/src/spectrum_analyzer.rs`: Real-time FFT spectrum analysis
- `crates/ui-egui/src/meter.rs`: Professional audio level meters

#### Dependencies
- Enhanced audio visualization capabilities
- Improved DSP processing pipeline

## [0.4.1] - 2025-10-17

### Added

#### Album Art Lookup 🎨
- **External Album Art Lookup**: Integrated iTunes Search API for album artwork
- **High-Resolution Images**: Album art displayed at 600x600 resolution
- **Smart Caching**: New `load_as()` function prevents cache key collisions
- **Async Loading**: Non-blocking album art fetches with loading state tracking
- **Lookup URL Scheme**: Introduced `lookup://Artist|Album` for external lookups

#### Windows Platform Improvements
- **Console Control**: Added `--console` flag to show/hide console window
- **Embedded Icons**: Album art fallback icon compiled into binary
- **Audio Device Detection**: Improved Windows loopback device enumeration

### Fixed

#### Album Art Issues 🐛
- **WiiM API Album Art**: Fixed album art loading in WiiM API mode
  - WiiM devices don't provide `/Artwork` endpoint (returns 404)
  - Implemented fallback to iTunes Search API lookup
  - Graceful degradation to default icon on failures
- **Cache Key Collisions**: Fixed images cached under wrong keys
  - Separated lookup keys from actual image URLs
  - Ensured proper state tracking during async loads
- **Tray Window Visibility**: Fixed show/hide functionality on Windows

### Changed

#### UI/UX Improvements
- **Profile Examples**: Updated README from "Car" to "Living Room" references
- **Custom EQ Visibility**: Hidden custom presets in WiiM API mode (device limitation)
- **Debug Logging**: Enhanced album art troubleshooting with comprehensive logs
- **Loading States**: Better visual feedback during album art lookup

### Technical Details

#### New Modules
- `crates/ui-egui/src/album_art_lookup.rs`: iTunes Search API integration
- Enhanced `AlbumArtCache` with `load_as()`, `mark_loading()`, `mark_failed()`

#### Dependencies
- Added `urlencoding = "2.1"` for safe API queries

## [0.4.0] - 2025-10-16

### Added

#### Multiple Profiles Support 🎯
- **Profile Management**: Create and manage multiple EQ profiles (e.g., "Headphones", "Speakers", "Car")
- **Profile Switching**: Instantly switch between profiles with automatic preset reapplication
- **Profile Persistence**: Active profile saved and restored across app restarts
- **Per-Profile Mappings**: Each profile maintains separate song/album/genre → preset mappings
- **Profile UI**: Dropdown selector with create, rename, and delete dialogs
- **Built-in Profiles**: Default profile created automatically, cannot be deleted

#### Audio Improvements
- **Format Auto-Fallback**: LocalDacSink now automatically falls back between F32 ↔ S16LE formats
- **Device Detection**: Check supported audio formats before opening stream to prevent failures
- **Setup Script Options**: `--with-loopback` flag for optional audio loopback creation
- **Improved Documentation**: Step-by-step AAEQ usage instructions in setup script

#### UI/UX Enhancements
- **Logo Display**: Added AAEQ logo to README header for better branding
- **XFCE Tray Fix**: Added helpful diagnostics and instructions for XFCE users
- **Error Messages**: Smart detection of desktop environment with actionable guidance

### Fixed

#### Critical Bug Fixes 🐛
- **Device Persistence**: Input/Output device selections now persist correctly across restarts
  - Fixed `INSERT OR REPLACE` SQL pattern that was clearing other columns
  - Changed to `UPDATE` pattern to preserve all settings independently
  - Applies to: last_connected_host, last_input_device, last_output_device, active_profile_id

- **Genre Persistence**: Manual genre edits now persist correctly
  - Genre overrides now loaded on every poll, not just on track change
  - Fixed in both WiiM and MPRIS (DSP) polling paths
  - Manual genre changes no longer revert to "Unknown"

#### Audio Device Compatibility
- Automatic format fallback prevents device initialization failures
- Better handling of devices with limited format support
- Improved error messages for audio device issues

### Changed

#### Database Schema
- Added `profile` table for storing user profiles
- Added `profile_id` column to `mapping` table (foreign key)
- Added `active_profile_id` to `app_settings` table
- Updated `mapping` unique constraint to include `profile_id`
- Migration: `007_profiles.sql`

#### Setup Script
- Default behavior: only creates virtual sink (no loopback)
- Loopback now optional with `--with-loopback` flag
- Clearer instructions for AAEQ integration
- Warning about double audio when using both loopback and AAEQ

#### Documentation
- Added comprehensive Multiple Profiles section to README
- Updated Known Limitations with detailed XFCE tray icon solution
- Added profile database tables to schema documentation
- Improved audio capture setup instructions

### Technical Details

#### Architecture Changes
- Profile-scoped rule resolution in async worker
- Profile switching triggers full rules index reload
- `ReapplyPresetForCurrentTrack` command for profile changes
- Profile repository with full CRUD operations

#### Performance
- Efficient profile switching with targeted database queries
- Rules index cached per profile for fast lookups
- Minimal overhead for profile management

## [0.1.4] - 2025-10-11

### Fixed
- HTML entity decoding in song names for proper EQ matching
- Visual feedback for EQ application status

### Added
- Database backup and restore functionality
- ARM64 (aarch64) Linux build support in release workflow

## [0.1.0] - 2025-10-10

### Added
- Initial release of AAEQ (Adaptive Audio Equalizer)
- WiiM/LinkPlay device support via HTTP API
- Automatic EQ preset switching based on song, album, or genre
- Manual genre editing for tracks without metadata
- Last connected IP persistence and auto-connect on startup
- SQLite-based local storage with migrations
- Cross-platform desktop GUI using egui/eframe
- GitHub Actions CI/CD for Linux, macOS, and Windows builds
- Multi-architecture Docker support (amd64/arm64)

### Features
- **Smart EQ Resolution**: Hierarchical preset matching (Song → Album → Genre → Default)
- **Genre Override System**: Manually assign and persist genres for tracks lacking metadata
- **Real-time Polling**: Monitors WiiM device every second for track changes
- **Preset Management**: List, apply, and save EQ preset mappings
- **Connection Management**: Remembers last connected device IP
- **Debouncing**: Only applies preset changes when necessary to reduce device calls

### Technical
- Built with Rust 1.89+ for performance and safety
- Async architecture using tokio runtime
- GUI framework: egui 0.29 with eframe
- Database: SQLite with SQLx 0.8 (offline mode for Docker builds)
- Device API: WiiM LinkPlay HTTP API
- Edition 2024 Rust features

---

## Release Guidelines

- **Major version (X.0.0)**: Breaking changes to APIs or data formats
- **Minor version (0.X.0)**: New features, non-breaking changes
- **Patch version (0.0.X)**: Bug fixes, minor improvements

[Unreleased]: https://github.com/jaschadub/AAEQ/compare/v0.6.4...HEAD
[0.6.4]: https://github.com/jaschadub/AAEQ/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/jaschadub/AAEQ/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/jaschadub/AAEQ/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/jaschadub/AAEQ/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/jaschadub/AAEQ/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/jaschadub/AAEQ/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/jaschadub/AAEQ/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/jaschadub/AAEQ/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/jaschadub/AAEQ/compare/v0.1.4...v0.4.0
[0.1.4]: https://github.com/jaschadub/AAEQ/compare/v0.1.0...v0.1.4
[0.1.0]: https://github.com/jaschadub/AAEQ/releases/tag/v0.1.0
