/// Example: Test AirPlay streaming
///
/// Usage: cargo run --example test_airplay [device_name]
///
/// This will discover AirPlay devices and stream a test tone to the specified device.
/// If no device name is provided, it will list available devices.

use anyhow::Result;
use stream_server::*;
use std::env;
use std::f64::consts::PI;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== AirPlay Streaming Test ===\n");

    let args: Vec<String> = env::args().collect();

    // Discover devices
    println!("Discovering AirPlay devices (5 second timeout)...");
    let devices = AirPlaySink::discover(5).await?;

    if devices.is_empty() {
        println!("✗ No AirPlay devices found");
        println!("\nMake sure:");
        println!("  1. Your AirPlay device is powered on");
        println!("  2. You're on the same network");
        println!("  3. Firewall allows mDNS (port 5353 UDP)");
        return Ok(());
    }

    println!("✓ Found {} device(s):\n", devices.len());
    for (i, device) in devices.iter().enumerate() {
        println!("{}. {}", i + 1, device.name);
        println!("   Host: {}", device.hostname);
        println!("   Port: {}", device.port);
        if let Some(model) = &device.model {
            println!("   Model: {}", model);
        }
        println!();
    }

    // Select device
    let device = if args.len() > 1 {
        let search_name = &args[1];
        devices.iter()
            .find(|d| d.name.to_lowercase().contains(&search_name.to_lowercase()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Device '{}' not found", search_name))?
    } else {
        println!("Usage: {} <device_name>", args[0]);
        println!("\nExample: {} \"Living Room\"", args[0]);
        return Ok(());
    };

    println!("Selected device: {}\n", device.name);

    // Create AirPlay sink
    let mut sink = AirPlaySink::new();
    sink.set_device(device);

    // Configure output
    let config = OutputConfig {
        sample_rate: 44100, // AirPlay typically uses 44.1kHz
        channels: 2,
        format: SampleFormat::S16LE,
        buffer_ms: 200,
        exclusive: false,
    };

    println!("Connecting to AirPlay device...");
    println!("  Sample rate: {} Hz", config.sample_rate);
    println!("  Channels: {}", config.channels);
    println!("  Format: {:?}", config.format);
    println!();

    match sink.open(config.clone()).await {
        Ok(_) => println!("✓ Connected successfully\n"),
        Err(e) => {
            eprintln!("✗ Connection failed: {}", e);
            eprintln!("\nNote: Full AirPlay support requires:");
            eprintln!("  - Device authentication support");
            eprintln!("  - Proper ALAC encoding");
            eprintln!("  - Compatible AirPlay receiver");
            return Err(e);
        }
    }

    println!("Streaming test tone for 10 seconds...\n");

    // Stream test tone
    let duration_secs = 10.0;
    let chunk_duration = 0.1; // 100ms chunks
    let sample_rate = config.sample_rate as f64;
    let frequency = 440.0; // A4 note

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
    }

    println!("\n\nDraining audio buffer...");
    sink.drain().await?;

    println!("✓ Stream completed");
    println!("\nLatency: {}ms (typical for AirPlay)", sink.latency_ms());

    sink.close().await?;
    println!("✓ Connection closed");

    Ok(())
}
