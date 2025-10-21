/// Audio input/capture abstractions for the stream server
///
/// This module provides functionality for capturing audio from various sources:
/// - System audio (loopback/monitor devices)
/// - Application-specific audio
/// - File playback
/// - Network streams

use crate::types::OutputConfig;
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

// Windows-specific imports for WASAPI loopback
#[cfg(target_os = "windows")]
use wasapi::*;

/// Input source that captures from system audio (loopback/monitor device)
pub struct LocalDacInput;

/// Wrapper around cpal::Stream that implements Send
/// This is safe because we manage the stream carefully and don't actually
/// access it from multiple threads - we just need to move it for lifetime management
struct StreamHolder(#[allow(dead_code)] cpal::Stream);

unsafe impl Send for StreamHolder {}
unsafe impl Sync for StreamHolder {}

impl LocalDacInput {
    /// List available input devices
    pub fn list_devices() -> Result<Vec<String>> {
        let host = cpal::default_host();
        let mut devices = Vec::new();

        // Add default device
        if let Some(device) = host.default_input_device() {
            if let Ok(name) = device.name() {
                devices.push(format!("default ({})", name));
            }
        }

        // On Windows, enumerate WASAPI loopback devices (system audio capture)
        #[cfg(target_os = "windows")]
        {
            if let Ok(loopback_devices) = Self::list_windows_loopback_devices() {
                devices.extend(loopback_devices);
            }
        }

        // Add ALSA-configured capture devices
        // Check if aaeq_monitor or aaeq_capture exists (configured in .asoundrc)
        if let Ok(output) = std::process::Command::new("arecord")
            .args(["-L"])
            .output()
        {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                for line in output_str.lines() {
                    let line = line.trim();
                    if line == "aaeq_capture" {
                        devices.push("🔊 aaeq_capture (AAEQ Capture - System Audio)".to_string());
                    } else if line == "aaeq_monitor" {
                        devices.push("🔊 aaeq_monitor (AAEQ Monitor - System Audio)".to_string());
                    } else if line == "pulse" && !devices.iter().any(|d| d.contains("pulse")) {
                        devices.push("pulse (PulseAudio)".to_string());
                    }
                }
            }
        }

        // Try to get PulseAudio monitor devices using pactl
        if let Ok(output) = std::process::Command::new("pactl")
            .args(["list", "sources"])
            .output()
        {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                let lines: Vec<&str> = output_str.lines().collect();
                let mut i = 0;
                while i < lines.len() {
                    if lines[i].trim().starts_with("Name:") {
                        let name = lines[i]
                            .trim()
                            .strip_prefix("Name:")
                            .unwrap_or("")
                            .trim()
                            .to_string();

                        // Look ahead for description
                        let mut description = name.clone();
                        for j in i + 1..std::cmp::min(i + 5, lines.len()) {
                            if lines[j].trim().starts_with("Description:") {
                                description = lines[j]
                                    .trim()
                                    .strip_prefix("Description:")
                                    .unwrap_or("")
                                    .trim()
                                    .to_string();
                                break;
                            }
                        }

                        // Check if it's a monitor device
                        if name.contains(".monitor") {
                            // Don't add if already added via ALSA
                            if !devices.iter().any(|d| d.contains(&name)) {
                                devices.push(format!("🔊 {} ({})", name, description));
                            }
                        }
                    }
                    i += 1;
                }
            }
        }

        // List all input devices with indicators for loopback/monitor devices
        for device in host.input_devices()? {
            if let Ok(name) = device.name() {
                // Skip if already added (avoid duplicates from ALSA/PulseAudio discovery)
                if devices.iter().any(|d| d.contains(&name)) {
                    continue;
                }

                let lower_name = name.to_lowercase();

                // Mark monitor/loopback devices that capture system audio
                if lower_name.contains("monitor")
                    || lower_name.contains("loopback")
                    || lower_name.contains("stereo mix")
                    || lower_name.contains("wave out mix")
                    || lower_name.contains("what u hear")
                {
                    devices.push(format!("🔊 {} (system audio)", name));
                } else {
                    devices.push(name);
                }
            }
        }

        Ok(devices)
    }

    /// Start capturing audio from the specified device
    ///
    /// This function starts audio capture and manages the stream lifetime internally.
    /// Audio samples are sent through the provided channel as Vec<f64>.
    ///
    /// # Arguments
    /// * `device_name` - Optional device name. If None, uses default input device
    /// * `cfg` - Audio configuration (sample rate, channels, etc.)
    /// * `tx` - Channel to send captured audio samples
    ///
    /// # Returns
    /// A stop_sender channel - send `()` to stop the capture. The stream handle is
    /// managed internally in a dedicated thread and will be dropped when stopped.
    pub fn start_capture(
        device_name: Option<String>,
        cfg: OutputConfig,
        tx: mpsc::Sender<Vec<f64>>,
    ) -> Result<mpsc::Sender<()>> {
        info!(
            "Starting local DAC input capture ({})",
            device_name.as_deref().unwrap_or("default")
        );

        // On Windows, check if this is a loopback device
        #[cfg(target_os = "windows")]
        {
            if let Some(ref name) = device_name {
                if name.contains("(Loopback)") {
                    info!("Detected Windows WASAPI loopback device, using WASAPI directly");
                    return Self::start_wasapi_loopback_capture(name, cfg, tx);
                }
            }
        }

        let host = cpal::default_host();

        // Find the device
        let device = if let Some(ref name) = device_name {
            if name.starts_with("default") {
                host.default_input_device()
                    .ok_or_else(|| anyhow!("No default input device available"))?
            } else {
                // Strip the emoji and description if present
                // Format: "🔊 device_name (Description)"
                let mut clean_name = name
                    .trim_start_matches("🔊 ")
                    .to_string();

                // Find the last '(' to remove description
                if let Some(idx) = clean_name.rfind(" (") {
                    clean_name = clean_name[..idx].trim().to_string();
                }

                info!("Looking for input device: '{}' (cleaned: '{}')", name, clean_name);

                // Try to find device by exact name match first
                if let Some(device) = host.input_devices()?.find(|d| {
                    d.name()
                        .map(|n| n == clean_name || n == *name)
                        .unwrap_or(false)
                }) {
                    device
                } else {
                    // If not found, it might be an ALSA device name
                    // Try to open it by name string
                    info!("Device '{}' not found in CPAL list, trying as ALSA device name", clean_name);

                    // For ALSA device names like "aaeq_monitor" or "pulse", we need to use
                    // cpal's device_from_name functionality
                    // Since CPAL doesn't expose this directly, we'll try listing all devices
                    // and checking if any match
                    host.input_devices()?
                        .find(|d| {
                            if let Ok(d_name) = d.name() {
                                // Check if device name contains our target
                                d_name.contains(&clean_name)
                                    || clean_name.contains(&d_name)
                                    || d_name == "pulse"  // PulseAudio default
                                    || d_name == "default"  // System default
                            } else {
                                false
                            }
                        })
                        .ok_or_else(|| {
                            anyhow!(
                                "Input device '{}' not found. Make sure ALSA can access it. \
                                 Try running 'arecord -L' to see available devices.",
                                clean_name
                            )
                        })?
                }
            }
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow!("No default input device available"))?
        };

        let device_name = device.name()?;
        info!("Using input device: {}", device_name);

        // Get supported config
        let supported_configs = device.supported_input_configs()?;

        // Log all supported sample rates for diagnostic purposes
        // Note: We need to collect into Vec first since SupportedInputConfigs iterator consumes itself
        let configs_vec: Vec<_> = supported_configs.collect();
        let mut supported_rates = Vec::new();
        for config in &configs_vec {
            supported_rates.push(format!(
                "{}-{} Hz ({:?}, {} ch)",
                config.min_sample_rate().0,
                config.max_sample_rate().0,
                config.sample_format(),
                config.channels()
            ));
        }
        info!("Input device '{}' supported configs: {:?}", device_name, supported_rates);

        let supported_config = configs_vec.into_iter()
            .filter(|c| c.channels() == cfg.channels)
            .find(|c| {
                c.min_sample_rate().0 <= cfg.sample_rate
                    && c.max_sample_rate().0 >= cfg.sample_rate
            })
            .ok_or_else(|| anyhow!("No supported config found for {} Hz", cfg.sample_rate))?;

        let config = supported_config.with_sample_rate(cpal::SampleRate(cfg.sample_rate));
        let sample_format = config.sample_format();

        info!(
            "Input capture starting: {} channels, {} Hz (requested), {:?} format",
            config.channels(),
            cfg.sample_rate,
            sample_format
        );

        // Check if the device's natural sample rate matches what we're requesting
        if supported_config.min_sample_rate().0 != supported_config.max_sample_rate().0 {
            info!(
                "Device supports variable sample rate {}-{} Hz, will use {} Hz",
                supported_config.min_sample_rate().0,
                supported_config.max_sample_rate().0,
                cfg.sample_rate
            );
        } else if supported_config.min_sample_rate().0 != cfg.sample_rate {
            warn!(
                "Device native rate is {} Hz but requesting {} Hz - CPAL/ALSA will resample, which may introduce artifacts!",
                supported_config.min_sample_rate().0,
                cfg.sample_rate
            );
        } else {
            info!("Using device native sample rate: {} Hz (no resampling needed)", cfg.sample_rate);
        }

        // Create stop channel
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
        let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Spawn a thread to wait for stop signal
        let _stop_handle = std::thread::spawn(move || {
            let _result = stop_rx.blocking_recv();
            stop_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        // Build the input stream
        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let tx = tx.clone();
                let stop_flag = stop_flag.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }

                        // Convert f32 to f64
                        let samples: Vec<f64> = data.iter().map(|&s| s as f64).collect();

                        // Send to channel (non-blocking)
                        if let Ok(tx_lock) = tx.lock() {
                            if let Some(ref sender) = *tx_lock {
                                let _ = sender.try_send(samples);
                            }
                        }
                    },
                    move |err| {
                        error!("Input stream error: {}", err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let tx = tx.clone();
                let stop_flag = stop_flag.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }

                        // Convert i16 to f64 (-1.0 to 1.0)
                        let samples: Vec<f64> = data.iter().map(|&s| s as f64 / 32768.0).collect();

                        // Send to channel (non-blocking)
                        if let Ok(tx_lock) = tx.lock() {
                            if let Some(ref sender) = *tx_lock {
                                let _ = sender.try_send(samples);
                            }
                        }
                    },
                    move |err| {
                        error!("Input stream error: {}", err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let tx = tx.clone();
                let stop_flag = stop_flag.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }

                        // Convert u16 to f64 (-1.0 to 1.0)
                        let samples: Vec<f64> = data
                            .iter()
                            .map(|&s| (s as f64 - 32768.0) / 32768.0)
                            .collect();

                        // Send to channel (non-blocking)
                        if let Ok(tx_lock) = tx.lock() {
                            if let Some(ref sender) = *tx_lock {
                                let _ = sender.try_send(samples);
                            }
                        }
                    },
                    move |err| {
                        error!("Input stream error: {}", err);
                    },
                    None,
                )?
            }
            cpal::SampleFormat::U8 => {
                let tx = tx.clone();
                let stop_flag = stop_flag.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }

                        // Convert u8 to f64 (-1.0 to 1.0)
                        let samples: Vec<f64> = data
                            .iter()
                            .map(|&s| (s as f64 - 128.0) / 128.0)
                            .collect();

                        // Send to channel (non-blocking)
                        if let Ok(tx_lock) = tx.lock() {
                            if let Some(ref sender) = *tx_lock {
                                let _ = sender.try_send(samples);
                            }
                        }
                    },
                    move |err| {
                        error!("Input stream error: {}", err);
                    },
                    None,
                )?
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported sample format for input: {:?}",
                    sample_format
                ))
            }
        };

        // Start the stream
        use cpal::traits::StreamTrait;
        stream.play()?;
        info!("Input stream started successfully");

        // Wrap the stream in a Send-able holder
        let stream_holder = StreamHolder(stream);

        // Spawn a dedicated thread to hold the stream handle
        // The stream must be kept alive for capture to continue
        let (thread_stop_tx, thread_stop_rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _stream = stream_holder; // Keep stream alive
            // Block until stop signal received
            let _ = thread_stop_rx.recv();
            info!("Input stream thread exiting, stream will be dropped");
        });

        // Wrap the stop_tx to signal both the callback and the thread
        let (final_stop_tx, mut final_stop_rx) = mpsc::channel::<()>(1);
        tokio::spawn(async move {
            let _ = final_stop_rx.recv().await;
            let _ = stop_tx.send(()).await; // Signal stop flag
            let _ = thread_stop_tx.send(()); // Signal thread to exit
        });

        Ok(final_stop_tx)
    }

    // Windows-specific: List WASAPI loopback devices (system audio capture)
    #[cfg(target_os = "windows")]
    fn list_windows_loopback_devices() -> Result<Vec<String>> {
        let mut loopback_devices = Vec::new();

        // Initialize COM
        match initialize_mta() {
            Ok(_) => {
                info!("WASAPI: COM initialized successfully");
            }
            Err(e) => {
                info!("WASAPI: COM initialization returned: {:?} (might already be initialized)", e);
            }
        }

        // Get render (output) devices - these can be used for loopback capture
        let render_collection = match DeviceCollection::new(&Direction::Render) {
            Ok(c) => c,
            Err(e) => {
                info!("WASAPI: Failed to get render device collection: {:?}", e);
                return Ok(loopback_devices);
            }
        };

        let device_count = match render_collection.get_nbr_devices() {
            Ok(count) => count,
            Err(e) => {
                info!("WASAPI: Failed to get device count: {:?}", e);
                return Ok(loopback_devices);
            }
        };

        info!("WASAPI: Found {} render device(s) for loopback", device_count);

        // Enumerate each render device
        for i in 0..device_count {
            if let Ok(device) = render_collection.get_device(i) {
                if let Ok(name) = device.get_friendlyname() {
                    // Add device with loopback indicator
                    loopback_devices.push(format!("🔊 {} (Loopback)", name));
                    info!("WASAPI: Added loopback device: {}", name);
                }
            }
        }

        Ok(loopback_devices)
    }

    /// Windows-specific: Start WASAPI loopback capture
    #[cfg(target_os = "windows")]
    fn start_wasapi_loopback_capture(
        device_name: &str,
        cfg: OutputConfig,
        tx: mpsc::Sender<Vec<f64>>,
    ) -> Result<mpsc::Sender<()>> {
        use std::sync::atomic::{AtomicBool, Ordering};

        // Extract clean device name (remove emoji and "(Loopback)")
        let clean_name = device_name
            .trim_start_matches("🔊 ")
            .trim_end_matches(" (Loopback)")
            .to_string();

        info!("Starting WASAPI loopback capture for: {}", clean_name);

        // Initialize COM
        match initialize_mta() {
            Ok(_) => info!("WASAPI: COM initialized for capture"),
            Err(e) => info!("WASAPI: COM initialization returned: {:?}", e),
        }

        // Get render devices
        let render_collection = DeviceCollection::new(&Direction::Render)
            .map_err(|e| anyhow!("Failed to get render device collection: {:?}", e))?;

        let device_count = render_collection
            .get_nbr_devices()
            .map_err(|e| anyhow!("Failed to get device count: {:?}", e))?;

        // Find matching device
        let mut target_device = None;
        for i in 0..device_count {
            if let Ok(device) = render_collection.get_device(i) {
                if let Ok(name) = device.get_friendlyname() {
                    if name == clean_name {
                        target_device = Some(device);
                        break;
                    }
                }
            }
        }

        let device = target_device
            .ok_or_else(|| anyhow!("WASAPI device '{}' not found", clean_name))?;

        // Initialize audio client in loopback mode
        let audio_client = device
            .get_iaudioclient()
            .map_err(|e| anyhow!("Failed to get audio client: {:?}", e))?;

        // Get device format
        let waveformat = audio_client
            .get_mixformat()
            .map_err(|e| anyhow!("Failed to get device format: {:?}", e))?;

        info!(
            "WASAPI device format: {} Hz, {} channels, {} bits",
            waveformat.get_samplespersec(),
            waveformat.get_nchannels(),
            waveformat.get_bitspersample()
        );

        // Initialize in loopback mode
        let blockalign = waveformat.get_blockalign();
        let (def_time, min_time) = audio_client
            .get_periods()
            .map_err(|e| anyhow!("Failed to get periods: {:?}", e))?;

        audio_client
            .initialize_client(
                &waveformat,
                def_time,
                &Direction::Capture,
                &ShareMode::Shared,
                true, // loopback mode
            )
            .map_err(|e| anyhow!("Failed to initialize audio client: {:?}", e))?;

        let buffer_frame_count = audio_client
            .get_bufferframecount()
            .map_err(|e| anyhow!("Failed to get buffer frame count: {:?}", e))?;

        let capture_client = audio_client
            .get_audiocaptureclient()
            .map_err(|e| anyhow!("Failed to get capture client: {:?}", e))?;

        let sample_rate = waveformat.get_samplespersec();
        let channels = waveformat.get_nchannels() as u32;

        info!(
            "WASAPI: Initialized loopback capture: {} Hz, {} channels, {} frames buffer",
            sample_rate, channels, buffer_frame_count
        );

        // Start capture
        audio_client
            .start_stream()
            .map_err(|e| anyhow!("Failed to start stream: {:?}", e))?;

        // Create stop channel
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        let stop_flag = std::sync::Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Spawn capture thread
        std::thread::spawn(move || {
            info!("WASAPI: Capture thread started");

            while !stop_flag.load(Ordering::Relaxed) {
                // Check for stop signal (non-blocking)
                if stop_rx.try_recv().is_ok() {
                    stop_flag.store(true, Ordering::Relaxed);
                    break;
                }

                // Get available frames
                match capture_client.get_next_nbr_frames() {
                    Ok(frames_available) if frames_available > 0 => {
                        // Read buffer
                        match capture_client.read_from_device(frames_available as usize) {
                            Ok(data) => {
                                // Convert to f64 samples
                                let samples: Vec<f64> = match waveformat.get_bitspersample() {
                                    16 => {
                                        // 16-bit PCM
                                        data.chunks_exact(2)
                                            .map(|chunk| {
                                                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                                                sample as f64 / 32768.0
                                            })
                                            .collect()
                                    }
                                    24 => {
                                        // 24-bit PCM
                                        data.chunks_exact(3)
                                            .map(|chunk| {
                                                let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]) >> 8;
                                                sample as f64 / 8388608.0
                                            })
                                            .collect()
                                    }
                                    32 => {
                                        // 32-bit float
                                        data.chunks_exact(4)
                                            .map(|chunk| {
                                                let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                                                sample as f64
                                            })
                                            .collect()
                                    }
                                    _ => {
                                        error!("WASAPI: Unsupported bit depth: {}", waveformat.get_bitspersample());
                                        continue;
                                    }
                                };

                                // Send to channel (non-blocking)
                                let _ = tx.try_send(samples);
                            }
                            Err(e) => {
                                error!("WASAPI: Failed to read from device: {:?}", e);
                            }
                        }
                    }
                    Ok(_) => {
                        // No frames available, sleep briefly
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(e) => {
                        error!("WASAPI: Failed to get frame count: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            }

            // Stop stream
            let _ = audio_client.stop_stream();
            info!("WASAPI: Capture thread exiting");
        });

        Ok(stop_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_input_devices() {
        // This test may fail on systems without audio hardware
        match LocalDacInput::list_devices() {
            Ok(devices) => {
                println!("Found {} input device(s)", devices.len());
                for device in devices {
                    println!("  - {}", device);
                }
            }
            Err(e) => {
                println!("Failed to list input devices (this is OK in CI): {}", e);
            }
        }
    }
}
