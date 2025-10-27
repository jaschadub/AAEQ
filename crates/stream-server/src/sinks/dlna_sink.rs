use crate::convert::convert_format;
use crate::sink::OutputSink;
use crate::sinks::dlna::{
    avtransport::AVTransport,
    device_description::{
        generate_av_transport_scpd, generate_connection_manager_scpd, generate_content_directory_scpd,
        generate_device_description, generate_device_uuid,
    },
    didl::{generate_didl_lite, MediaMetadata},
    discovery::DlnaDevice,
    ssdp_server::SsdpServer,
};
use crate::types::{AudioBlock, OutputConfig, SampleFormat};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Get the local IP address that can reach the target device
fn get_local_ip_for_device(device_ip: &IpAddr) -> Option<String> {
    use std::net::UdpSocket;

    // Create a UDP socket and connect to the device
    // This doesn't actually send data, but determines which local interface would be used
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect((*device_ip, 1234)).ok()?;

    // Get the local address that would be used to reach the target
    let local_addr = socket.local_addr().ok()?;
    Some(local_addr.ip().to_string())
}

/// Operating mode for DLNA sink
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlnaMode {
    /// Pull mode: Device pulls stream from AAEQ HTTP server (default)
    Pull,
    /// Push mode: AAEQ controls device via AVTransport
    Push,
}

/// DLNA/UPnP PCM streaming sink
/// Supports both pull mode (device pulls from HTTP server) and push mode (AVTransport control)
pub struct DlnaSink {
    device_name: String,
    device: Option<DlnaDevice>,
    mode: DlnaMode,
    config: Option<OutputConfig>,
    server_addr: SocketAddr,
    shutdown_tx: Option<mpsc::Sender<()>>,
    buffer: Arc<Mutex<Vec<u8>>>,
    avtransport: Option<AVTransport>,
    ssdp_server: Option<SsdpServer>,
    device_uuid: String,
    is_open: bool,
}

impl DlnaSink {
    /// Create a new DLNA sink in pull mode
    pub fn new(device_name: String, bind_addr: SocketAddr) -> Self {
        let device_uuid = generate_device_uuid().unwrap_or_else(|_| format!("uuid:{}", uuid::Uuid::new_v4()));

        Self {
            device_name,
            device: None,
            mode: DlnaMode::Pull,
            config: None,
            server_addr: bind_addr,
            shutdown_tx: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            avtransport: None,
            ssdp_server: None,
            device_uuid,
            is_open: false,
        }
    }

    /// Create a new DLNA sink with a discovered device (supports both modes)
    pub fn with_device(device: DlnaDevice, bind_addr: SocketAddr, mode: DlnaMode) -> Self {
        let device_name = device.name.clone();
        let device_uuid = generate_device_uuid().unwrap_or_else(|_| format!("uuid:{}", uuid::Uuid::new_v4()));

        Self {
            device_name,
            device: Some(device),
            mode,
            config: None,
            server_addr: bind_addr,
            shutdown_tx: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            avtransport: None,
            ssdp_server: None,
            device_uuid,
            is_open: false,
        }
    }

    /// Set the DLNA device to use
    pub fn set_device(&mut self, device: DlnaDevice, mode: DlnaMode) {
        self.device_name = device.name.clone();
        self.device = Some(device);
        self.mode = mode;
    }

    /// Get the stream URL that clients should connect to
    pub fn stream_url(&self) -> Option<String> {
        if self.is_open {
            // Get the local IP address that can reach the device
            if let Some(device) = &self.device {
                if let Some(device_ip) = &device.ip {
                    if let Some(local_ip) = get_local_ip_for_device(device_ip) {
                        return Some(format!("http://{}:{}/stream.wav", local_ip, self.server_addr.port()));
                    }
                }
            }
            // Fallback to bind address
            Some(format!("http://{}/stream.wav", self.server_addr))
        } else {
            None
        }
    }

    /// Start HTTP server for streaming
    async fn start_server(
        addr: SocketAddr,
        buffer: Arc<Mutex<Vec<u8>>>,
        config: OutputConfig,
        device_uuid: String,
        device_name: String,
    ) -> Result<mpsc::Sender<()>> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let app_state = AppState {
            buffer: buffer.clone(),
            config: config.clone(),
            device_uuid: device_uuid.clone(),
            device_name: device_name.clone(),
            port: addr.port(),
        };

        let app = Router::new()
            .route("/stream.wav", get(stream_handler))
            .route("/status", get(status_handler))
            .route("/album_art.png", get(album_art_handler))
            .route("/device.xml", get(device_description_handler))
            .route("/upnp/ContentDirectory.xml", get(content_directory_handler))
            .route("/upnp/ConnectionManager.xml", get(connection_manager_handler))
            .route("/upnp/AVTransport.xml", get(av_transport_handler))
            .with_state(app_state);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("DLNA HTTP server listening on {}", addr);

        tokio::spawn(async move {
            tokio::select! {
                result = axum::serve(listener, app) => {
                    match result {
                        Ok(_) => info!("DLNA server stopped"),
                        Err(e) => error!("DLNA server error: {}", e),
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("DLNA server shutdown requested");
                }
            }
        });

        Ok(shutdown_tx)
    }

}

#[derive(Clone)]
struct AppState {
    buffer: Arc<Mutex<Vec<u8>>>,
    config: OutputConfig,
    device_uuid: String,
    device_name: String,
    port: u16,
}

async fn stream_handler(State(state): State<AppState>) -> Response {
    info!("Client connected to DLNA stream");

    // Create WAV header
    let header = create_wav_header_for_config(&state.config);

    // Create streaming response
    use axum::body::Bytes;

    let stream = async_stream::stream! {
        // Send WAV header first
        yield Ok::<Bytes, std::io::Error>(Bytes::from(header));

        // Stream audio data in small chunks for low latency
        loop {
            // Read from buffer in smaller chunks (max 200ms worth)
            // This reduces latency when EQ changes
            let data = {
                let mut buffer = state.buffer.lock().unwrap();
                if !buffer.is_empty() {
                    // Calculate max chunk size (200ms of audio)
                    let bytes_per_sample = state.config.format.bytes_per_sample();
                    let samples_per_sec = state.config.sample_rate * state.config.channels as u32;
                    let max_chunk_size = (samples_per_sec / 5) as usize * bytes_per_sample; // 200ms

                    let chunk_size = buffer.len().min(max_chunk_size);
                    let chunk: Vec<u8> = buffer.drain(..chunk_size).collect();
                    chunk
                } else {
                    Vec::new()
                }
            };

            if !data.is_empty() {
                yield Ok(Bytes::from(data));
            } else {
                // No data available, wait a bit
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/wav")
        .header(header::TRANSFER_ENCODING, "chunked")
        .body(Body::from_stream(stream))
        .unwrap()
}

async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let buffer_size = {
        let buffer = state.buffer.lock().unwrap();
        buffer.len()
    };

    let status = serde_json::json!({
        "status": "streaming",
        "sample_rate": state.config.sample_rate,
        "channels": state.config.channels,
        "format": format!("{:?}", state.config.format),
        "buffer_bytes": buffer_size,
    });

    (StatusCode::OK, axum::Json(status))
}

async fn album_art_handler() -> Response {
    // Serve the default aaeq-icon.png as album art
    let icon_path = "aaeq-icon.png";

    match tokio::fs::read(icon_path).await {
        Ok(data) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/png")
                .header(header::CACHE_CONTROL, "public, max-age=86400")
                .body(Body::from(data))
                .unwrap()
        }
        Err(e) => {
            error!("Failed to read album art icon: {}", e);
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap()
        }
    }
}

async fn device_description_handler(State(state): State<AppState>) -> Response {
    match generate_device_description(&state.device_uuid, &state.device_name, state.port) {
        Ok(xml) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
            .header(header::CACHE_CONTROL, "public, max-age=1800")
            .body(Body::from(xml))
            .unwrap(),
        Err(e) => {
            error!("Failed to generate device description: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        }
    }
}

async fn content_directory_handler() -> Response {
    let xml = generate_content_directory_scpd();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=1800")
        .body(Body::from(xml))
        .unwrap()
}

async fn connection_manager_handler() -> Response {
    let xml = generate_connection_manager_scpd();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=1800")
        .body(Body::from(xml))
        .unwrap()
}

async fn av_transport_handler() -> Response {
    let xml = generate_av_transport_scpd();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=1800")
        .body(Body::from(xml))
        .unwrap()
}

fn create_wav_header_for_config(cfg: &OutputConfig) -> Vec<u8> {
    let sample_rate = cfg.sample_rate;
    let channels = cfg.channels;
    let bits_per_sample = cfg.format.bit_depth() as u16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);

    let mut header = Vec::new();

    // RIFF header
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
    header.extend_from_slice(b"WAVE");

    // fmt chunk
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    header.extend_from_slice(b"data");
    header.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());

    header
}

#[async_trait]
impl OutputSink for DlnaSink {
    fn name(&self) -> &'static str {
        "dlna"
    }

    async fn open(&mut self, cfg: OutputConfig) -> Result<()> {
        debug!("Opening DLNA sink for device: {}", self.device_name);

        // Validate format - DLNA typically supports PCM only
        if !matches!(cfg.format, SampleFormat::S16LE | SampleFormat::S24LE) {
            return Err(anyhow!(
                "DLNA sink requires S16LE or S24LE format, got {:?}",
                cfg.format
            ));
        }

        // Start HTTP server (required for both modes)
        let shutdown_tx = Self::start_server(
            self.server_addr,
            self.buffer.clone(),
            cfg.clone(),
            self.device_uuid.clone(),
            self.device_name.clone(),
        )
        .await?;

        self.config = Some(cfg.clone());
        self.shutdown_tx = Some(shutdown_tx);
        self.is_open = true;

        let stream_url = self.stream_url().unwrap();
        info!("DLNA stream available at: {}", stream_url);

        // Start SSDP server for automatic device discovery
        let mut ssdp_server = SsdpServer::new(
            self.device_uuid.clone(),
            self.device_name.clone(),
            self.server_addr.port(),
        );
        if let Err(e) = ssdp_server.start().await {
            warn!("Failed to start SSDP server: {}", e);
        } else {
            info!("SSDP server started - AAEQ is now discoverable on the network");
            self.ssdp_server = Some(ssdp_server);
        }

        // If in push mode, set up AVTransport control
        if self.mode == DlnaMode::Push {
            if let Some(device) = &self.device {
                // Find AVTransport service
                let avtransport_service = device
                    .services
                    .iter()
                    .find(|s| s.service_type.contains("AVTransport"))
                    .ok_or_else(|| {
                        anyhow!("Device {} does not support AVTransport", device.name)
                    })?;

                info!("Setting up AVTransport control for push mode");
                let avtransport = AVTransport::new(
                    avtransport_service.control_url.clone(),
                    avtransport_service.service_type.clone(),
                );

                // Generate DIDL-Lite metadata with album art
                // Extract host:port from stream URL and construct album art URL
                let album_art_url = if let Some(host_port) = stream_url.split("://").nth(1).and_then(|s| s.split('/').next()) {
                    format!("http://{}/album_art.png", host_port)
                } else {
                    format!("http://{}/album_art.png", self.server_addr)
                };

                let metadata = MediaMetadata {
                    title: "AAEQ Stream".to_string(),
                    artist: None,
                    album: None,
                    genre: None,
                    duration: None,
                    album_art_uri: Some(album_art_url),
                };

                let didl = generate_didl_lite(&stream_url, &metadata, &cfg);

                // Set the URI on the renderer
                avtransport
                    .set_av_transport_uri(&stream_url, Some(&didl))
                    .await?;

                // Start playback
                avtransport.play().await?;

                self.avtransport = Some(avtransport);
                info!("Push mode active: device will pull from {}", stream_url);
            } else {
                return Err(anyhow!(
                    "Push mode requires a discovered device. Use set_device() first."
                ));
            }
        } else {
            info!("Pull mode: Configure your device to pull from this URL");
        }

        Ok(())
    }

    async fn write(&mut self, block: AudioBlock<'_>) -> Result<()> {
        if !self.is_open {
            return Err(anyhow!("DLNA sink not open"));
        }

        let cfg = self.config.as_ref().unwrap();

        // Convert to target format
        let mut converted = Vec::new();
        convert_format(block, cfg.format, &mut converted)?;

        // Append to buffer
        let mut buffer = self.buffer.lock().unwrap();
        buffer.extend_from_slice(&converted);

        // Limit buffer size to prevent excessive latency
        // Calculate max buffer size for ~1 second of audio
        let bytes_per_sample = cfg.format.bytes_per_sample();
        let samples_per_sec = cfg.sample_rate * cfg.channels as u32;
        const MAX_BUFFER_MS: u32 = 1000; // 1 second max to minimize EQ change latency
        let max_buffer_size = (samples_per_sec * MAX_BUFFER_MS / 1000) as usize * bytes_per_sample;

        if buffer.len() > max_buffer_size {
            debug!("DLNA buffer has {} bytes (max {}), dropping old data to reduce latency",
                   buffer.len(), max_buffer_size);
            // Drop old data to keep latency low
            let excess = buffer.len() - max_buffer_size;
            buffer.drain(0..excess);
        }

        Ok(())
    }

    async fn drain(&mut self) -> Result<()> {
        // Wait for buffer to be consumed by clients
        loop {
            let buffer_size = {
                let buffer = self.buffer.lock().unwrap();
                buffer.len()
            };

            if buffer_size == 0 {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        debug!("Closing DLNA sink");

        // Stop SSDP server
        if let Some(mut ssdp_server) = self.ssdp_server.take() {
            info!("Stopping SSDP server");
            if let Err(e) = ssdp_server.stop().await {
                warn!("Failed to stop SSDP server: {}", e);
            }
        }

        // Stop playback if in push mode
        if let Some(avtransport) = &self.avtransport {
            info!("Stopping AVTransport playback");
            if let Err(e) = avtransport.stop().await {
                warn!("Failed to stop AVTransport: {}", e);
            }
        }

        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        self.config = None;
        self.avtransport = None;
        self.is_open = false;

        // Clear buffer
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();

        info!("DLNA sink closed");
        Ok(())
    }

    fn latency_ms(&self) -> u32 {
        // DLNA typically has higher latency due to network buffering and device buffering
        // Most DLNA devices buffer 2-4 seconds for network reliability
        if let Some(cfg) = &self.config {
            let buffer_size = {
                let buffer = self.buffer.lock().unwrap();
                buffer.len()
            };

            let bytes_per_sample = cfg.format.bytes_per_sample();
            let samples = buffer_size / (bytes_per_sample * cfg.channels as usize);
            let buffer_latency_ms = (samples as f64 / cfg.sample_rate as f64 * 1000.0) as u32;

            // Base DLNA device latency (4 seconds is typical for network devices like WiiM)
            // DLNA devices buffer heavily for network reliability and multi-room sync
            // + internal buffer latency + configured buffer
            let base_dlna_latency = 4000;
            base_dlna_latency + buffer_latency_ms + cfg.buffer_ms
        } else {
            0
        }
    }

    fn is_open(&self) -> bool {
        self.is_open
    }

    fn stats(&self) -> crate::sink::SinkStats {
        if let Some(cfg) = &self.config {
            // Calculate buffer fill (0.0 to 1.0)
            let buffer_size = {
                let buffer = self.buffer.lock().unwrap();
                buffer.len()
            };

            // Max buffer is 1 second of audio (see write method MAX_BUFFER_MS)
            let bytes_per_sample = cfg.format.bytes_per_sample();
            let samples_per_sec = cfg.sample_rate as usize * cfg.channels as usize;
            let max_buffer_size = samples_per_sec * bytes_per_sample; // 1 second

            let buffer_fill = if max_buffer_size > 0 {
                (buffer_size as f32 / max_buffer_size as f32).min(1.0)
            } else {
                0.0
            };

            // Log buffer metrics periodically for debugging
            tracing::trace!(
                "DLNA buffer: {} / {} bytes ({:.1}%)",
                buffer_size,
                max_buffer_size,
                buffer_fill * 100.0
            );

            crate::sink::SinkStats {
                frames_written: 0, // Managed by OutputManager
                underruns: 0,      // DLNA doesn't track underruns (network buffering handles this)
                overruns: 0,       // DLNA doesn't have overruns (buffer is capped)
                buffer_fill,
            }
        } else {
            crate::sink::SinkStats::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dlna_sink_create() {
        let addr = "127.0.0.1:8090".parse().unwrap();
        let sink = DlnaSink::new("Test Device".to_string(), addr);
        assert_eq!(sink.name(), "dlna");
        assert!(!sink.is_open());
    }

    #[test]
    fn test_wav_header_creation() {
        let config = OutputConfig {
            sample_rate: 48000,
            channels: 2,
            format: SampleFormat::S16LE,
            buffer_ms: 150,
            exclusive: false,
        };

        let header = create_wav_header_for_config(&config);

        // WAV header should be 44 bytes
        assert_eq!(header.len(), 44);

        // Check RIFF signature
        assert_eq!(&header[0..4], b"RIFF");

        // Check WAVE signature
        assert_eq!(&header[8..12], b"WAVE");

        // Check fmt chunk
        assert_eq!(&header[12..16], b"fmt ");
    }
}
