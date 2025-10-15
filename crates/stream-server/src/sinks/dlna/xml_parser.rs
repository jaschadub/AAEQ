/// Proper XML parsing for UPnP device descriptions using quick-xml

use anyhow::{anyhow, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use super::discovery::{DlnaDevice, DlnaService};

/// Parse UPnP device description XML using quick-xml
pub fn parse_device_xml_proper(xml: &str, location: &str) -> Result<DlnaDevice> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut device_info = DeviceInfo::default();
    let mut services = Vec::new();
    let mut current_path = Vec::new();
    let mut current_text = String::new();
    let mut in_device = false;
    let mut in_service = false;
    let mut current_service = ServiceInfo::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_path.push(name.clone());

                if name == "device" && !in_device {
                    in_device = true;
                }
                if name == "service" {
                    in_service = true;
                    current_service = ServiceInfo::default();
                }

                current_text.clear();
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                // Process text content for device fields
                if in_device && !in_service {
                    match name.as_str() {
                        "friendlyName" => device_info.friendly_name = current_text.trim().to_string(),
                        "UDN" => device_info.udn = current_text.trim().to_string(),
                        "manufacturer" => device_info.manufacturer = Some(current_text.trim().to_string()),
                        "modelName" => device_info.model = Some(current_text.trim().to_string()),
                        _ => {}
                    }
                }

                // Process text content for service fields
                if in_service {
                    match name.as_str() {
                        "serviceType" => current_service.service_type = current_text.trim().to_string(),
                        "serviceId" => current_service.service_id = current_text.trim().to_string(),
                        "controlURL" => current_service.control_url = current_text.trim().to_string(),
                        "eventSubURL" => current_service.event_sub_url = current_text.trim().to_string(),
                        "SCPDURL" => current_service.scpd_url = current_text.trim().to_string(),
                        "service" => {
                            // End of service element
                            if !current_service.service_type.is_empty() {
                                services.push(current_service.clone());
                            }
                            in_service = false;
                        }
                        _ => {}
                    }
                }

                current_path.pop();
                current_text.clear();
            }
            Ok(Event::Text(e)) => {
                current_text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow!("XML parsing error at position {}: {}", reader.buffer_position(), e));
            }
            _ => {}
        }
    }

    // Extract UUID from UDN
    let uuid = device_info.udn
        .strip_prefix("uuid:")
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("unknown-{}", location));

    // Resolve relative URLs for services
    let resolved_services = services.into_iter()
        .map(|s| resolve_service_urls(s, location))
        .collect();

    Ok(DlnaDevice {
        name: device_info.friendly_name,
        location: location.to_string(),
        uuid,
        manufacturer: device_info.manufacturer,
        model: device_info.model,
        ip: None, // Will be set by caller
        services: resolved_services,
    })
}

#[derive(Debug, Clone, Default)]
struct DeviceInfo {
    friendly_name: String,
    udn: String,
    manufacturer: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ServiceInfo {
    service_type: String,
    service_id: String,
    control_url: String,
    event_sub_url: String,
    scpd_url: String,
}

/// Convert ServiceInfo to DlnaService
fn resolve_service_urls(service: ServiceInfo, base_url: &str) -> DlnaService {
    let base = base_url.trim_end_matches('/');

    DlnaService {
        service_type: service.service_type,
        service_id: service.service_id,
        control_url: resolve_url(base, &service.control_url),
        event_sub_url: resolve_url(base, &service.event_sub_url),
        scpd_url: resolve_url(base, &service.scpd_url),
    }
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
    fn test_parse_device_xml_proper() {
        let xml = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <device>
    <friendlyName>Test Renderer</friendlyName>
    <UDN>uuid:12345678-1234-1234-1234-123456789012</UDN>
    <manufacturer>ACME Corp</manufacturer>
    <modelName>Renderer v1</modelName>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:AVTransport</serviceId>
        <controlURL>/upnp/control/avtransport1</controlURL>
        <eventSubURL>/upnp/event/avtransport1</eventSubURL>
        <SCPDURL>/avtransportSCPD.xml</SCPDURL>
      </service>
    </serviceList>
  </device>
</root>"#;

        let device = parse_device_xml_proper(xml, "http://192.168.1.100/device.xml").unwrap();
        assert_eq!(device.name, "Test Renderer");
        assert_eq!(device.uuid, "12345678-1234-1234-1234-123456789012");
        assert_eq!(device.manufacturer, Some("ACME Corp".to_string()));
        assert_eq!(device.model, Some("Renderer v1".to_string()));
        assert_eq!(device.services.len(), 1);

        let service = &device.services[0];
        assert_eq!(service.service_type, "urn:schemas-upnp-org:service:AVTransport:1");
        assert_eq!(service.control_url, "http://192.168.1.100/upnp/control/avtransport1");
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
}
