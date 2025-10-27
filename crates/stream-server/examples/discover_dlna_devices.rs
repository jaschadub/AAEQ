/// Example: Discover DLNA/UPnP MediaRenderer devices
///
/// Usage: cargo run -p stream-server --example discover_dlna_devices
///
/// This will discover DLNA MediaRenderer devices on your network
use anyhow::Result;
use stream_server::sinks::dlna::discover_devices;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== DLNA/UPnP Device Discovery ===\n");

    println!("Searching for DLNA MediaRenderer devices (15 second timeout)...");
    println!("Make sure your network devices are powered on and connected.\n");

    let devices = discover_devices(15).await?;

    if devices.is_empty() {
        println!("✗ No DLNA devices found\n");
        println!("Troubleshooting:");
        println!("  1. Ensure your DLNA devices are powered on");
        println!("  2. Check you're on the same network");
        println!("  3. Verify firewall allows multicast (UDP 239.255.255.250:1900)");
        println!("  4. Some devices may not support MediaRenderer");
        return Ok(());
    }

    println!("✓ Found {} DLNA device(s):\n", devices.len());

    for (i, device) in devices.iter().enumerate() {
        println!("{}. {}", i + 1, device.name);
        println!("   UUID: {}", device.uuid);

        if let Some(ip) = device.ip {
            println!("   IP: {}", ip);
        }

        if let Some(manufacturer) = &device.manufacturer {
            println!("   Manufacturer: {}", manufacturer);
        }

        if let Some(model) = &device.model {
            println!("   Model: {}", model);
        }

        println!("   Location: {}", device.location);

        if !device.services.is_empty() {
            println!("   Services:");
            for service in &device.services {
                println!("      - {}", service.service_type);
            }
        }

        println!();
    }

    println!("Usage Examples:");
    println!("────────────────────────────────────────────────────────────────");

    println!("\nTest DLNA streaming (pull mode - manual setup):");
    println!("  cargo run -p stream-server --example test_dlna");
    println!();

    if !devices.is_empty() {
        let first_device_name = &devices[0].name;
        println!("Test DLNA streaming (push mode - automatic setup):");
        println!(
            "  cargo run -p stream-server --example test_dlna_push \"{}\"",
            first_device_name
        );
    }

    println!();

    Ok(())
}
