use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::time::Duration;
use tracing::{debug, info};

/// Information about a discovered AirPlay device
#[derive(Debug, Clone)]
pub struct AirPlayDevice {
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub addresses: Vec<std::net::IpAddr>,
    pub model: Option<String>,
    pub features: Option<String>,
}

/// Discover AirPlay devices on the local network using mDNS
pub async fn discover_devices(timeout_secs: u64) -> Result<Vec<AirPlayDevice>> {
    info!("Starting AirPlay device discovery...");

    let mdns = ServiceDaemon::new()?;

    // Browse for _raop._tcp services (AirPlay audio)
    let receiver = mdns.browse("_raop._tcp.local.")?;

    let mut devices = Vec::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(timeout_secs) {
        match tokio::time::timeout(
            Duration::from_millis(100),
            tokio::task::spawn_blocking({
                let receiver = receiver.clone();
                move || receiver.recv_timeout(Duration::from_millis(100))
            }),
        )
        .await
        {
            Ok(Ok(Ok(event))) => {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        debug!("Discovered AirPlay device: {}", info.get_fullname());

                        let name = info
                            .get_properties()
                            .get("fn")
                            .map(|v| v.val_str().to_string())
                            .unwrap_or_else(|| info.get_hostname().trim_end_matches('.').to_string());

                        let model = info
                            .get_properties()
                            .get("am")
                            .map(|v| v.val_str().to_string());

                        let features = info
                            .get_properties()
                            .get("ft")
                            .map(|v| v.val_str().to_string());

                        let device = AirPlayDevice {
                            name: name.clone(),
                            hostname: info.get_hostname().to_string(),
                            port: info.get_port(),
                            addresses: info.get_addresses().iter().copied().collect(),
                            model,
                            features,
                        };

                        info!(
                            "Found AirPlay device: {} at {}:{}",
                            device.name, device.hostname, device.port
                        );

                        devices.push(device);
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        debug!("AirPlay device removed: {}", fullname);
                    }
                    ServiceEvent::SearchStarted(_) => {
                        debug!("mDNS search started");
                    }
                    ServiceEvent::SearchStopped(_) => {
                        debug!("mDNS search stopped");
                    }
                    ServiceEvent::ServiceFound(_, _) => {
                        debug!("Service found (will resolve)");
                    }
                }
            }
            Ok(Ok(Err(_))) | Ok(Err(_)) | Err(_) => {
                // Timeout or task error, continue
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }

    // Shutdown may log a harmless "sending on a closed channel" error
    // This is a known mdns_sd library issue and can be safely ignored
    if let Err(e) = mdns.shutdown() {
        debug!("mDNS shutdown error (harmless): {}", e);
    }

    info!("Discovery complete. Found {} device(s)", devices.len());
    Ok(devices)
}

/// Find a specific AirPlay device by name
pub async fn find_device_by_name(name: &str, timeout_secs: u64) -> Result<Option<AirPlayDevice>> {
    let devices = discover_devices(timeout_secs).await?;

    Ok(devices.into_iter().find(|d| {
        d.name.to_lowercase().contains(&name.to_lowercase())
            || d.hostname.to_lowercase().contains(&name.to_lowercase())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network and actual AirPlay devices
    async fn test_discover_devices() {
        let devices = discover_devices(5).await.unwrap();
        println!("Found {} devices", devices.len());
        for device in devices {
            println!("  - {} ({}:{})", device.name, device.hostname, device.port);
        }
    }
}
