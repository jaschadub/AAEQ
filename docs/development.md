# Development

## Prerequisites

- Rust 1.75+ (stable)
- SQLite development libraries
- **Linux only**: GTK3, libxdo, and libappindicator3 (see Build from Source section in README)

## Running in Development

```bash
cargo run
```

## Running Tests

```bash
cargo test
```

## Code Style

```bash
cargo fmt
cargo clippy
```

## Project Structure

```
AAEQ/
├── apps/
│   └── desktop/          # Main desktop application
├── crates/
│   ├── core/             # Core logic and models
│   ├── device-wiim/      # WiiM device integration
│   ├── media-session/    # Cross-platform Now Playing detection
│   ├── persistence/      # SQLite database layer
│   ├── stream-server/    # DSP engine and streaming
│   └── ui-egui/          # egui-based UI with DSP controls
├── docs/                 # Implementation documentation
├── migrations/           # Database migrations
└── setup-audio-loopback.sh  # Audio capture setup script
```
