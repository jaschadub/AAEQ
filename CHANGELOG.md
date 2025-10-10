# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of AAEQ (Adaptive Audio Equalizer)
- Support for WiiM/LinkPlay devices
- Automatic EQ preset switching based on song, album, and genre
- Manual genre editing for tracks without metadata
- SQLite-based local storage
- Cross-platform GUI using egui
- Multi-platform builds (Linux, macOS, Windows)
- Docker support

### Features
- **Smart EQ Resolution**: Song → Album → Genre → Default priority
- **Genre Override System**: Manually assign genres to tracks
- **Real-time Polling**: Polls WiiM device every second for track changes
- **Preset Management**: List, apply, and save mappings for device presets
- **Debouncing**: Only applies preset changes when necessary

### Technical
- Built with Rust for performance and safety
- Async architecture using tokio
- GUI framework: egui
- Database: SQLite with SQLx
- Device API: WiiM LinkPlay HTTP API

## [0.1.0] - YYYY-MM-DD

### Added
- Initial public release
- Basic WiiM device support
- Core EQ mapping functionality
- Desktop GUI application

---

## Release Guidelines

- **Major version (X.0.0)**: Breaking changes to APIs or data formats
- **Minor version (0.X.0)**: New features, non-breaking changes
- **Patch version (0.0.X)**: Bug fixes, minor improvements

[Unreleased]: https://github.com/YOUR_USERNAME/AAEQ/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/YOUR_USERNAME/AAEQ/releases/tag/v0.1.0
