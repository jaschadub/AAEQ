/// UPnP/SSDP device discovery for DLNA renderers
///
/// This module discovers UPnP MediaRenderer devices on the network using SSDP (Simple Service Discovery Protocol).

use anyhow::Result;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Represents a discovered DLNA/UPnP MediaRenderer device
#[derive(Debug, Clone)]
pub struct DlnaDevice {
    pub name: String,
    pub location: String, // URL to device description XML
    pub uuid: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub ip: Option<IpAddr>,
    pub services: Vec<DlnaService>,
}

/// UPnP service on a DLNA device
#[derive(Debug, Clone)]
pub struct DlnaService {
    pub service_type: String,
    pub service_id: String,
    pub control_url: String,
    pub event_sub_url: String,
    pub scpd_url: String,
}

const SSDP_ADDR: &str = "239.255.255.250:1900";
const SSDP_MX: u8 = 3; // Max wait time in seconds

/// Discover DLNA MediaRenderer devices on the network
///
/// # Arguments
/// * `timeout_secs` - How long to search for devices
///
/// # Returns
/// A vector of discovered DLNA devices
pub async fn discover_devices(timeout_secs: u64) -> Result<Vec<DlnaDevice>> {
    info!("Starting DLNA device discovery ({}s timeout)", timeout_secs);

    let socket = create_ssdp_socket()?;
    send_msearch(&socket)?;

    let mut devices: HashMap<String, DlnaDevice> = HashMap::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

    // Non-blocking socket for async operation
    socket.set_nonblocking(true)?;

    while tokio::time::Instant::now() < deadline {
        let mut buf = [0u8; 2048];

        match timeout(Duration::from_millis(100), async {
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((len, addr)) => {
                        return Ok::<(usize, SocketAddr), std::io::Error>((len, addr));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
        })
        .await
        {
            Ok(Ok((len, addr))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                debug!("Received SSDP response from {}: {}", addr, response);

                if let Some(device) = parse_ssdp_response(&response, addr.ip()).await {
                    if !devices.contains_key(&device.uuid) {
                        info!("Discovered DLNA device: {} ({})", device.name, device.uuid);
                        devices.insert(device.uuid.clone(), device);
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Socket error during discovery: {}", e);
            }
            Err(_) => {
                // Timeout, continue searching
            }
        }
    }

    info!("DLNA discovery complete, found {} device(s)", devices.len());
    Ok(devices.into_values().collect())
}

/// Find a specific DLNA device by name
pub async fn find_device_by_name(
    name: &str,
    timeout_secs: u64,
) -> Result<Option<DlnaDevice>> {
    let devices = discover_devices(timeout_secs).await?;
    let name_lower = name.to_lowercase();

    Ok(devices
        .into_iter()
        .find(|d| d.name.to_lowercase().contains(&name_lower)))
}

/// Create a UDP socket for SSDP multicast
fn create_ssdp_socket() -> Result<UdpSocket> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    Ok(socket)
}

/// Send M-SEARCH request to discover MediaRenderer devices
fn send_msearch(socket: &UdpSocket) -> Result<()> {
    let msearch = format!(
        "M-SEARCH * HTTP/1.1\r\n\
         HOST: {}\r\n\
         MAN: \"ssdp:discover\"\r\n\
         MX: {}\r\n\
         ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\
         \r\n",
        SSDP_ADDR, SSDP_MX
    );

    socket.send_to(msearch.as_bytes(), SSDP_ADDR)?;
    debug!("Sent M-SEARCH for MediaRenderer devices");
    Ok(())
}

/// Parse SSDP response and fetch device description
async fn parse_ssdp_response(response: &str, source_ip: IpAddr) -> Option<DlnaDevice> {
    // Look for LOCATION header
    let location = response
        .lines()
        .find(|line| line.to_lowercase().starts_with("location:"))?
        .split(':')
        .skip(1)
        .collect::<Vec<_>>()
        .join(":")
        .trim()
        .to_string();

    debug!("Fetching device description from: {}", location);

    // Fetch and parse device description XML
    match fetch_device_description(&location).await {
        Ok(device) => Some(DlnaDevice {
            ip: Some(source_ip),
            location,
            ..device
        }),
        Err(e) => {
            warn!("Failed to fetch device description: {}", e);
            None
        }
    }
}

/// Fetch and parse UPnP device description XML
async fn fetch_device_description(location: &str) -> Result<DlnaDevice> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response = client.get(location).send().await?;
    let xml_text = response.text().await?;

    // Try using the improved XML parser first
    match super::xml_parser::parse_device_xml_proper(&xml_text, location) {
        Ok(device) => Ok(device),
        Err(e) => {
            // Fall back to simple parser if quick-xml fails
            warn!("quick-xml parser failed, falling back to simple parser: {}", e);
            parse_device_xml(&xml_text, location)
        }
    }
}

/// Parse UPnP device description XML
fn parse_device_xml(xml: &str, location: &str) -> Result<DlnaDevice> {
    // Simple XML parsing - for production, use a proper XML parser like quick-xml
    // For now, we'll do basic string searching

    let name = extract_xml_value(xml, "friendlyName")
        .unwrap_or_else(|| "Unknown Device".to_string());

    let uuid = extract_xml_value(xml, "UDN")
        .and_then(|udn| udn.strip_prefix("uuid:").map(|s| s.to_string()))
        .unwrap_or_else(|| format!("unknown-{}", location));

    let manufacturer = extract_xml_value(xml, "manufacturer");
    let model = extract_xml_value(xml, "modelName");

    // Extract services (AVTransport, RenderingControl, ConnectionManager)
    let services = extract_services(xml, location);

    Ok(DlnaDevice {
        name,
        location: location.to_string(),
        uuid,
        manufacturer,
        model,
        ip: None, // Will be set by caller
        services,
    })
}

/// Extract text content from an XML tag
fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)?;

    Some(xml[start..start + end].trim().to_string())
}

/// Extract service information from device XML
fn extract_services(xml: &str, base_url: &str) -> Vec<DlnaService> {
    let mut services = Vec::new();

    // Look for AVTransport service (most important for MediaRenderer)
    if let Some(avtransport) = extract_service(
        xml,
        "urn:schemas-upnp-org:service:AVTransport",
        base_url,
    ) {
        services.push(avtransport);
    }

    // Look for RenderingControl service
    if let Some(rendering) = extract_service(
        xml,
        "urn:schemas-upnp-org:service:RenderingControl",
        base_url,
    ) {
        services.push(rendering);
    }

    // Look for ConnectionManager service
    if let Some(connection) = extract_service(
        xml,
        "urn:schemas-upnp-org:service:ConnectionManager",
        base_url,
    ) {
        services.push(connection);
    }

    services
}

/// Extract a single service from XML
fn extract_service(xml: &str, service_type_prefix: &str, base_url: &str) -> Option<DlnaService> {
    // Find service block containing this service type
    let service_start = xml.find("<service>")?;
    let service_section = &xml[service_start..];

    // Check if this section contains our service type
    if !service_section.contains(service_type_prefix) {
        // Try to find next service block (simple recursive-like search)
        let next_service = service_section.find("</service>")? + "</service>".len();
        return extract_service(&service_section[next_service..], service_type_prefix, base_url);
    }

    let service_type = extract_xml_value(service_section, "serviceType")?;
    if !service_type.starts_with(service_type_prefix) {
        return None;
    }

    let service_id = extract_xml_value(service_section, "serviceId")?;
    let control_url = extract_xml_value(service_section, "controlURL")?;
    let event_sub_url = extract_xml_value(service_section, "eventSubURL")?;
    let scpd_url = extract_xml_value(service_section, "SCPDURL")?;

    // Resolve relative URLs
    let base = base_url.trim_end_matches('/');
    let control_url = resolve_url(base, &control_url);
    let event_sub_url = resolve_url(base, &event_sub_url);
    let scpd_url = resolve_url(base, &scpd_url);

    Some(DlnaService {
        service_type,
        service_id,
        control_url,
        event_sub_url,
        scpd_url,
    })
}

/// Resolve a potentially relative URL against a base URL
fn resolve_url(base: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if url.starts_with('/') {
        // Extract base URL (protocol + host + port)
        if let Some(pos) = base.find("://") {
            let after_protocol = &base[pos + 3..];
            if let Some(slash_pos) = after_protocol.find('/') {
                format!("{}{}", &base[..pos + 3 + slash_pos], url)
            } else {
                format!("{}{}", base, url)
            }
        } else {
            format!("{}{}", base, url)
        }
    } else {
        // Relative path
        if let Some(last_slash) = base.rfind('/') {
            format!("{}/{}", &base[..last_slash], url)
        } else {
            format!("{}/{}", base, url)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_value() {
        let xml = "<root><friendlyName>My Device</friendlyName></root>";
        assert_eq!(
            extract_xml_value(xml, "friendlyName"),
            Some("My Device".to_string())
        );
    }

    #[test]
    fn test_resolve_url() {
        let base = "http://192.168.1.100:8080/device.xml";

        assert_eq!(
            resolve_url(base, "/control"),
            "http://192.168.1.100:8080/control"
        );

        assert_eq!(
            resolve_url(base, "service.xml"),
            "http://192.168.1.100:8080/service.xml"
        );

        assert_eq!(
            resolve_url(base, "http://other.com/path"),
            "http://other.com/path"
        );
    }

    #[test]
    fn test_parse_device_xml() {
        let xml = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <device>
    <friendlyName>Test Renderer</friendlyName>
    <UDN>uuid:12345678-1234-1234-1234-123456789012</UDN>
    <manufacturer>ACME Corp</manufacturer>
    <modelName>Renderer v1</modelName>
  </device>
</root>"#;

        let device = parse_device_xml(xml, "http://192.168.1.100/device.xml").unwrap();
        assert_eq!(device.name, "Test Renderer");
        assert_eq!(device.uuid, "12345678-1234-1234-1234-123456789012");
        assert_eq!(device.manufacturer, Some("ACME Corp".to_string()));
        assert_eq!(device.model, Some("Renderer v1".to_string()));
    }
}
