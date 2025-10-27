/// Example: Test DLNA/UPnP streaming
///
/// Usage: cargo run --example test_dlna
///
/// This will start a DLNA server and stream a test tone.
/// Access the stream at: http://localhost:8090/stream.wav
use anyhow::Result;
use stream_server::*;
use std::f64::consts::PI;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== DLNA/UPnP Streaming Test ===\n");

    // Create DLNA sink
    let bind_addr = "0.0.0.0:8090".parse()?;
    let mut sink = DlnaSink::new("AAEQ Test Server".to_string(), bind_addr);

    // Configure output
    let config = OutputConfig {
        sample_rate: 48000,
        channels: 2,
        format: SampleFormat::S16LE,
        buffer_ms: 200,
        exclusive: false,
    };

    println!("Starting DLNA server...");
    println!("  Bind address: {}", bind_addr);
    println!("  Sample rate: {} Hz", config.sample_rate);
    println!("  Channels: {}", config.channels);
    println!("  Format: {:?}", config.format);
    println!();

    sink.open(config.clone()).await?;
    println!("✓ DLNA server started\n");

    if let Some(url) = sink.stream_url() {
        println!("Stream URL: {}", url);
        println!();
        println!("To test:");
        println!("  1. Open the URL in VLC or another media player");
        println!("  2. Or use curl: curl {} > /dev/null", url);
        println!("  3. Or configure your network audio device to pull from this URL");
        println!();
    }

    println!("Streaming test tone for 30 seconds...");
    println!("(Press Ctrl+C to stop)\n");

    // Stream for 30 seconds
    let duration_secs = 30.0;
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

        // Small delay to simulate real-time streaming
        tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
    }

    println!("\n\n✓ Stream completed");
    println!("\nLatency: {}ms", sink.latency_ms());

    println!("\nKeeping server alive for 5 more seconds...");
    println!("(Clients can still connect)");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    sink.close().await?;
    println!("\n✓ DLNA server stopped");

    Ok(())
}
