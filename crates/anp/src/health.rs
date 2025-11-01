//! Health telemetry implementation for AANP protocol
//!
//! Implements the enhanced health telemetry as specified in the AANP v0.4 specification.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Health message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMessage {
    /// Timestamp in microseconds
    pub timestamp_us: u64,
    /// Connection health
    pub connection: ConnectionHealth,
    /// Playback health
    pub playback: PlaybackHealth,
    /// Latency information
    pub latency: LatencyHealth,
    /// Clock synchronization
    pub clock_sync: ClockHealth,
    /// Integrity information
    pub integrity: IntegrityHealth,
    /// Errors
    pub errors: ErrorHealth,
    /// Volume status
    pub volume: VolumeHealth,
    /// DSP status
    pub dsp: DspHealth,
}

/// Connection health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealth {
    /// Current state
    pub state: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Packets received (lifetime counter)
    pub packets_received: u64,
    /// Packets lost (lifetime counter)
    pub packets_lost: u64,
    /// Bytes received (lifetime counter)
    pub bytes_received: u64,
}

/// Playback health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackHealth {
    /// Current state
    pub state: String,
    /// Buffer size in milliseconds
    pub buffer_ms: f64,
    /// Buffer health indicator
    pub buffer_health: String,
    /// Buffer fill percentage
    pub buffer_fill_percent: u8,
}

/// Latency health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyHealth {
    /// Network latency in milliseconds
    pub network_ms: f64,
    /// Jitter buffer latency in milliseconds
    pub jitter_buffer_ms: f64,
    /// DAC latency in milliseconds
    pub dac_ms: f64,
    /// Pipeline latency in milliseconds
    pub pipeline_ms: f64,
    /// Total latency in milliseconds
    pub total_ms: f64,
}

/// Clock synchronization health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockHealth {
    /// Drift in ppm
    pub drift_ppm: f64,
    /// Phase in microseconds
    pub phase_us: f64,
    /// PLL state
    pub pll_state: String,
    /// Adjustment in ppm
    pub adjustment_ppm: f64,
}

/// Integrity health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityHealth {
    /// CRC OK count (lifetime counter)
    pub crc_ok: u64,
    /// CRC failure count (lifetime counter)
    pub crc_fail: u64,
    /// Last CRC failure sequence
    pub last_crc_fail_seq: Option<u32>,
}

/// Error health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHealth {
    /// Xruns count (lifetime counter)
    pub xruns: u64,
    /// Buffer underruns count (lifetime counter)
    pub buffer_underruns: u64,
    /// Buffer overruns count (lifetime counter)
    pub buffer_overruns: u64,
    /// Last xrun timestamp
    pub last_xrun_timestamp_us: Option<u64>,
}

/// Volume health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeHealth {
    /// Current volume level (0.0-1.0)
    pub level: f32,
    /// Mute state
    pub mute: bool,
    /// Hardware control flag
    pub hardware_control: bool,
    /// Gain in dB
    pub gain_db: f32,
}

/// DSP health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspHealth {
    /// Current profile hash
    pub current_profile_hash: u32,
    /// EQ active
    pub eq_active: bool,
    /// Convolution active
    pub convolution_active: bool,
}

/// Health manager for tracking and reporting system metrics
pub struct HealthManager {
    /// Current timestamp
    timestamp: u64,
    /// Connection metrics
    connection: ConnectionHealth,
    /// Playback metrics
    playback: PlaybackHealth,
    /// Latency metrics
    latency: LatencyHealth,
    /// Clock synchronization metrics
    clock_sync: ClockHealth,
    /// Integrity metrics
    integrity: IntegrityHealth,
    /// Error metrics
    errors: ErrorHealth,
    /// Volume metrics
    volume: VolumeHealth,
    /// DSP metrics
    dsp: DspHealth,
    /// Internal counters
    counters: HealthCounters,
}

/// Health counters for lifetime tracking
#[derive(Debug, Clone, Default)]
pub struct HealthCounters {
    /// Packets received
    pub packets_received: u64,
    /// Packets lost
    pub packets_lost: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// CRC OK
    pub crc_ok: u64,
    /// CRC failures
    pub crc_fail: u64,
    /// Xruns
    pub xruns: u64,
    /// Buffer underruns
    pub buffer_underruns: u64,
    /// Buffer overruns
    pub buffer_overruns: u64,
}

impl HealthManager {
    /// Create a new health manager
    pub fn new() -> Self {
        Self {
            timestamp: Self::get_current_timestamp(),
            connection: ConnectionHealth {
                state: "idle".to_string(),
                uptime_seconds: 0,
                packets_received: 0,
                packets_lost: 0,
                bytes_received: 0,
            },
            playback: PlaybackHealth {
                state: "idle".to_string(),
                buffer_ms: 0.0,
                buffer_health: "good".to_string(),
                buffer_fill_percent: 0,
            },
            latency: LatencyHealth {
                network_ms: 0.0,
                jitter_buffer_ms: 0.0,
                dac_ms: 0.0,
                pipeline_ms: 0.0,
                total_ms: 0.0,
            },
            clock_sync: ClockHealth {
                drift_ppm: 0.0,
                phase_us: 0.0,
                pll_state: "seeking".to_string(),
                adjustment_ppm: 0.0,
            },
            integrity: IntegrityHealth {
                crc_ok: 0,
                crc_fail: 0,
                last_crc_fail_seq: None,
            },
            errors: ErrorHealth {
                xruns: 0,
                buffer_underruns: 0,
                buffer_overruns: 0,
                last_xrun_timestamp_us: None,
            },
            volume: VolumeHealth {
                level: 0.75,
                mute: false,
                hardware_control: false,
                gain_db: -5.1,
            },
            dsp: DspHealth {
                current_profile_hash: 0,
                eq_active: false,
                convolution_active: false,
            },
            counters: Default::default(),
        }
    }

    /// Get current timestamp in microseconds
    fn get_current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Update connection metrics
    pub fn update_connection_metrics(
        &mut self,
        state: &str,
        uptime_seconds: u64,
        packets_received: u64,
        packets_lost: u64,
        bytes_received: u64,
    ) {
        self.connection.state = state.to_string();
        self.connection.uptime_seconds = uptime_seconds;
        self.connection.packets_received = packets_received;
        self.connection.packets_lost = packets_lost;
        self.connection.bytes_received = bytes_received;
        
        // Update counters
        self.counters.packets_received = packets_received;
        self.counters.packets_lost = packets_lost;
        self.counters.bytes_received = bytes_received;
    }

    /// Update playback metrics
    pub fn update_playback_metrics(
        &mut self,
        state: &str,
        buffer_ms: f64,
        buffer_health: &str,
        buffer_fill_percent: u8,
    ) {
        self.playback.state = state.to_string();
        self.playback.buffer_ms = buffer_ms;
        self.playback.buffer_health = buffer_health.to_string();
        self.playback.buffer_fill_percent = buffer_fill_percent;
    }

    /// Update latency metrics
    pub fn update_latency_metrics(
        &mut self,
        network_ms: f64,
        jitter_buffer_ms: f64,
        dac_ms: f64,
        pipeline_ms: f64,
        total_ms: f64,
    ) {
        self.latency.network_ms = network_ms;
        self.latency.jitter_buffer_ms = jitter_buffer_ms;
        self.latency.dac_ms = dac_ms;
        self.latency.pipeline_ms = pipeline_ms;
        self.latency.total_ms = total_ms;
    }

    /// Update clock synchronization metrics
    pub fn update_clock_sync_metrics(
        &mut self,
        drift_ppm: f64,
        phase_us: f64,
        pll_state: &str,
        adjustment_ppm: f64,
    ) {
        self.clock_sync.drift_ppm = drift_ppm;
        self.clock_sync.phase_us = phase_us;
        self.clock_sync.pll_state = pll_state.to_string();
        self.clock_sync.adjustment_ppm = adjustment_ppm;
    }

    /// Update integrity metrics
    pub fn update_integrity_metrics(
        &mut self,
        crc_ok: u64,
        crc_fail: u64,
        last_crc_fail_seq: Option<u32>,
    ) {
        self.integrity.crc_ok = crc_ok;
        self.integrity.crc_fail = crc_fail;
        self.integrity.last_crc_fail_seq = last_crc_fail_seq;
        
        // Update counters
        self.counters.crc_ok = crc_ok;
        self.counters.crc_fail = crc_fail;
    }

    /// Update error metrics
    pub fn update_error_metrics(
        &mut self,
        xruns: u64,
        buffer_underruns: u64,
        buffer_overruns: u64,
        last_xrun_timestamp_us: Option<u64>,
    ) {
        self.errors.xruns = xruns;
        self.errors.buffer_underruns = buffer_underruns;
        self.errors.buffer_overruns = buffer_overruns;
        self.errors.last_xrun_timestamp_us = last_xrun_timestamp_us;
        
        // Update counters
        self.counters.xruns = xruns;
        self.counters.buffer_underruns = buffer_underruns;
        self.counters.buffer_overruns = buffer_overruns;
    }

    /// Update volume metrics
    pub fn update_volume_metrics(
        &mut self,
        level: f32,
        mute: bool,
        hardware_control: bool,
        gain_db: f32,
    ) {
        self.volume.level = level;
        self.volume.mute = mute;
        self.volume.hardware_control = hardware_control;
        self.volume.gain_db = gain_db;
    }

    /// Update DSP metrics
    pub fn update_dsp_metrics(
        &mut self,
        profile_hash: u32,
        eq_active: bool,
        convolution_active: bool,
    ) {
        self.dsp.current_profile_hash = profile_hash;
        self.dsp.eq_active = eq_active;
        self.dsp.convolution_active = convolution_active;
    }

    /// Get current health message
    pub fn get_health_message(&mut self) -> HealthMessage {
        self.timestamp = Self::get_current_timestamp();
        
        HealthMessage {
            timestamp_us: self.timestamp,
            connection: self.connection.clone(),
            playback: self.playback.clone(),
            latency: self.latency.clone(),
            clock_sync: self.clock_sync.clone(),
            integrity: self.integrity.clone(),
            errors: self.errors.clone(),
            volume: self.volume.clone(),
            dsp: self.dsp.clone(),
        }
    }

    /// Get current counters
    pub fn get_counters(&self) -> &HealthCounters {
        &self.counters
    }

    /// Reset counters
    pub fn reset_counters(&mut self) {
        self.counters = Default::default();
    }
}

/// Health metrics collector
pub struct HealthMetricsCollector {
    /// Manager instance
    manager: HealthManager,
    /// Collection interval
    collection_interval: std::time::Duration,
}

impl HealthMetricsCollector {
    /// Create a new collector
    pub fn new() -> Self {
        Self {
            manager: HealthManager::new(),
            collection_interval: std::time::Duration::from_millis(1000), // 1 second
        }
    }

    /// Collect health metrics
    pub fn collect_metrics(&mut self) -> HealthMessage {
        self.manager.get_health_message()
    }

    /// Get current manager
    pub fn get_manager(&mut self) -> &mut HealthManager {
        &mut self.manager
    }
}

/// Health state machine for tracking session states
pub struct HealthStateMachine {
    /// Current state
    pub current_state: HealthState,
    /// State transition history
    pub state_history: Vec<(HealthState, u64)>,
}

/// Health states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    /// Disconnected
    Disconnected,
    /// Idle
    Idle,
    /// Buffering
    Buffering,
    /// Playing
    Playing,
    /// Paused
    Paused,
    /// Error
    Error,
}

impl HealthStateMachine {
    /// Create a new state machine
    pub fn new() -> Self {
        Self {
            current_state: HealthState::Disconnected,
            state_history: Vec::new(),
        }
    }

    /// Transition to a new state
    pub fn transition_to(&mut self, new_state: HealthState) {
        let timestamp = Self::get_current_timestamp();
        self.state_history.push((self.current_state, timestamp));
        self.current_state = new_state;
    }

    /// Get current timestamp
    fn get_current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Get current state
    pub fn get_state(&self) -> HealthState {
        self.current_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_manager_creation() {
        let manager = HealthManager::new();
        assert_eq!(manager.connection.state, "idle");
        assert_eq!(manager.playback.state, "idle");
        assert_eq!(manager.volume.level, 0.75);
    }

    #[test]
    fn test_health_message_serialization() {
        let mut manager = HealthManager::new();
        
        // Update some metrics
        manager.update_connection_metrics(
            "connected",
            3600,
            172800,
            3,
            497664000,
        );
        
        manager.update_playback_metrics(
            "playing",
            140.1,
            "good",
            93,
        );
        
        let health_message = manager.get_health_message();
        
        // Verify fields are populated
        assert_eq!(health_message.connection.state, "connected");
        assert_eq!(health_message.playback.state, "playing");
        assert_eq!(health_message.playback.buffer_ms, 140.1);
        assert_eq!(health_message.timestamp_us, manager.timestamp);
    }

    #[test]
    fn test_health_counters() {
        let mut manager = HealthManager::new();
        
        // Update counters
        manager.update_integrity_metrics(2700, 0, None);
        manager.update_error_metrics(0, 0, 0, None);
        
        let counters = manager.get_counters();
        assert_eq!(counters.crc_ok, 2700);
        assert_eq!(counters.crc_fail, 0);
    }
}