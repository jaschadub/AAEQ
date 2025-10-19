# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/jaschadub/AAEQ/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/jaschadub/AAEQ/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/jaschadub/AAEQ/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/jaschadub/AAEQ/compare/v0.1.4...v0.4.0
[0.1.4]: https://github.com/jaschadub/AAEQ/compare/v0.1.0...v0.1.4
[0.1.0]: https://github.com/jaschadub/AAEQ/releases/tag/v0.1.0
