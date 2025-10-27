/// Example: Discover all audio devices
///
/// Usage: cargo run --example discover_devices
///
/// This will discover local DACs and AirPlay devices on your network
use anyhow::Result;
use stream_server::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Audio Device Discovery ===\n");

    // Discover local DACs
    println!("[1] Local Audio Devices (DACs)");
    println!("──────────────────────────────────────────────────");
    match LocalDacSink::list_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("No local audio devices found");
            } else {
                for (i, device) in devices.iter().enumerate() {
                    println!("  {}. {}", i + 1, device);
                }
            }
        }
        Err(e) => {
            eprintln!("Error listing local devices: {}", e);
        }
    }

    println!("\n[2] AirPlay Devices (Network)");
    println!("──────────────────────────────────────────────────");
    println!("Searching for 10 seconds...\n");

    match AirPlaySink::discover(10).await {
        Ok(devices) => {
            if devices.is_empty() {
                println!("No AirPlay devices found");
                println!("\nTroubleshooting:");
                println!("  • Ensure AirPlay devices are powered on");
                println!("  • Check you're on the same network");
                println!("  • Verify firewall allows mDNS (UDP port 5353)");
            } else {
                println!("Found {} AirPlay device(s):\n", devices.len());
                for (i, device) in devices.iter().enumerate() {
                    println!("{}. {}", i + 1, device.name);
                    println!("   Hostname: {}", device.hostname);
                    println!("   Port: {}", device.port);

                    if !device.addresses.is_empty() {
                        println!("   IP Addresses:");
                        for addr in &device.addresses {
                            println!("      - {}", addr);
                        }
                    }

                    if let Some(model) = &device.model {
                        println!("   Model: {}", model);
                    }

                    if let Some(features) = &device.features {
                        println!("   Features: {}", features);
                    }

                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("Error discovering AirPlay devices: {}", e);
        }
    }

    println!("\n[3] Usage Examples");
    println!("──────────────────────────────────────────────────");
    println!("Test local DAC:");
    println!("  cargo run --example test_local_dac");
    println!();
    println!("Test DLNA streaming:");
    println!("  cargo run --example test_dlna");
    println!();
    println!("Test AirPlay (if devices found):");
    println!("  cargo run --example test_airplay \"Device Name\"");
    println!();

    Ok(())
}
