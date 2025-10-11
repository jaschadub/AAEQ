# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/jaschadub/AAEQ/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/jaschadub/AAEQ/compare/v0.1.0...v0.1.4
[0.1.0]: https://github.com/jaschadub/AAEQ/releases/tag/v0.1.0
