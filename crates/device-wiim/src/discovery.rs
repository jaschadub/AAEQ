use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::time::Duration;

/// Discovered WiiM/LinkPlay device information
#[derive(Clone, Debug)]
pub struct DiscoveredDevice {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub service_type: String,
}

/// Discover WiiM/LinkPlay devices on the local network using mDNS
///
/// This function searches for devices advertising the LinkPlay service type
/// and returns a list of discovered devices with their IP addresses.
pub async fn discover_devices(timeout_secs: u64) -> Result<Vec<DiscoveredDevice>> {
    tracing::info!("Starting mDNS discovery for WiiM/LinkPlay devices...");

    // Create mDNS daemon
    let mdns = ServiceDaemon::new()
        .context("Failed to create mDNS service daemon")?;

    // Service types to search for
    // LinkPlay devices typically advertise as _linkplay._tcp.local.
    // Some may also use _http._tcp.local.
    let service_types = vec![
        "_linkplay._tcp.local.",
        "_raop._tcp.local.",  // AirPlay (WiiM supports this)
    ];

    let mut discovered = Vec::new();

    for service_type in service_types {
        tracing::debug!("Browsing for service type: {}", service_type);

        // Browse for the service
        let receiver = mdns.browse(service_type)
            .context(format!("Failed to browse for service: {}", service_type))?;

        // Collect results for the specified timeout
        let timeout = Duration::from_secs(timeout_secs);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            // Try to receive events with a short timeout
            match tokio::time::timeout(
                Duration::from_millis(100),
                tokio::task::spawn_blocking({
                    let receiver = receiver.clone();
                    move || receiver.recv_timeout(Duration::from_millis(100))
                })
            ).await {
                Ok(Ok(Ok(event))) => {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            tracing::info!(
                                "Discovered device: {} at {}:{}",
                                info.get_fullname(),
                                info.get_addresses().iter().next().map(|a| a.to_string()).unwrap_or_else(|| "unknown".to_string()),
                                info.get_port()
                            );

                            // Extract IP address (prefer IPv4)
                            if let Some(addr) = info.get_addresses().iter()
                                .find(|a| a.is_ipv4())
                                .or_else(|| info.get_addresses().iter().next())
                            {
                                let device = DiscoveredDevice {
                                    name: info.get_fullname().to_string(),
                                    host: addr.to_string(),
                                    port: info.get_port(),
                                    service_type: service_type.to_string(),
                                };

                                // Avoid duplicates
                                if !discovered.iter().any(|d: &DiscoveredDevice| d.host == device.host) {
                                    discovered.push(device);
                                }
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, _) => {
                            // Device went offline, ignore for discovery
                        }
                        _ => {}
                    }
                }
                _ => {
                    // Timeout or error, continue
                }
            }
        }
    }

    // Shutdown the mDNS daemon
    mdns.shutdown().ok();

    tracing::info!("Discovery complete. Found {} device(s)", discovered.len());
    Ok(discovered)
}

/// Quick device discovery with 3 second timeout
pub async fn discover_devices_quick() -> Result<Vec<DiscoveredDevice>> {
    discover_devices(3).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_devices() {
        // This test will actually try to discover devices on the network
        // It might not find anything in a test environment
        let result = discover_devices_quick().await;

        match result {
            Ok(devices) => {
                println!("Discovered {} devices", devices.len());
                for device in devices {
                    println!("  - {} at {}", device.name, device.host);
                }
            }
            Err(e) => {
                println!("Discovery failed (this is okay in test environment): {}", e);
            }
        }
    }
}
