/// Example: Run the Control API server
///
/// Usage: cargo run -p stream-server --example control_api_server
///
/// This starts the HTTP control API server that allows controlling
/// the stream server via REST endpoints.
///
/// Once running, you can test with:
/// curl http://localhost:8080/v1/health
/// curl http://localhost:8080/v1/outputs
/// curl http://localhost:8080/v1/capabilities
use anyhow::Result;
use std::sync::Arc;
use stream_server::*;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== AAEQ Stream Server Control API ===\n");

    // Create output manager and register sinks
    let mut manager = OutputManager::new();

    println!("Registering output sinks...");
    manager.register_sink(Box::new(LocalDacSink::new(None)));
    manager.register_sink(Box::new(DlnaSink::new(
        "AAEQ DLNA".to_string(),
        "0.0.0.0:8090".parse()?,
    )));
    manager.register_sink(Box::new(AirPlaySink::new()));

    println!("✓ Registered {} sinks\n", manager.sink_count());

    // Wrap manager in Arc<RwLock> for sharing
    let manager = Arc::new(RwLock::new(manager));

    // Create and start control server
    let bind_addr = "127.0.0.1:8080".parse()?;
    let mut server = ControlServer::new(bind_addr, manager.clone());

    server.start().await?;

    println!("✓ Control API server started on {}\n", bind_addr);
    println!("API Endpoints:");
    println!("──────────────────────────────────────────────────");
    println!("  GET  http://localhost:8080/v1/health");
    println!("  GET  http://localhost:8080/v1/outputs");
    println!("  POST http://localhost:8080/v1/outputs/select");
    println!("  POST http://localhost:8080/v1/outputs/start");
    println!("  POST http://localhost:8080/v1/outputs/stop");
    println!("  GET  http://localhost:8080/v1/outputs/metrics");
    println!("  GET  http://localhost:8080/v1/route");
    println!("  POST http://localhost:8080/v1/route");
    println!("  GET  http://localhost:8080/v1/capabilities");
    println!();

    println!("Example commands:");
    println!("──────────────────────────────────────────────────");
    println!("# Check health");
    println!("curl http://localhost:8080/v1/health");
    println!();
    println!("# List outputs");
    println!("curl http://localhost:8080/v1/outputs");
    println!();
    println!("# Get capabilities");
    println!("curl http://localhost:8080/v1/capabilities");
    println!();
    println!("# Select local DAC output");
    println!(r#"curl -X POST http://localhost:8080/v1/outputs/select \"#);
    println!(r#"  -H "Content-Type: application/json" \"#);
    println!(r#"  -d '{{"name":"local_dac","config":{{"sample_rate":48000,"channels":2,"format":"F32","buffer_ms":150,"exclusive":false}}}}'"#);
    println!();
    println!("# Get metrics");
    println!("curl http://localhost:8080/v1/outputs/metrics");
    println!();
    println!("# Stop output");
    println!("curl -X POST http://localhost:8080/v1/outputs/stop");
    println!();

    println!("Server running. Press Ctrl+C to stop...\n");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    println!("\n\nShutting down...");
    server.stop().await;
    println!("✓ Server stopped");

    Ok(())
}
