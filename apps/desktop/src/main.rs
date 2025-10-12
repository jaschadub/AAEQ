use aaeq_ui_egui::AaeqApp;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,aaeq=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting AAEQ - Adaptive Audio Equalizer");

    // Get database path
    let db_path = get_db_path()?;
    tracing::info!("Database path: {}", db_path.display());

    // Create parent directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Initialize database
    let pool = aaeq_persistence::init_db(&db_path).await?;

    // Create app
    let mut app = AaeqApp::new(pool, db_path.clone());

    // Initialize app (load mappings, etc.)
    app.initialize().await?;

    tracing::info!("Launching UI...");

    // Create tray icon
    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Show Window", true, None);
    let hide_item = MenuItem::new("Hide Window", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    tray_menu.append(&show_item)?;
    tray_menu.append(&hide_item)?;
    tray_menu.append(&PredefinedMenuItem::separator())?;
    tray_menu.append(&quit_item)?;

    // Store IDs for comparison
    let show_id = show_item.id().clone();
    let hide_id = hide_item.id().clone();
    let quit_id = quit_item.id().clone();

    let icon = load_icon();
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("AAEQ - Adaptive Audio Equalizer")
        .with_icon(icon)
        .build()?;

    // Track window visibility
    let window_visible = Arc::new(Mutex::new(true));
    let window_visible_clone = window_visible.clone();

    // Handle tray icon events
    let tray_channel = MenuEvent::receiver();

    // Run UI
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("AAEQ - Adaptive Audio Equalizer"),
        ..Default::default()
    };

    eframe::run_native(
        "AAEQ",
        native_options,
        Box::new(move |cc| {
            // Handle tray events in the app
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                loop {
                    if let Ok(event) = tray_channel.try_recv() {
                        match event.id {
                            id if id == show_id => {
                                tracing::info!("Show window clicked");
                                *window_visible_clone.lock().unwrap() = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                            }
                            id if id == hide_id => {
                                tracing::info!("Hide window clicked");
                                *window_visible_clone.lock().unwrap() = false;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                            }
                            id if id == quit_id => {
                                tracing::info!("Quit clicked from tray");
                                // Force quit by setting visible and then closing
                                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                std::thread::sleep(std::time::Duration::from_millis(50));
                                std::process::exit(0);
                            }
                            _ => {}
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            });

            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run app: {}", e))?;

    Ok(())
}

/// Load embedded icon for system tray
fn load_icon() -> tray_icon::Icon {
    // Create a simple 32x32 RGBA icon with EQ bars pattern
    let size = 32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Draw a simple EQ bars icon
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;

            // Create 5 bars with different heights
            let bar_width = size / 6;
            let bar_idx = x / bar_width;
            let heights = [20, 28, 24, 30, 22]; // Different bar heights

            if bar_idx < 5 {
                let bar_height = heights[bar_idx as usize];
                let bar_start = size - bar_height;

                // Draw bar
                if y >= bar_start && x % bar_width < bar_width - 1 {
                    // Gradient from blue to cyan
                    rgba[idx] = 50;      // R
                    rgba[idx + 1] = 150 + ((y - bar_start) * 105 / bar_height) as u8;  // G
                    rgba[idx + 2] = 255; // B
                    rgba[idx + 3] = 255; // A
                } else {
                    rgba[idx + 3] = 0; // Transparent
                }
            } else {
                rgba[idx + 3] = 0; // Transparent
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, size, size)
        .expect("Failed to create icon")
}

/// Get the database path (platform-specific)
fn get_db_path() -> Result<PathBuf> {
    let config_dir = if cfg!(target_os = "windows") {
        dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?
            .join("AAEQ")
    } else if cfg!(target_os = "macos") {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?
            .join("Library")
            .join("Application Support")
            .join("AAEQ")
    } else {
        // Linux
        dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?
            .join("aaeq")
    };

    Ok(config_dir.join("aaeq.db"))
}
