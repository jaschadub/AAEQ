/// Type definitions for the Control API

use crate::types::OutputConfig;
use serde::{Deserialize, Serialize};

/// Response for GET /v1/outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputsResponse {
    pub outputs: Vec<OutputInfo>,
    pub active: Option<String>,
}

/// Information about an available output sink
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputInfo {
    pub name: String,
    pub is_open: bool,
    pub is_active: bool,
    pub config: Option<OutputConfig>,
    pub latency_ms: u32,
}

/// Request for POST /v1/outputs/select
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOutputRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    pub config: OutputConfig,
}

/// Response for POST /v1/outputs/select
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOutputResponse {
    pub success: bool,
    pub message: String,
    pub active_output: Option<String>,
}

/// Response for GET /v1/outputs/metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub output_name: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub format: Option<String>,
    pub latency_ms: u32,
    pub underruns: u64,
    pub overruns: u64,
    pub bytes_written: u64,
}

/// Request for POST /v1/route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub input: String,  // "SystemMix" | "App" | "File"
    pub output: String, // "dlna" | "dac" | "airplay"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<OutputConfig>,
}

/// Response for GET /v1/route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    pub input: Option<String>,
    pub output: Option<String>,
    pub device: Option<String>,
    pub is_active: bool,
}

/// Response for GET /v1/capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub outputs: Vec<OutputCapability>,
}

/// Capability information for an output type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputCapability {
    pub name: String,
    pub supported_sample_rates: Vec<u32>,
    pub supported_formats: Vec<String>,
    pub min_channels: u16,
    pub max_channels: u16,
    pub supports_exclusive: bool,
    pub requires_device_discovery: bool,
}

/// Generic success response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

impl OutputCapability {
    pub fn for_local_dac() -> Self {
        Self {
            name: "local_dac".to_string(),
            supported_sample_rates: vec![44100, 48000, 88200, 96000, 176400, 192000],
            supported_formats: vec![
                "F32".to_string(),
                "S24LE".to_string(),
                "S16LE".to_string(),
            ],
            min_channels: 1,
            max_channels: 8,
            supports_exclusive: true,
            requires_device_discovery: false,
        }
    }

    pub fn for_dlna() -> Self {
        Self {
            name: "dlna".to_string(),
            supported_sample_rates: vec![44100, 48000, 96000, 192000],
            supported_formats: vec!["S24LE".to_string(), "S16LE".to_string()],
            min_channels: 2,
            max_channels: 2,
            supports_exclusive: false,
            requires_device_discovery: true,
        }
    }

    pub fn for_airplay() -> Self {
        Self {
            name: "airplay".to_string(),
            supported_sample_rates: vec![44100, 48000],
            supported_formats: vec!["S16LE".to_string()],
            min_channels: 2,
            max_channels: 2,
            supports_exclusive: false,
            requires_device_discovery: true,
        }
    }
}
