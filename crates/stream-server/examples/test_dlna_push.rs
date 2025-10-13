/// Example: Test DLNA streaming with push mode (AVTransport control)
///
/// Usage: cargo run -p stream-server --example test_dlna_push [device_name]
///
/// This will discover DLNA devices, connect to the specified device,
/// and automatically start streaming using AVTransport control.

use anyhow::Result;
use std::env;
use std::f64::consts::PI;
use stream_server::sinks::dlna::find_device_by_name;
use stream_server::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== DLNA Push Mode Test ===\n");

    let args: Vec<String> = env::args().collect();

    // Discover devices
    println!("Discovering DLNA devices (10 second timeout)...");

    let device = if args.len() > 1 {
        let device_name = &args[1];
        println!("Looking for device: {}\n", device_name);

        match find_device_by_name(device_name, 10).await? {
            Some(device) => device,
            None => {
                println!("✗ Device '{}' not found", device_name);
                println!("\nRun this to see available devices:");
                println!("  cargo run -p stream-server --example discover_dlna_devices");
                return Ok(());
            }
        }
    } else {
        println!("Usage: {} <device_name>", args[0]);
        println!("\nExample: {} \"Living Room\"", args[0]);
        println!("\nRun this to discover available devices:");
        println!("  cargo run -p stream-server --example discover_dlna_devices");
        return Ok(());
    };

    println!("✓ Found device: {}", device.name);
    if let Some(manufacturer) = &device.manufacturer {
        println!("  Manufacturer: {}", manufacturer);
    }
    if let Some(model) = &device.model {
        println!("  Model: {}", model);
    }
    println!();

    // Check for AVTransport support
    let has_avtransport = device
        .services
        .iter()
        .any(|s| s.service_type.contains("AVTransport"));

    if !has_avtransport {
        println!("✗ Device does not support AVTransport (required for push mode)");
        println!("\nThis device may only support pull mode. Try:");
        println!("  cargo run -p stream-server --example test_dlna");
        return Ok(());
    }

    println!("✓ Device supports AVTransport\n");

    // Create DLNA sink with push mode
    let bind_addr = "0.0.0.0:8091".parse()?;
    let mut sink = DlnaSink::with_device(device, bind_addr, DlnaMode::Push);

    // Configure output
    let config = OutputConfig {
        sample_rate: 48000,
        channels: 2,
        format: SampleFormat::S16LE,
        buffer_ms: 200,
        exclusive: false,
    };

    println!("Starting DLNA server and setting up AVTransport...");
    println!("  Sample rate: {} Hz", config.sample_rate);
    println!("  Channels: {}", config.channels);
    println!("  Format: {:?}", config.format);
    println!("  Mode: Push (AVTransport)");
    println!();

    sink.open(config.clone()).await?;
    println!("✓ DLNA sink opened and playback started on device\n");

    // Stream test tone
    let duration_secs = 15.0;
    let chunk_duration = 0.1; // 100ms chunks
    let sample_rate = config.sample_rate as f64;
    let frequency = 440.0; // A4 note

    println!(
        "Streaming test tone for {} seconds...",
        duration_secs
    );
    println!("(You should hear audio on your DLNA device)\n");

    let total_chunks = (duration_secs / chunk_duration) as usize;

    for chunk in 0..total_chunks {
        let chunk_samples = (sample_rate * chunk_duration) as usize;
        let mut samples = Vec::with_capacity(chunk_samples * 2);

        let start_sample = chunk * chunk_samples;

        for i in 0..chunk_samples {
            let t = (start_sample + i) as f64 / sample_rate;
            let sample = (2.0 * PI * frequency * t).sin() * 0.4;

            samples.push(sample);
            samples.push(sample);
        }

        let block = AudioBlock::new(&samples, config.sample_rate, 2);
        sink.write(block).await?;

        if chunk % 10 == 0 {
            let elapsed = chunk as f64 * chunk_duration;
            print!("\rStreaming: {:.1}s / {:.1}s", elapsed, duration_secs);
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }

        // Small delay to simulate real-time streaming
        tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
    }

    println!("\n\n✓ Stream completed");
    println!("\nLatency: {}ms", sink.latency_ms());

    println!("\nStopping playback...");
    sink.close().await?;
    println!("✓ Playback stopped and DLNA sink closed");

    Ok(())
}
