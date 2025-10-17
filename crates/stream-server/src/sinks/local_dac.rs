use crate::convert::convert_format;
use crate::sink::OutputSink;
use crate::types::{AudioBlock, OutputConfig, SampleFormat};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Local DAC output sink using CPAL (cross-platform)
/// Note: This struct is not Send/Sync due to cpal::Stream
/// It should be used within a single thread or task
pub struct LocalDacSink {
    device_name: Option<String>,
    host: Host,
    device: Option<Device>,
    stream: Option<Stream>,
    config: Option<OutputConfig>,
    #[allow(dead_code)]
    tx: Option<mpsc::Sender<Vec<u8>>>,
    buffer: Arc<Mutex<RingBuffer>>,
    is_open: bool,
}

// Manual implementation of Send since we manage the Stream safely
unsafe impl Send for LocalDacSink {}
unsafe impl Sync for LocalDacSink {}

/// Simple ring buffer for audio data
struct RingBuffer {
    data: Vec<u8>,
    write_pos: usize,
    read_pos: usize,
    capacity: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            write_pos: 0,
            read_pos: 0,
            capacity,
        }
    }

    fn available_write(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.capacity - (self.write_pos - self.read_pos) - 1
        } else {
            self.read_pos - self.write_pos - 1
        }
    }

    fn available_read(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - (self.read_pos - self.write_pos)
        }
    }

    fn write(&mut self, data: &[u8]) -> usize {
        let available = self.available_write();
        let to_write = data.len().min(available);

        for i in 0..to_write {
            self.data[self.write_pos] = data[i];
            self.write_pos = (self.write_pos + 1) % self.capacity;
        }

        to_write
    }

    fn read(&mut self, output: &mut [u8]) -> usize {
        let available = self.available_read();
        let to_read = output.len().min(available);

        for i in 0..to_read {
            output[i] = self.data[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }

        to_read
    }
}

impl LocalDacSink {
    /// Create a new local DAC sink with optional device name
    pub fn new(device_name: Option<String>) -> Self {
        let host = cpal::default_host();
        Self {
            device_name,
            host,
            device: None,
            stream: None,
            config: None,
            tx: None,
            buffer: Arc::new(Mutex::new(RingBuffer::new(1024 * 1024))), // 1MB buffer
            is_open: false,
        }
    }

    /// List available audio output devices
    pub fn list_devices() -> Result<Vec<String>> {
        let host = cpal::default_host();
        let devices = host.output_devices()?;

        let mut names = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                names.push(name);
            }
        }

        Ok(names)
    }

    fn select_device(&mut self) -> Result<Device> {
        if let Some(ref name) = self.device_name {
            // Try to find device by name
            let devices = self.host.output_devices()?;
            for device in devices {
                if let Ok(device_name) = device.name() {
                    if device_name == *name {
                        info!("Selected audio device: {}", name);
                        return Ok(device);
                    }
                }
            }
            warn!("Device '{}' not found, using default", name);
        }

        // Use default device
        self.host
            .default_output_device()
            .ok_or_else(|| anyhow!("No output device available"))
    }

    fn create_stream_config(&self, cfg: &OutputConfig) -> Result<StreamConfig> {
        let cpal_config = StreamConfig {
            channels: cfg.channels,
            sample_rate: cpal::SampleRate(cfg.sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        Ok(cpal_config)
    }
}

#[async_trait]
impl OutputSink for LocalDacSink {
    fn name(&self) -> &'static str {
        "local_dac"
    }

    async fn open(&mut self, cfg: OutputConfig) -> Result<()> {
        debug!("Opening local DAC with config: {:?}", cfg);

        // Select device
        let device = self.select_device()?;
        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Using audio device: {}", device_name);

        // Check what formats the device supports
        let supported_configs = device.supported_output_configs()?;
        let mut supports_f32 = false;
        let mut supports_i16 = false;

        for config_range in supported_configs {
            match config_range.sample_format() {
                cpal::SampleFormat::F32 => supports_f32 = true,
                cpal::SampleFormat::I16 => supports_i16 = true,
                _ => {}
            }
        }

        debug!("Device format support: F32={}, I16={}", supports_f32, supports_i16);

        // Determine actual format to use
        let actual_format = match cfg.format {
            SampleFormat::F32 if !supports_f32 && supports_i16 => {
                warn!("Device doesn't support F32, falling back to S16LE");
                SampleFormat::S16LE
            }
            SampleFormat::S16LE if !supports_i16 && supports_f32 => {
                warn!("Device doesn't support S16LE, falling back to F32");
                SampleFormat::F32
            }
            _ => cfg.format
        };

        // Create stream config
        let stream_config = self.create_stream_config(&cfg)?;

        // Create buffer for audio data
        let buffer = self.buffer.clone();

        // Create audio stream based on actual format we'll use
        let stream = match actual_format {
            SampleFormat::F32 => {
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let mut buffer = buffer.lock().unwrap();
                        let bytes_needed = data.len() * 4; // f32 = 4 bytes
                        let mut bytes = vec![0u8; bytes_needed];
                        let bytes_read = buffer.read(&mut bytes);

                        // Convert bytes to f32 samples
                        for (i, chunk) in bytes[..bytes_read].chunks_exact(4).enumerate() {
                            if i < data.len() {
                                data[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                            }
                        }

                        // Fill remainder with silence
                        for i in (bytes_read / 4)..data.len() {
                            data[i] = 0.0;
                        }
                    },
                    |err| error!("Stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::S16LE => {
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let mut buffer = buffer.lock().unwrap();
                        let bytes_needed = data.len() * 2; // i16 = 2 bytes
                        let mut bytes = vec![0u8; bytes_needed];
                        let bytes_read = buffer.read(&mut bytes);

                        // Convert bytes to i16 samples
                        for (i, chunk) in bytes[..bytes_read].chunks_exact(2).enumerate() {
                            if i < data.len() {
                                data[i] = i16::from_le_bytes([chunk[0], chunk[1]]);
                            }
                        }

                        // Fill remainder with silence
                        for i in (bytes_read / 2)..data.len() {
                            data[i] = 0;
                        }
                    },
                    |err| error!("Stream error: {}", err),
                    None,
                )?
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported format {:?} for local DAC (use F32 or S16LE)",
                    actual_format
                ));
            }
        };

        // Start the stream
        stream.play()?;
        info!("Audio stream started with format: {:?}", actual_format);

        // Store the config with the actual format we're using
        let mut actual_cfg = cfg;
        actual_cfg.format = actual_format;

        self.device = Some(device);
        self.stream = Some(stream);
        self.config = Some(actual_cfg);
        self.is_open = true;

        Ok(())
    }

    async fn write(&mut self, block: AudioBlock<'_>) -> Result<()> {
        if !self.is_open() {
            return Err(anyhow!("Sink not open"));
        }

        let cfg = self.config.as_ref().unwrap();

        // Convert audio to target format
        let mut converted = Vec::new();
        convert_format(block, cfg.format, &mut converted)?;

        // Write to ring buffer
        let mut buffer = self.buffer.lock().unwrap();
        let written = buffer.write(&converted);

        if written < converted.len() {
            warn!("Buffer overflow: {} bytes dropped", converted.len() - written);
        }

        Ok(())
    }

    async fn drain(&mut self) -> Result<()> {
        // Wait for buffer to drain
        loop {
            let available = {
                let buffer = self.buffer.lock().unwrap();
                buffer.available_read()
            };

            if available == 0 {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        debug!("Closing local DAC sink");

        if let Some(stream) = self.stream.take() {
            drop(stream); // Stream will stop when dropped
        }

        self.device = None;
        self.config = None;
        self.is_open = false;

        info!("Local DAC sink closed");
        Ok(())
    }

    fn latency_ms(&self) -> u32 {
        // Estimate latency based on buffer fill
        if let Some(cfg) = &self.config {
            let buffer = self.buffer.lock().unwrap();
            let bytes_buffered = buffer.available_read();
            let bytes_per_sample = cfg.format.bytes_per_sample();
            let samples_buffered = bytes_buffered / (bytes_per_sample * cfg.channels as usize);
            let ms = (samples_buffered as f64 / cfg.sample_rate as f64 * 1000.0) as u32;
            ms + 20 // Add ~20ms for device latency
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

    #[test]
    fn test_ring_buffer_write_read() {
        let mut buffer = RingBuffer::new(100);

        let data = vec![1u8, 2, 3, 4, 5];
        let written = buffer.write(&data);
        assert_eq!(written, 5);

        let mut output = vec![0u8; 5];
        let read = buffer.read(&mut output);
        assert_eq!(read, 5);
        assert_eq!(output, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_ring_buffer_wrap_around() {
        let mut buffer = RingBuffer::new(10);

        // Fill most of buffer
        let data = vec![1u8; 8];
        buffer.write(&data);

        // Read some
        let mut output = vec![0u8; 5];
        buffer.read(&mut output);

        // Write more (should wrap around)
        let data2 = vec![2u8; 5];
        let written = buffer.write(&data2);
        assert_eq!(written, 5);
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buffer = RingBuffer::new(10);

        let data = vec![1u8; 20]; // More than capacity
        let written = buffer.write(&data);
        assert!(written < 20); // Should not write all
        assert!(written <= 9); // Max available (capacity - 1)
    }

    #[tokio::test]
    async fn test_local_dac_create() {
        let sink = LocalDacSink::new(None);
        assert_eq!(sink.name(), "local_dac");
        assert!(!sink.is_open());
    }

    // Note: Can't easily test actual audio output without a physical device
    // Integration tests should be run manually
}
