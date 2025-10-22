/// Device-specific profiles and quirks for DLNA/UPnP renderers
///
/// Different DLNA devices have different quirks, limitations, and preferred settings.
/// This module provides device profiles to handle these differences gracefully.
use crate::types::{OutputConfig, SampleFormat};
use super::discovery::DlnaDevice;

/// Device profile with quirks and optimal settings
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    pub name: String,
    pub manufacturer: Option<String>,
    pub quirks: DeviceQuirks,
    pub optimal_config: OptimalConfig,
}

/// Device-specific quirks and limitations
#[derive(Debug, Clone, Default)]
pub struct DeviceQuirks {
    /// Device requires specific HTTP headers
    pub requires_custom_headers: bool,
    pub custom_headers: Vec<(String, String)>,

    /// Device doesn't support chunked transfer encoding
    pub no_chunked_transfer: bool,

    /// Device requires specific DIDL-Lite metadata fields
    pub requires_extended_metadata: bool,

    /// Device needs additional delay between commands
    pub command_delay_ms: u64,

    /// Device doesn't fully implement AVTransport
    pub limited_avtransport: bool,

    /// Specific sample rates that cause issues
    pub problematic_sample_rates: Vec<u32>,

    /// Requires authentication
    pub requires_auth: bool,

    /// Sonos-specific: needs specific group coordination
    pub is_sonos: bool,

    /// WiiM-specific optimizations
    pub is_wiim: bool,

    /// Prefers WAV over other formats
    pub prefers_wav: bool,
}

/// Optimal configuration for a device
#[derive(Debug, Clone)]
pub struct OptimalConfig {
    /// Preferred sample rate
    pub sample_rate: u32,

    /// Preferred format
    pub format: SampleFormat,

    /// Preferred buffer size
    pub buffer_ms: u32,

    /// Recommended channels
    pub channels: u16,
}

impl Default for OptimalConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            format: SampleFormat::S24LE,
            buffer_ms: 200,
            channels: 2,
        }
    }
}

impl DeviceProfile {
    /// Detect device profile from DlnaDevice information
    pub fn from_device(device: &DlnaDevice) -> Self {
        let name = device.name.to_lowercase();
        let manufacturer = device.manufacturer.as_ref().map(|m| m.to_lowercase());
        let model = device.model.as_ref().map(|m| m.to_lowercase());

        // Check for known manufacturers
        if let Some(ref mfr) = manufacturer {
            if mfr.contains("wiim") {
                return Self::wiim_profile(device);
            } else if mfr.contains("bluesound") {
                return Self::bluesound_profile(device);
            } else if mfr.contains("sonos") {
                return Self::sonos_profile(device);
            } else if mfr.contains("denon") || mfr.contains("heos") {
                return Self::heos_profile(device);
            }
        }

        // Check model name
        if let Some(ref mdl) = model {
            if mdl.contains("wiim") {
                return Self::wiim_profile(device);
            }
        }

        // Check device name
        if name.contains("wiim") {
            return Self::wiim_profile(device);
        } else if name.contains("sonos") {
            return Self::sonos_profile(device);
        } else if name.contains("bluesound") {
            return Self::bluesound_profile(device);
        }

        // Default generic profile
        Self::generic_profile(device)
    }

    /// WiiM device profile (Pro, Ultra, Mini, etc.)
    fn wiim_profile(device: &DlnaDevice) -> Self {
        Self {
            name: device.name.clone(),
            manufacturer: device.manufacturer.clone(),
            quirks: DeviceQuirks {
                is_wiim: true,
                prefers_wav: true,
                ..Default::default()
            },
            optimal_config: OptimalConfig {
                sample_rate: 48000,
                format: SampleFormat::S24LE,
                buffer_ms: 150,
                channels: 2,
            },
        }
    }

    /// Bluesound device profile
    fn bluesound_profile(device: &DlnaDevice) -> Self {
        Self {
            name: device.name.clone(),
            manufacturer: device.manufacturer.clone(),
            quirks: DeviceQuirks {
                prefers_wav: true,
                ..Default::default()
            },
            optimal_config: OptimalConfig {
                sample_rate: 96000, // Bluesound supports high-res
                format: SampleFormat::S24LE,
                buffer_ms: 200,
                channels: 2,
            },
        }
    }

    /// Sonos device profile
    fn sonos_profile(device: &DlnaDevice) -> Self {
        Self {
            name: device.name.clone(),
            manufacturer: device.manufacturer.clone(),
            quirks: DeviceQuirks {
                is_sonos: true,
                requires_custom_headers: true,
                custom_headers: vec![
                    ("X-Sonos-Codec".to_string(), "wav".to_string()),
                ],
                no_chunked_transfer: true,
                ..Default::default()
            },
            optimal_config: OptimalConfig {
                sample_rate: 48000,
                format: SampleFormat::S16LE, // Sonos prefers 16-bit
                buffer_ms: 250,
                channels: 2,
            },
        }
    }

    /// Denon HEOS device profile
    fn heos_profile(device: &DlnaDevice) -> Self {
        Self {
            name: device.name.clone(),
            manufacturer: device.manufacturer.clone(),
            quirks: DeviceQuirks {
                prefers_wav: true,
                ..Default::default()
            },
            optimal_config: OptimalConfig {
                sample_rate: 48000,
                format: SampleFormat::S24LE,
                buffer_ms: 200,
                channels: 2,
            },
        }
    }

    /// Generic device profile (fallback)
    fn generic_profile(device: &DlnaDevice) -> Self {
        Self {
            name: device.name.clone(),
            manufacturer: device.manufacturer.clone(),
            quirks: DeviceQuirks::default(),
            optimal_config: OptimalConfig::default(),
        }
    }

    /// Apply profile-specific adjustments to a configuration
    pub fn adjust_config(&self, mut config: OutputConfig) -> OutputConfig {
        // Adjust sample rate if current one is problematic
        if self.quirks.problematic_sample_rates.contains(&config.sample_rate) {
            config.sample_rate = self.optimal_config.sample_rate;
        }

        // For Sonos, force 16-bit
        if self.quirks.is_sonos && config.format != SampleFormat::S16LE {
            config.format = SampleFormat::S16LE;
        }

        // Ensure buffer size is adequate
        if config.buffer_ms < self.optimal_config.buffer_ms {
            config.buffer_ms = self.optimal_config.buffer_ms;
        }

        config
    }

    /// Get recommended configuration for this device
    pub fn recommended_config(&self) -> OutputConfig {
        OutputConfig {
            sample_rate: self.optimal_config.sample_rate,
            channels: self.optimal_config.channels,
            format: self.optimal_config.format,
            buffer_ms: self.optimal_config.buffer_ms,
            exclusive: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiim_profile_detection() {
        let device = DlnaDevice {
            name: "WiiM Pro".to_string(),
            manufacturer: Some("WiiM".to_string()),
            model: Some("Pro".to_string()),
            uuid: "test".to_string(),
            location: "http://test".to_string(),
            ip: None,
            services: vec![],
        };

        let profile = DeviceProfile::from_device(&device);
        assert!(profile.quirks.is_wiim);
        assert!(profile.quirks.prefers_wav);
        assert_eq!(profile.optimal_config.sample_rate, 48000);
        assert_eq!(profile.optimal_config.format, SampleFormat::S24LE);
    }

    #[test]
    fn test_sonos_profile_detection() {
        let device = DlnaDevice {
            name: "Sonos One".to_string(),
            manufacturer: Some("Sonos".to_string()),
            model: Some("One".to_string()),
            uuid: "test".to_string(),
            location: "http://test".to_string(),
            ip: None,
            services: vec![],
        };

        let profile = DeviceProfile::from_device(&device);
        assert!(profile.quirks.is_sonos);
        assert!(profile.quirks.no_chunked_transfer);
        assert_eq!(profile.optimal_config.format, SampleFormat::S16LE);
    }

    #[test]
    fn test_config_adjustment() {
        let device = DlnaDevice {
            name: "Sonos One".to_string(),
            manufacturer: Some("Sonos".to_string()),
            model: None,
            uuid: "test".to_string(),
            location: "http://test".to_string(),
            ip: None,
            services: vec![],
        };

        let profile = DeviceProfile::from_device(&device);
        let config = OutputConfig {
            sample_rate: 48000,
            channels: 2,
            format: SampleFormat::S24LE, // Will be adjusted to S16LE
            buffer_ms: 100,              // Will be adjusted to 250
            exclusive: false,
        };

        let adjusted = profile.adjust_config(config);
        assert_eq!(adjusted.format, SampleFormat::S16LE);
        assert_eq!(adjusted.buffer_ms, 250);
    }

    #[test]
    fn test_generic_profile() {
        let device = DlnaDevice {
            name: "Unknown Device".to_string(),
            manufacturer: Some("Unknown".to_string()),
            model: None,
            uuid: "test".to_string(),
            location: "http://test".to_string(),
            ip: None,
            services: vec![],
        };

        let profile = DeviceProfile::from_device(&device);
        assert!(!profile.quirks.is_wiim);
        assert!(!profile.quirks.is_sonos);
        assert_eq!(profile.optimal_config.sample_rate, 48000);
    }
}
