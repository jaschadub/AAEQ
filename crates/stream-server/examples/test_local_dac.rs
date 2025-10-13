/// Example: Test local DAC output with sine wave
///
/// Usage: cargo run --example test_local_dac
///
/// This will play a 1kHz sine wave through your default audio device

use anyhow::Result;
use stream_server::*;
use std::f64::consts::PI;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Local DAC Test ===\n");

    // List available devices
    println!("Available audio devices:");
    match LocalDacSink::list_devices() {
        Ok(devices) => {
            for (i, device) in devices.iter().enumerate() {
                println!("  {}. {}", i + 1, device);
            }
            println!();
        }
        Err(e) => {
            eprintln!("Warning: Could not list devices: {}", e);
        }
    }

    // Create sink with default device
    let mut sink = LocalDacSink::new(None);

    // Configure output
    let config = OutputConfig {
        sample_rate: 48000,
        channels: 2,
        format: SampleFormat::F32,
        buffer_ms: 150,
        exclusive: false,
    };

    println!("Opening audio device...");
    println!("  Sample rate: {} Hz", config.sample_rate);
    println!("  Channels: {}", config.channels);
    println!("  Format: {:?}", config.format);
    println!();

    sink.open(config.clone()).await?;
    println!("✓ Audio device opened successfully\n");

    // Generate test tones
    let duration_secs = 3.0;
    let chunk_duration = 0.1; // 100ms chunks
    let sample_rate = config.sample_rate as f64;
    let frequency = 1000.0; // 1kHz

    println!("Playing {} second test tone at {}Hz...", duration_secs, frequency);
    println!("(You should hear a pure tone)\n");

    let total_chunks = (duration_secs / chunk_duration) as usize;

    for chunk in 0..total_chunks {
        let chunk_samples = (sample_rate * chunk_duration) as usize;
        let mut samples = Vec::with_capacity(chunk_samples * 2);

        let start_sample = chunk * chunk_samples;

        for i in 0..chunk_samples {
            let t = (start_sample + i) as f64 / sample_rate;
            let sample = (2.0 * PI * frequency * t).sin() * 0.3; // 30% volume

            samples.push(sample); // Left channel
            samples.push(sample); // Right channel
        }

        let block = AudioBlock::new(&samples, config.sample_rate, 2);
        sink.write(block).await?;

        // Show progress
        let progress = ((chunk + 1) as f64 / total_chunks as f64 * 100.0) as u32;
        print!("\rProgress: {}%", progress);
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    }

    println!("\n\nDraining audio buffer...");
    sink.drain().await?;

    println!("✓ Test completed successfully");

    // Show latency
    println!("\nLatency: {}ms", sink.latency_ms());

    sink.close().await?;
    println!("✓ Audio device closed");

    Ok(())
}
