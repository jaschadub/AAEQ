/// UPnP Device Description XML generation for SSDP service announcement
///
/// This module generates device description XML that allows AAEQ to be discovered
/// as a UPnP MediaServer/MediaRenderer on the network via SSDP.
use anyhow::Result;
use std::net::IpAddr;
use uuid::Uuid;

/// Generate a persistent device UUID based on the hostname
///
/// This ensures the same device UUID is used across restarts
pub fn generate_device_uuid() -> Result<String> {
    // Use hostname to generate a deterministic UUID
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string();

    // Create a namespace UUID for AAEQ devices
    let namespace = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8")?; // DNS namespace

    // Generate UUID v5 (deterministic based on hostname)
    let uuid = Uuid::new_v5(&namespace, hostname.as_bytes());

    Ok(format!("uuid:{}", uuid))
}

/// Get the local IP address for the device
///
/// This tries to find a non-loopback IPv4 address
fn get_local_ip() -> Result<IpAddr> {
    // Try to get the local IP by connecting to a public DNS server
    // (doesn't actually send packets, just uses the OS routing table)
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    let addr = socket.local_addr()?;
    Ok(addr.ip())
}

/// Generate the root device description XML
///
/// This creates a UPnP device description that advertises AAEQ as both
/// a MediaServer and MediaRenderer, making it discoverable on the network.
pub fn generate_device_description(
    device_uuid: &str,
    friendly_name: &str,
    port: u16,
) -> Result<String> {
    let local_ip = get_local_ip()?;
    let base_url = format!("http://{}:{}", local_ip, port);

    let xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <specVersion>
    <major>1</major>
    <minor>0</minor>
  </specVersion>
  <device>
    <deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>
    <friendlyName>{friendly_name}</friendlyName>
    <manufacturer>AAEQ</manufacturer>
    <manufacturerURL>https://github.com/jaschadub/AAEQ</manufacturerURL>
    <modelDescription>Adaptive Audio Equalizer with DSP Streaming</modelDescription>
    <modelName>AAEQ</modelName>
    <modelNumber>1.0</modelNumber>
    <modelURL>https://github.com/jaschadub/AAEQ</modelURL>
    <serialNumber>1</serialNumber>
    <UDN>{device_uuid}</UDN>
    <presentationURL>{base_url}</presentationURL>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:ContentDirectory</serviceId>
        <SCPDURL>/upnp/ContentDirectory.xml</SCPDURL>
        <controlURL>/upnp/control/ContentDirectory</controlURL>
        <eventSubURL>/upnp/event/ContentDirectory</eventSubURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:ConnectionManager</serviceId>
        <SCPDURL>/upnp/ConnectionManager.xml</SCPDURL>
        <controlURL>/upnp/control/ConnectionManager</controlURL>
        <eventSubURL>/upnp/event/ConnectionManager</eventSubURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:AVTransport</serviceId>
        <SCPDURL>/upnp/AVTransport.xml</SCPDURL>
        <controlURL>/upnp/control/AVTransport</controlURL>
        <eventSubURL>/upnp/event/AVTransport</eventSubURL>
      </service>
    </serviceList>
  </device>
</root>"#,
        friendly_name = friendly_name,
        device_uuid = device_uuid,
        base_url = base_url,
    );

    Ok(xml)
}

/// Generate ContentDirectory service description XML
pub fn generate_content_directory_scpd() -> String {
    r#"<?xml version="1.0" encoding="utf-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
  <specVersion>
    <major>1</major>
    <minor>0</minor>
  </specVersion>
  <actionList>
    <action>
      <name>Browse</name>
      <argumentList>
        <argument>
          <name>ObjectID</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_ObjectID</relatedStateVariable>
        </argument>
        <argument>
          <name>BrowseFlag</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_BrowseFlag</relatedStateVariable>
        </argument>
        <argument>
          <name>Filter</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_Filter</relatedStateVariable>
        </argument>
        <argument>
          <name>StartingIndex</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_Index</relatedStateVariable>
        </argument>
        <argument>
          <name>RequestedCount</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable>
        </argument>
        <argument>
          <name>SortCriteria</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_SortCriteria</relatedStateVariable>
        </argument>
        <argument>
          <name>Result</name>
          <direction>out</direction>
          <relatedStateVariable>A_ARG_TYPE_Result</relatedStateVariable>
        </argument>
        <argument>
          <name>NumberReturned</name>
          <direction>out</direction>
          <relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable>
        </argument>
        <argument>
          <name>TotalMatches</name>
          <direction>out</direction>
          <relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable>
        </argument>
        <argument>
          <name>UpdateID</name>
          <direction>out</direction>
          <relatedStateVariable>A_ARG_TYPE_UpdateID</relatedStateVariable>
        </argument>
      </argumentList>
    </action>
  </actionList>
  <serviceStateTable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_ObjectID</name>
      <dataType>string</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_Result</name>
      <dataType>string</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_BrowseFlag</name>
      <dataType>string</dataType>
      <allowedValueList>
        <allowedValue>BrowseMetadata</allowedValue>
        <allowedValue>BrowseDirectChildren</allowedValue>
      </allowedValueList>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_Filter</name>
      <dataType>string</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_SortCriteria</name>
      <dataType>string</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_Index</name>
      <dataType>ui4</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_Count</name>
      <dataType>ui4</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_UpdateID</name>
      <dataType>ui4</dataType>
    </stateVariable>
  </serviceStateTable>
</scpd>"#.to_string()
}

/// Generate ConnectionManager service description XML
pub fn generate_connection_manager_scpd() -> String {
    r#"<?xml version="1.0" encoding="utf-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
  <specVersion>
    <major>1</major>
    <minor>0</minor>
  </specVersion>
  <actionList>
    <action>
      <name>GetProtocolInfo</name>
      <argumentList>
        <argument>
          <name>Source</name>
          <direction>out</direction>
          <relatedStateVariable>SourceProtocolInfo</relatedStateVariable>
        </argument>
        <argument>
          <name>Sink</name>
          <direction>out</direction>
          <relatedStateVariable>SinkProtocolInfo</relatedStateVariable>
        </argument>
      </argumentList>
    </action>
  </actionList>
  <serviceStateTable>
    <stateVariable sendEvents="yes">
      <name>SourceProtocolInfo</name>
      <dataType>string</dataType>
    </stateVariable>
    <stateVariable sendEvents="yes">
      <name>SinkProtocolInfo</name>
      <dataType>string</dataType>
    </stateVariable>
  </serviceStateTable>
</scpd>"#.to_string()
}

/// Generate AVTransport service description XML
pub fn generate_av_transport_scpd() -> String {
    r#"<?xml version="1.0" encoding="utf-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
  <specVersion>
    <major>1</major>
    <minor>0</minor>
  </specVersion>
  <actionList>
    <action>
      <name>SetAVTransportURI</name>
      <argumentList>
        <argument>
          <name>InstanceID</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_InstanceID</relatedStateVariable>
        </argument>
        <argument>
          <name>CurrentURI</name>
          <direction>in</direction>
          <relatedStateVariable>AVTransportURI</relatedStateVariable>
        </argument>
        <argument>
          <name>CurrentURIMetaData</name>
          <direction>in</direction>
          <relatedStateVariable>AVTransportURIMetaData</relatedStateVariable>
        </argument>
      </argumentList>
    </action>
    <action>
      <name>Play</name>
      <argumentList>
        <argument>
          <name>InstanceID</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_InstanceID</relatedStateVariable>
        </argument>
        <argument>
          <name>Speed</name>
          <direction>in</direction>
          <relatedStateVariable>TransportPlaySpeed</relatedStateVariable>
        </argument>
      </argumentList>
    </action>
    <action>
      <name>Stop</name>
      <argumentList>
        <argument>
          <name>InstanceID</name>
          <direction>in</direction>
          <relatedStateVariable>A_ARG_TYPE_InstanceID</relatedStateVariable>
        </argument>
      </argumentList>
    </action>
  </actionList>
  <serviceStateTable>
    <stateVariable sendEvents="no">
      <name>A_ARG_TYPE_InstanceID</name>
      <dataType>ui4</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>AVTransportURI</name>
      <dataType>string</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>AVTransportURIMetaData</name>
      <dataType>string</dataType>
    </stateVariable>
    <stateVariable sendEvents="no">
      <name>TransportPlaySpeed</name>
      <dataType>string</dataType>
      <allowedValueList>
        <allowedValue>1</allowedValue>
      </allowedValueList>
    </stateVariable>
  </serviceStateTable>
</scpd>"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_device_uuid() {
        let uuid = generate_device_uuid().unwrap();
        assert!(uuid.starts_with("uuid:"));

        // UUID should be deterministic based on hostname
        let uuid2 = generate_device_uuid().unwrap();
        assert_eq!(uuid, uuid2);
    }

    #[test]
    fn test_generate_device_description() {
        let uuid = "uuid:12345678-1234-1234-1234-123456789012";
        let xml = generate_device_description(uuid, "Test AAEQ", 8090).unwrap();

        assert!(xml.contains(uuid));
        assert!(xml.contains("Test AAEQ"));
        assert!(xml.contains("MediaServer"));
        assert!(xml.contains("ContentDirectory"));
        assert!(xml.contains("ConnectionManager"));
        assert!(xml.contains("AVTransport"));
    }

    #[test]
    fn test_generate_service_descriptions() {
        let content_dir = generate_content_directory_scpd();
        assert!(content_dir.contains("ContentDirectory"));
        assert!(content_dir.contains("Browse"));

        let conn_mgr = generate_connection_manager_scpd();
        assert!(conn_mgr.contains("ConnectionManager"));
        assert!(conn_mgr.contains("GetProtocolInfo"));

        let av_transport = generate_av_transport_scpd();
        assert!(av_transport.contains("AVTransport"));
        assert!(av_transport.contains("Play"));
    }
}
