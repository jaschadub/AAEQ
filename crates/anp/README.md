# AANP Protocol Implementation

This crate implements the AAEQ Node Protocol (ANP) v0.4 specification for high-fidelity, low-latency network audio streaming.

## Overview

The AANP protocol provides a comprehensive solution for network audio streaming with features including:
- High-fidelity, low-latency audio transport
- Advanced clock synchronization with Micro-PLL
- Bit-perfect delivery with CRC verification
- Remote volume control with multiple curve types
- Comprehensive health telemetry
- Robust error handling and recovery

## Key Features

### Protocol Components

1. **RTP Transport** - Audio packet handling with proper endianness
2. **WebSocket Control** - Snake_case JSON messaging for control
3. **mDNS Discovery** - Node advertisement with UUID-first TXT records
4. **Session Management** - Feature negotiation and session lifecycle
5. **Health Telemetry** - Lifetime counters and detailed metrics
6. **Error Handling** - Standardized error codes and recovery protocols

### Protocol Compliance

- Implements all v0.4 specification requirements
- Proper RTP header structure with correct field values
- Network byte order handling for S24LE samples
- Standardized error codes (E101-E602)
- Complete session negotiation flow

## Usage

```rust
use anp_protocol::{SessionManager, HealthManager, FeatureSet};

// Initialize session
let mut session_manager = SessionManager::new();
let session_init = SessionInit {
    protocol_version: "0.4".to_string(),
    node_uuid: uuid::Uuid::new_v4(),
    features: vec!["micro_pll".to_string(), "crc_verify".to_string()],
    optional_features: vec!["dsp_transfer".to_string()],
    // ... other fields
};

// Process session initialization
let session_accept = session_manager.initialize_session(&session_init).unwrap();
```

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   AANP Protocol │    │   Network Layer │    │   Hardware      │
│   Implementation │    │   (RTP/WebSocket)│    │   (Audio Devices) │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                       │                       │
          │    ┌─────────────────┐    │                       │
          │    │   Session Layer   │    │                       │
          │    │   (Negotiation) │    │                       │
          │    └─────────┬───────┘    │                       │
          │              │           │                       │
          │    ┌─────────▼───────┐    │                       │
          │    │   Transport Layer │    │                       │
          │    │   (RTP/Packet)   │    │                       │
          │    └─────────┬───────┘    │                       │
          │              │           │                       │
          │    ┌─────────▼───────┐    │                       │
          │    │   Control Layer   │    │                       │
          │    │   (WebSocket)   │    │                       │
          │    └─────────────────┘    │                       │
          └───────────────────────────┴───────────────────────────┘
```

## API Reference

### Core Modules

- `protocol` - Protocol constants and enumerations
- `discovery` - mDNS service discovery
- `session` - Session management and negotiation
- `rtp` - RTP packet handling
- `websocket` - WebSocket control channel
- `health` - Health telemetry and metrics
- `errors` - Error handling and recovery
- `features` - Feature negotiation and management

## Compliance

This implementation strictly follows the AANP v0.4 specification with:
- Proper RTP header structure
- Correct endianness handling for audio samples
- Standardized error codes and recovery protocols
- Complete feature negotiation framework
- Comprehensive health telemetry system

## License

MIT License