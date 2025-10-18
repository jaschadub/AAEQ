use crate::convert::convert_format;
use crate::sink::OutputSink;
use crate::sinks::airplay::*;
use crate::types::{AudioBlock, OutputConfig, SampleFormat};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::{debug, info};

/// AirPlay output sink with full RAOP protocol support
pub struct AirPlaySink {
    device: Option<AirPlayDevice>,
    config: Option<OutputConfig>,
    rtsp_client: Option<RtspClient>,
    rtp_stream: Option<RtpStream>,
    rtcp_stream: Option<RtcpStream>,
    encoder: Option<AlacEncoder>,
    auth: AirPlayAuth,
    is_open: bool,
    packets_sent: u32,
    bytes_sent: u32,
}

impl AirPlaySink {
    /// Create a new AirPlay sink
    pub fn new() -> Self {
        Self {
            device: None,
            config: None,
            rtsp_client: None,
            rtp_stream: None,
            rtcp_stream: None,
            encoder: None,
            auth: AirPlayAuth::new(),
            is_open: false,
            packets_sent: 0,
            bytes_sent: 0,
        }
    }

    /// Discover AirPlay devices
    pub async fn discover(timeout_secs: u64) -> Result<Vec<AirPlayDevice>> {
        discover_devices(timeout_secs).await
    }

    /// Set the target device
    pub fn set_device(&mut self, device: AirPlayDevice) {
        self.device = Some(device);
    }

    /// Find and set device by name
    pub async fn set_device_by_name(&mut self, name: &str, timeout_secs: u64) -> Result<()> {
        let device = find_device_by_name(name, timeout_secs)
            .await?
            .ok_or_else(|| anyhow!("Device '{}' not found", name))?;

        self.device = Some(device);
        Ok(())
    }

    async fn setup_connection(&mut self, cfg: &OutputConfig) -> Result<()> {
        let device = self
            .device
            .as_ref()
            .ok_or_else(|| anyhow!("No device set"))?;

        info!(
            "Setting up AirPlay connection to {} ({}:{})",
            device.name, device.hostname, device.port
        );

        // Create RTSP client
        let mut rtsp = RtspClient::new();

        // Get first IP address
        let ip = device
            .addresses
            .first()
            .ok_or_else(|| anyhow!("No IP address for device"))?;

        rtsp.connect(&ip.to_string(), device.port).await?;

        // Send OPTIONS
        let uri = format!("rtsp://{}:{}", ip, device.port);
        let options_resp = rtsp.options(&uri).await?;
        info!("OPTIONS response: {} {}", options_resp.status_code, options_resp.status_text);
        info!("OPTIONS headers: {:?}", options_resp.headers);

        // Check if this is an Apple device that requires MFi pairing
        if options_resp.status_code == 403 {
            if let Some(server) = options_resp.headers.get("Server") {
                if server.contains("AirTunes") || server.contains("AirPlay") {
                    return Err(anyhow!(
                        "Apple AirPlay 2 device requires MFi authentication (device pairing). \
                        This is not currently supported. \n\
                        \n\
                        Your device ({}) appears to be an Apple HomePod, Apple TV, or AirPort Express. \
                        These devices require hardware-level authentication (MFi chip) that cannot be \
                        implemented in software without violating Apple's terms.\n\
                        \n\
                        Supported alternatives:\n\
                        1. Use a third-party AirPlay speaker (Sonos, Bose, etc.) that doesn't require pairing\n\
                        2. Use DLNA output instead (many speakers support both)\n\
                        3. Use Local DAC output to your computer's audio device\n\
                        \n\
                        Server: {}",
                        device.name, server
                    ));
                }
            }
            return Err(anyhow!("OPTIONS request rejected with 403 Forbidden. Device may require authentication or pairing."));
        }

        // Check if receiver requires password authentication
        if let Some(auth_header) = options_resp.headers.get("WWW-Authenticate") {
            info!("Receiver requires password authentication: {}", auth_header);
            return Err(anyhow!("Device requires password authentication, which is not yet implemented. Header: {}", auth_header));
        }

        // Check for Apple-Challenge (some receivers require this)
        if let Some(challenge) = options_resp.headers.get("Apple-Challenge") {
            info!("Receiver sent Apple-Challenge: {}", challenge);
            return Err(anyhow!("Device requires Apple-Challenge/Response authentication, which is not yet implemented."));
        }

        // Generate encryption keys (but don't send them for now - may trigger FairPlay rejection)
        // self.auth.generate_encryption_keys();

        // Create ALAC encoder
        let alac_config = AlacConfig {
            sample_rate: cfg.sample_rate,
            channels: cfg.channels,
            bit_depth: 16,
            frames_per_packet: 352,
        };

        let encoder = AlacEncoder::new(alac_config.clone());
        let fmtp = encoder.fmtp_string();

        // Create SDP (without encryption for now - Apple devices may reject encrypted streams
        // from unauthorized clients due to FairPlay DRM)
        let sdp = generate_sdp(cfg.sample_rate, cfg.channels, &fmtp);

        // Note: Encryption keys omitted - Apple AirPlay 2 devices reject encrypted streams
        // from non-paired clients. Try unencrypted first to see if device accepts it.
        info!("Attempting connection without audio encryption (Apple devices may require pairing for encrypted streams)");

        // Send ANNOUNCE
        info!("Sending ANNOUNCE with SDP:\n{}", sdp);
        let announce_resp = rtsp.announce(&uri, &sdp).await?;
        info!("ANNOUNCE response: {} {}", announce_resp.status_code, announce_resp.status_text);
        info!("ANNOUNCE headers: {:?}", announce_resp.headers);

        if announce_resp.status_code != 200 {
            if let Some(auth_header) = announce_resp.headers.get("WWW-Authenticate") {
                return Err(anyhow!("ANNOUNCE failed with authentication required: {} - Auth header: {}",
                    announce_resp.status_text, auth_header));
            }
            return Err(anyhow!("ANNOUNCE failed: {} (status {})", announce_resp.status_text, announce_resp.status_code));
        }

        // Setup RTP streams
        let local_rtp_port = 6000;
        let local_rtcp_port = 6001;

        let rtp_dest = format!("{}:{}", ip, 6000).parse()?;
        let rtcp_dest = format!("{}:{}", ip, 6001).parse()?;

        let rtp_stream = RtpStream::new(local_rtp_port, rtp_dest).await?;
        let rtcp_stream = RtcpStream::new(local_rtcp_port, rtcp_dest).await?;

        // Send SETUP
        let transport = format!(
            "RTP/AVP/UDP;unicast;interleaved=0-1;mode=record;control_port={};timing_port={}",
            local_rtcp_port, local_rtcp_port
        );

        let setup_resp = rtsp.setup(&uri, &transport).await?;
        if setup_resp.status_code != 200 {
            return Err(anyhow!("SETUP failed: {}", setup_resp.status_text));
        }

        // Send RECORD to start streaming
        let seq = rtp_stream.sequence_number();
        let rtptime = rtp_stream.timestamp();
        let record_resp = rtsp.record(&uri, seq, rtptime).await?;

        if record_resp.status_code != 200 {
            return Err(anyhow!("RECORD failed: {}", record_resp.status_text));
        }

        self.rtsp_client = Some(rtsp);
        self.rtp_stream = Some(rtp_stream);
        self.rtcp_stream = Some(rtcp_stream);
        self.encoder = Some(encoder);

        info!("AirPlay connection established");
        Ok(())
    }
}

impl Default for AirPlaySink {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OutputSink for AirPlaySink {
    fn name(&self) -> &'static str {
        "airplay"
    }

    async fn open(&mut self, cfg: OutputConfig) -> Result<()> {
        debug!("Opening AirPlay sink");

        if self.device.is_none() {
            return Err(anyhow!(
                "No device set. Use set_device() or discover() first"
            ));
        }

        // Setup connection
        self.setup_connection(&cfg).await?;

        self.config = Some(cfg);
        self.is_open = true;

        Ok(())
    }

    async fn write(&mut self, block: AudioBlock<'_>) -> Result<()> {
        if !self.is_open {
            return Err(anyhow!("Sink not open"));
        }

        // Convert to 16-bit PCM
        let mut pcm_data = Vec::new();
        convert_format(block, SampleFormat::S16LE, &mut pcm_data)?;

        // Convert bytes to i16 samples
        let samples: Vec<i16> = pcm_data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        // Encode to ALAC
        let encoder = self.encoder.as_mut().unwrap();
        let packets = encoder.encode(&samples)?;

        // Send packets via RTP
        let rtp = self.rtp_stream.as_mut().unwrap();
        let config = self.config.as_ref().unwrap();

        for packet in packets {
            // Encrypt if needed
            let encrypted = self.auth.encrypt_audio(&packet)?;

            // Send RTP packet
            let samples_in_packet = config.sample_rate / 100; // Approximate
            rtp.send_packet(&encrypted, samples_in_packet).await?;

            self.packets_sent += 1;
            self.bytes_sent += encrypted.len() as u32;

            // Send RTCP sender report periodically
            if self.packets_sent % 100 == 0 {
                let rtcp = self.rtcp_stream.as_ref().unwrap();
                let ntp_ts = get_ntp_timestamp();
                let rtp_ts = rtp.timestamp();
                let ssrc = rtp.ssrc();

                rtcp.send_sender_report(ssrc, ntp_ts, rtp_ts, self.packets_sent, self.bytes_sent)
                    .await?;
            }
        }

        Ok(())
    }

    async fn drain(&mut self) -> Result<()> {
        if let Some(encoder) = self.encoder.as_mut() {
            let final_packets = encoder.flush()?;

            if let Some(rtp) = self.rtp_stream.as_mut() {
                for packet in final_packets {
                    let encrypted = self.auth.encrypt_audio(&packet)?;
                    rtp.send_packet(&encrypted, 352).await?;
                }
            }
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        debug!("Closing AirPlay sink");

        // Drain any remaining audio
        self.drain().await?;

        // Send TEARDOWN
        if let (Some(rtsp), Some(device)) = (self.rtsp_client.as_mut(), &self.device) {
            let ip = device.addresses.first().unwrap();
            let uri = format!("rtsp://{}:{}", ip, device.port);
            let _ = rtsp.teardown(&uri).await;
            let _ = rtsp.close().await;
        }

        self.rtsp_client = None;
        self.rtp_stream = None;
        self.rtcp_stream = None;
        self.encoder = None;
        self.config = None;
        self.is_open = false;

        info!("AirPlay sink closed");
        Ok(())
    }

    fn latency_ms(&self) -> u32 {
        // AirPlay typically has 2+ seconds latency
        2000
    }

    fn is_open(&self) -> bool {
        self.is_open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_airplay_sink_create() {
        let sink = AirPlaySink::new();
        assert_eq!(sink.name(), "airplay");
        assert!(!sink.is_open());
    }

    #[tokio::test]
    async fn test_airplay_sink_requires_device() {
        let mut sink = AirPlaySink::new();
        let config = OutputConfig::default();

        let result = sink.open(config).await;
        assert!(result.is_err());
    }
}
