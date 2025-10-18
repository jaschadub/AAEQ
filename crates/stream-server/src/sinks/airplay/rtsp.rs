use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::info;

/// Simple RTSP client for AirPlay (RAOP)
pub struct RtspClient {
    stream: Option<TcpStream>,
    cseq: u32,
    session: Option<String>,
    client_instance: String,
    dacp_id: String,
    active_remote: String,
}

#[derive(Debug, Clone)]
pub struct RtspResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl RtspClient {
    pub fn new() -> Self {
        // Generate random client identifiers
        let client_instance = format!("{:016X}", rand::random::<u64>());
        let dacp_id = format!("{:016X}", rand::random::<u64>());
        let active_remote = format!("{:016X}", rand::random::<u64>());

        Self {
            stream: None,
            cseq: 1,
            session: None,
            client_instance,
            dacp_id,
            active_remote,
        }
    }

    /// Connect to an RTSP server
    pub async fn connect(&mut self, host: &str, port: u16) -> Result<()> {
        info!("Connecting to RTSP server at {}:{}", host, port);
        let stream = TcpStream::connect((host, port)).await?;
        self.stream = Some(stream);
        Ok(())
    }

    /// Send an RTSP request and receive response
    async fn send_request(
        &mut self,
        method: &str,
        uri: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<RtspResponse> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;

        // Build request
        let mut request = format!("{} {} RTSP/1.0\r\n", method, uri);
        request.push_str(&format!("CSeq: {}\r\n", self.cseq));

        if let Some(session) = &self.session {
            request.push_str(&format!("Session: {}\r\n", session));
        }

        // Add AirPlay-required headers
        request.push_str("User-Agent: AirPlay/595.17.1\r\n"); // Mimic iTunes
        request.push_str(&format!("Client-Instance: {}\r\n", self.client_instance));
        request.push_str(&format!("DACP-ID: {}\r\n", self.dacp_id));
        request.push_str(&format!("Active-Remote: {}\r\n", self.active_remote));

        for (key, value) in headers {
            request.push_str(&format!("{}: {}\r\n", key, value));
        }

        if let Some(body_data) = body {
            request.push_str(&format!("Content-Length: {}\r\n", body_data.len()));
        }

        request.push_str("\r\n");

        info!("RTSP Request:\n{}", request);

        // Send request
        stream.write_all(request.as_bytes()).await?;

        if let Some(body_data) = body {
            stream.write_all(body_data).await?;
        }

        stream.flush().await?;

        self.cseq += 1;

        // Read response
        self.read_response().await
    }

    async fn read_response(&mut self) -> Result<RtspResponse> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;

        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        reader.read_line(&mut status_line).await?;

        // Parse status line: RTSP/1.0 200 OK
        let parts: Vec<&str> = status_line.trim().split_whitespace().collect();
        if parts.len() < 3 {
            return Err(anyhow!("Invalid RTSP response"));
        }

        let status_code: u16 = parts[1].parse()?;
        let status_text = parts[2..].join(" ");

        // Read headers
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;

            if line.trim().is_empty() {
                break;
            }

            if let Some((key, value)) = line.split_once(':') {
                headers.insert(
                    key.trim().to_string(),
                    value.trim().to_string(),
                );
            }
        }

        // Store session if present
        if let Some(session) = headers.get("Session") {
            self.session = Some(session.clone());
        }

        // Read body if Content-Length is present
        let body = if let Some(length_str) = headers.get("Content-Length") {
            let length: usize = length_str.parse()?;
            let mut body_data = vec![0u8; length];
            reader.get_mut().read_exact(&mut body_data).await?;
            body_data
        } else {
            Vec::new()
        };

        info!("RTSP Response: {} {} (headers: {:?})", status_code, status_text, headers);

        Ok(RtspResponse {
            status_code,
            status_text,
            headers,
            body,
        })
    }

    /// Send OPTIONS request
    pub async fn options(&mut self, uri: &str) -> Result<RtspResponse> {
        self.send_request("OPTIONS", uri, &[], None).await
    }

    /// Send ANNOUNCE request (setup audio format)
    pub async fn announce(
        &mut self,
        uri: &str,
        sdp: &str,
    ) -> Result<RtspResponse> {
        let headers = [("Content-Type", "application/sdp")];
        self.send_request("ANNOUNCE", uri, &headers, Some(sdp.as_bytes()))
            .await
    }

    /// Send SETUP request (establish RTP channels)
    pub async fn setup(
        &mut self,
        uri: &str,
        transport: &str,
    ) -> Result<RtspResponse> {
        let headers = [("Transport", transport)];
        self.send_request("SETUP", uri, &headers, None).await
    }

    /// Send RECORD request (start streaming)
    pub async fn record(
        &mut self,
        uri: &str,
        seq: u16,
        rtptime: u32,
    ) -> Result<RtspResponse> {
        let headers = [
            ("Range", "npt=0-"),
            ("RTP-Info", &format!("seq={};rtptime={}", seq, rtptime)),
        ];
        self.send_request("RECORD", uri, &headers, None).await
    }

    /// Send FLUSH request (clear buffers)
    pub async fn flush(
        &mut self,
        uri: &str,
        seq: u16,
        rtptime: u32,
    ) -> Result<RtspResponse> {
        let rtp_info = format!("seq={};rtptime={}", seq, rtptime);
        let headers = [("RTP-Info", rtp_info.as_str())];
        self.send_request("FLUSH", uri, &headers, None).await
    }

    /// Send TEARDOWN request (close session)
    pub async fn teardown(&mut self, uri: &str) -> Result<RtspResponse> {
        self.send_request("TEARDOWN", uri, &[], None).await
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<()> {
        if let Some(mut stream) = self.stream.take() {
            stream.shutdown().await?;
        }
        Ok(())
    }
}

impl Default for RtspClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate SDP (Session Description Protocol) for AirPlay audio
pub fn generate_sdp(
    _sample_rate: u32,
    _channels: u16,
    fmtp: &str,
) -> String {
    format!(
        "v=0\r\n\
         o=AAEQ 0 0 IN IP4 127.0.0.1\r\n\
         s=AAEQ\r\n\
         c=IN IP4 0.0.0.0\r\n\
         t=0 0\r\n\
         m=audio 0 RTP/AVP 96\r\n\
         a=rtpmap:96 AppleLossless\r\n\
         a=fmtp:96 {}\r\n",
        fmtp
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sdp() {
        let sdp = generate_sdp(44100, 2, "352 0 16 40 10 14 2 255 0 0 44100");
        assert!(sdp.contains("m=audio"));
        assert!(sdp.contains("AppleLossless"));
        assert!(sdp.contains("44100"));
    }

    #[tokio::test]
    async fn test_rtsp_client_create() {
        let client = RtspClient::new();
        assert_eq!(client.cseq, 1);
        assert!(client.session.is_none());
    }
}
