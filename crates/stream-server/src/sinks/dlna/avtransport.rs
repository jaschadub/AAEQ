/// AVTransport SOAP control for UPnP MediaRenderers
///
/// This module implements the AVTransport service control, allowing AAEQ to:
/// - Set the stream URL on a renderer (SetAVTransportURI)
/// - Start/stop playback (Play, Stop)
/// - Query playback state (GetTransportInfo)

use anyhow::{anyhow, Result};
use std::time::Duration;
use tracing::{debug, info};

/// AVTransport controller for a UPnP MediaRenderer
pub struct AVTransport {
    control_url: String,
    service_type: String,
}

impl AVTransport {
    /// Create a new AVTransport controller
    pub fn new(control_url: String, service_type: String) -> Self {
        Self {
            control_url,
            service_type,
        }
    }

    /// Set the URI for playback (tell renderer to pull from AAEQ)
    ///
    /// # Arguments
    /// * `uri` - The URL to the audio stream (e.g., "http://192.168.1.100:8090/stream.wav")
    /// * `metadata` - DIDL-Lite XML metadata (optional)
    pub async fn set_av_transport_uri(&self, uri: &str, metadata: Option<&str>) -> Result<()> {
        info!("Setting AVTransport URI: {}", uri);

        let metadata_xml = metadata.unwrap_or("");
        let escaped_metadata = escape_xml(metadata_xml);

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:SetAVTransportURI xmlns:u="{}">
      <InstanceID>0</InstanceID>
      <CurrentURI>{}</CurrentURI>
      <CurrentURIMetaData>{}</CurrentURIMetaData>
    </u:SetAVTransportURI>
  </s:Body>
</s:Envelope>"#,
            self.service_type, uri, escaped_metadata
        );

        self.send_soap_action("SetAVTransportURI", &body).await?;

        info!("AVTransport URI set successfully");
        Ok(())
    }

    /// Start playback
    pub async fn play(&self) -> Result<()> {
        info!("Starting playback");

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Play xmlns:u="{}">
      <InstanceID>0</InstanceID>
      <Speed>1</Speed>
    </u:Play>
  </s:Body>
</s:Envelope>"#,
            self.service_type
        );

        self.send_soap_action("Play", &body).await?;

        info!("Playback started");
        Ok(())
    }

    /// Stop playback
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping playback");

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Stop xmlns:u="{}">
      <InstanceID>0</InstanceID>
    </u:Stop>
  </s:Body>
</s:Envelope>"#,
            self.service_type
        );

        self.send_soap_action("Stop", &body).await?;

        info!("Playback stopped");
        Ok(())
    }

    /// Pause playback
    pub async fn pause(&self) -> Result<()> {
        info!("Pausing playback");

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Pause xmlns:u="{}">
      <InstanceID>0</InstanceID>
    </u:Pause>
  </s:Body>
</s:Envelope>"#,
            self.service_type
        );

        self.send_soap_action("Pause", &body).await?;

        info!("Playback paused");
        Ok(())
    }

    /// Get transport state (PLAYING, STOPPED, PAUSED_PLAYBACK, etc.)
    pub async fn get_transport_info(&self) -> Result<TransportInfo> {
        debug!("Getting transport info");

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetTransportInfo xmlns:u="{}">
      <InstanceID>0</InstanceID>
    </u:GetTransportInfo>
  </s:Body>
</s:Envelope>"#,
            self.service_type
        );

        let response = self.send_soap_action("GetTransportInfo", &body).await?;

        // Parse response
        let state = extract_xml_value(&response, "CurrentTransportState")
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let status = extract_xml_value(&response, "CurrentTransportStatus")
            .unwrap_or_else(|| "OK".to_string());

        Ok(TransportInfo { state, status })
    }

    /// Get current position info
    pub async fn get_position_info(&self) -> Result<PositionInfo> {
        debug!("Getting position info");

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetPositionInfo xmlns:u="{}">
      <InstanceID>0</InstanceID>
    </u:GetPositionInfo>
  </s:Body>
</s:Envelope>"#,
            self.service_type
        );

        let response = self.send_soap_action("GetPositionInfo", &body).await?;

        // Parse response
        let track_duration =
            extract_xml_value(&response, "TrackDuration").unwrap_or_else(|| "0:00:00".to_string());

        let rel_time =
            extract_xml_value(&response, "RelTime").unwrap_or_else(|| "0:00:00".to_string());

        Ok(PositionInfo {
            track_duration,
            rel_time,
        })
    }

    /// Send a SOAP action to the control URL
    async fn send_soap_action(&self, action: &str, body: &str) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let soap_action = format!("\"{}#{}\"", self.service_type, action);

        debug!("Sending SOAP action: {}", soap_action);
        debug!("To URL: {}", self.control_url);
        debug!("Body: {}", body);

        let response = client
            .post(&self.control_url)
            .header("Content-Type", "text/xml; charset=utf-8")
            .header("SOAPAction", soap_action)
            .body(body.to_string())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "SOAP action failed with status {}: {}",
                status,
                error_body
            ));
        }

        let response_text = response.text().await?;
        debug!("SOAP response: {}", response_text);

        Ok(response_text)
    }
}

/// Transport state information
#[derive(Debug, Clone)]
pub struct TransportInfo {
    pub state: String, // PLAYING, STOPPED, PAUSED_PLAYBACK, etc.
    pub status: String, // OK, ERROR_OCCURRED
}

/// Position information
#[derive(Debug, Clone)]
pub struct PositionInfo {
    pub track_duration: String, // Format: H:MM:SS
    pub rel_time: String,       // Format: H:MM:SS
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Extract text content from an XML tag
fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)?;

    Some(xml[start..start + end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("Hello & <World>"), "Hello &amp; &lt;World&gt;");
        assert_eq!(escape_xml("It's \"quoted\""), "It&apos;s &quot;quoted&quot;");
    }

    #[test]
    fn test_extract_xml_value() {
        let xml = r#"<root><CurrentTransportState>PLAYING</CurrentTransportState></root>"#;
        assert_eq!(
            extract_xml_value(xml, "CurrentTransportState"),
            Some("PLAYING".to_string())
        );
    }
}
