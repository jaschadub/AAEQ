//! mDNS discovery implementation for AANP nodes
//!
//! Implements the mDNS TXT record format as specified in the AANP v0.4 specification.

use std::collections::HashMap;
use std::net::IpAddr;
use uuid::Uuid;

/// Node discovery record for mDNS advertising
#[derive(Debug, Clone)]
pub struct NodeDiscoveryRecord {
    /// Unique node identifier (UUID)
    pub uuid: Uuid,
    /// Protocol version
    pub version: String,
    /// Supported sample rates
    pub supported_sample_rates: Vec<u32>,
    /// Supported bit depths
    pub supported_bit_depths: Vec<String>,
    /// Number of channels
    pub channels: u8,
    /// Core features
    pub core_features: Vec<String>,
    /// Optional features
    pub optional_features: Vec<String>,
    /// WebSocket control URL
    pub control_url: Option<String>,
    /// Current state
    pub state: String,
    /// Current volume (0-100)
    pub volume: u8,
    /// DAC name
    pub dac_name: Option<String>,
    /// Hardware platform
    pub hardware_platform: Option<String>,
}

impl NodeDiscoveryRecord {
    /// Create a new discovery record
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            version: "0.4.0".to_string(),
            supported_sample_rates: vec![44100, 48000, 96000, 192000],
            supported_bit_depths: vec!["S16".to_string(), "S24".to_string(), "F32".to_string()],
            channels: 2,
            core_features: vec![
                "pll".to_string(),
                "crc".to_string(),
                "vol".to_string(),
                "gap".to_string(),
                "cap".to_string(),
            ],
            optional_features: vec![
                "dsp".to_string(),
                "conv".to_string(),
            ],
            control_url: None,
            state: "idle".to_string(),
            volume: 75,
            dac_name: None,
            hardware_platform: None,
        }
    }

    /// Convert to mDNS TXT record format
    pub fn to_txt_record(&self) -> HashMap<String, String> {
        let mut record = HashMap::new();
        
        // Required fields according to specification
        record.insert("uuid".to_string(), self.uuid.to_string());
        record.insert("v".to_string(), self.version.clone());
        record.insert("sr".to_string(), 
            self.supported_sample_rates
                .iter()
                .map(|sr| sr.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        record.insert("bd".to_string(), 
            self.supported_bit_depths
                .join(",")
        );
        record.insert("ch".to_string(), self.channels.to_string());
        record.insert("ft".to_string(), self.core_features.join(","));
        record.insert("opt".to_string(), self.optional_features.join(","));
        
        // Optional fields
        if let Some(url) = &self.control_url {
            record.insert("ctrl".to_string(), url.clone());
        }
        record.insert("st".to_string(), self.state.clone());
        record.insert("vol".to_string(), self.volume.to_string());
        
        if let Some(dac) = &self.dac_name {
            record.insert("dac".to_string(), dac.clone());
        }
        
        if let Some(hw) = &self.hardware_platform {
            record.insert("hw".to_string(), hw.clone());
        }
        
        record
    }

    /// Parse from mDNS TXT record format
    pub fn from_txt_record(record: &HashMap<String, String>) -> Result<Self, String> {
        let uuid_str = record.get("uuid").ok_or("Missing UUID")?;
        let uuid = Uuid::parse_str(uuid_str).map_err(|_| "Invalid UUID format")?;
        
        let version = record.get("v").cloned().unwrap_or_else(|| "0.4.0".to_string());
        
        let supported_sample_rates = record.get("sr")
            .map(|sr_str| {
                sr_str.split(',')
                    .filter_map(|s| s.parse::<u32>().ok())
                    .collect()
            })
            .unwrap_or(vec![44100, 48000, 96000, 192000]);
            
        let supported_bit_depths = record.get("bd")
            .map(|bd_str| bd_str.split(',').map(|s| s.to_string()).collect())
            .unwrap_or(vec!["S16".to_string(), "S24".to_string(), "F32".to_string()]);
            
        let channels = record.get("ch")
            .and_then(|ch| ch.parse::<u8>().ok())
            .unwrap_or(2);
            
        let core_features = record.get("ft")
            .map(|ft_str| ft_str.split(',').map(|s| s.to_string()).collect())
            .unwrap_or_else(|| vec![
                "pll".to_string(),
                "crc".to_string(),
                "vol".to_string(),
                "gap".to_string(),
                "cap".to_string(),
            ]);
            
        let optional_features = record.get("opt")
            .map(|opt_str| opt_str.split(',').map(|s| s.to_string()).collect())
            .unwrap_or_else(|| vec![
                "dsp".to_string(),
                "conv".to_string(),
            ]);
            
        let control_url = record.get("ctrl").cloned();
        let state = record.get("st").cloned().unwrap_or("idle".to_string());
        let volume = record.get("vol")
            .and_then(|vol| vol.parse::<u8>().ok())
            .unwrap_or(75);
            
        let dac_name = record.get("dac").cloned();
        let hardware_platform = record.get("hw").cloned();

        Ok(NodeDiscoveryRecord {
            uuid,
            version,
            supported_sample_rates,
            supported_bit_depths,
            channels,
            core_features,
            optional_features,
            control_url,
            state,
            volume,
            dac_name,
            hardware_platform,
        })
    }
}

/// mDNS service discovery for AANP nodes
pub struct AnpDiscovery {
    /// Service name (as per specification)
    service_name: String,
    /// Service domain
    service_domain: String,
}

impl AnpDiscovery {
    /// Create a new discovery service
    pub fn new() -> Self {
        Self {
            service_name: "_aaeq-anp._tcp".to_string(),
            service_domain: "local".to_string(),
        }
    }

    /// Get the full service name
    pub fn service_full_name(&self) -> String {
        format!("{}.{}", self.service_name, self.service_domain)
    }

    /// Start discovery (placeholder for actual implementation)
    pub async fn start_discovery(&self) -> Result<(), String> {
        // In a real implementation, this would:
        // 1. Initialize mDNS service discovery
        // 2. Listen for AANP node advertisements
        // 3. Parse TXT records
        // 4. Register discovered nodes
        Ok(())
    }

    /// Stop discovery (placeholder for actual implementation)
    pub async fn stop_discovery(&self) -> Result<(), String> {
        // In a real implementation, this would:
        // 1. Stop mDNS service discovery
        // 2. Clean up resources
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_record_creation() {
        let uuid = Uuid::new_v4();
        let record = NodeDiscoveryRecord::new(uuid);
        
        assert_eq!(record.uuid, uuid);
        assert_eq!(record.version, "0.4.0");
        assert_eq!(record.channels, 2);
        assert_eq!(record.state, "idle");
        assert_eq!(record.volume, 75);
    }

    #[test]
    fn test_txt_record_conversion() {
        let uuid = Uuid::new_v4();
        let mut record = NodeDiscoveryRecord::new(uuid);
        record.control_url = Some("wss://10.0.0.10:7443".to_string());
        record.dac_name = Some("HiFiBerry DAC+".to_string());
        record.hardware_platform = Some("RPi4".to_string());
        
        let txt_record = record.to_txt_record();
        
        assert_eq!(txt_record.get("uuid").unwrap(), &uuid.to_string());
        assert_eq!(txt_record.get("v").unwrap(), &"0.4.0".to_string());
        assert_eq!(txt_record.get("ctrl").unwrap(), &"wss://10.0.0.10:7443".to_string());
        assert_eq!(txt_record.get("dac").unwrap(), &"HiFiBerry DAC+".to_string());
        assert_eq!(txt_record.get("hw").unwrap(), &"RPi4".to_string());
    }
}