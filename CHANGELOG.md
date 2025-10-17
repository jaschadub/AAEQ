# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/jaschadub/AAEQ/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/jaschadub/AAEQ/compare/v0.1.4...v0.4.0
[0.1.4]: https://github.com/jaschadub/AAEQ/compare/v0.1.0...v0.1.4
[0.1.0]: https://github.com/jaschadub/AAEQ/releases/tag/v0.1.0
