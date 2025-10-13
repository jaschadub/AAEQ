use crate::convert::convert_format;
use crate::sink::OutputSink;
use crate::sinks::dlna::{
    avtransport::AVTransport, didl::generate_simple_didl_lite, discovery::DlnaDevice,
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
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

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
    is_open: bool,
}

impl DlnaSink {
    /// Create a new DLNA sink in pull mode
    pub fn new(device_name: String, bind_addr: SocketAddr) -> Self {
        Self {
            device_name,
            device: None,
            mode: DlnaMode::Pull,
            config: None,
            server_addr: bind_addr,
            shutdown_tx: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            avtransport: None,
            is_open: false,
        }
    }

    /// Create a new DLNA sink with a discovered device (supports both modes)
    pub fn with_device(device: DlnaDevice, bind_addr: SocketAddr, mode: DlnaMode) -> Self {
        let device_name = device.name.clone();

        Self {
            device_name,
            device: Some(device),
            mode,
            config: None,
            server_addr: bind_addr,
            shutdown_tx: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            avtransport: None,
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
            // Use the server's actual IP if we know it
            if let Some(device) = &self.device {
                if let Some(ip) = &device.ip {
                    return Some(format!("http://{}:{}/stream.wav", ip, self.server_addr.port()));
                }
            }
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
    ) -> Result<mpsc::Sender<()>> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let app_state = AppState {
            buffer: buffer.clone(),
            config: config.clone(),
        };

        let app = Router::new()
            .route("/stream.wav", get(stream_handler))
            .route("/status", get(status_handler))
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

        // Stream audio data
        loop {
            // Read from buffer
            let data = {
                let mut buffer = state.buffer.lock().unwrap();
                if !buffer.is_empty() {
                    let chunk = buffer.clone();
                    buffer.clear();
                    chunk
                } else {
                    Vec::new()
                }
            };

            if !data.is_empty() {
                yield Ok(Bytes::from(data));
            } else {
                // No data available, wait a bit
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
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
        let shutdown_tx =
            Self::start_server(self.server_addr, self.buffer.clone(), cfg.clone()).await?;

        self.config = Some(cfg.clone());
        self.shutdown_tx = Some(shutdown_tx);
        self.is_open = true;

        let stream_url = self.stream_url().unwrap();
        info!("DLNA stream available at: {}", stream_url);

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

                // Generate DIDL-Lite metadata
                let didl = generate_simple_didl_lite(&stream_url, "AAEQ Stream", &cfg);

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

        // Limit buffer size to prevent memory issues
        const MAX_BUFFER_SIZE: usize = 10 * 1024 * 1024; // 10MB
        if buffer.len() > MAX_BUFFER_SIZE {
            warn!("DLNA buffer overflow, dropping old data");
            let excess = buffer.len() - MAX_BUFFER_SIZE;
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
        // DLNA typically has higher latency due to network buffering
        if let Some(cfg) = &self.config {
            let buffer_size = {
                let buffer = self.buffer.lock().unwrap();
                buffer.len()
            };

            let bytes_per_sample = cfg.format.bytes_per_sample();
            let samples = buffer_size / (bytes_per_sample * cfg.channels as usize);
            let ms = (samples as f64 / cfg.sample_rate as f64 * 1000.0) as u32;
            ms + cfg.buffer_ms // Add configured network buffer
        } else {
            0
        }
    }

    fn is_open(&self) -> bool {
        self.is_open
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
