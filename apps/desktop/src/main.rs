#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use aaeq_ui_egui::AaeqApp;
use anyhow::Result;
use clap::Parser;
use single_instance::SingleInstance;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// AAEQ - Adaptive Audio Equalizer
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Show console window for logs (Windows only)
    #[arg(long)]
    console: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    #[cfg_attr(not(target_os = "windows"), allow(unused_variables))]
    let args = Args::parse();

    // On Windows, allocate console if --console flag is present
    #[cfg(target_os = "windows")]
    if args.console {
        use windows::Win32::System::Console::AllocConsole;
        unsafe {
            let _ = AllocConsole();
        }
    }

    // Get database path early (before logging, so we know where to put the log file)
    let db_path = get_db_path()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Initialize database (we need this to check the debug logging setting)
    let pool = aaeq_persistence::init_db(&db_path).await?;

    // Check if debug logging to file is enabled
    let settings_repo = aaeq_persistence::AppSettingsRepository::new(pool.clone());
    let enable_debug_logging = settings_repo.get_enable_debug_logging().await.unwrap_or(false);

    // Initialize logging with optional file output
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into());

    if enable_debug_logging {
        // Set up logging with both console and file output
        let log_dir = db_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Failed to get database parent directory"))?;

        let file_appender = tracing_appender::rolling::daily(log_dir, "debug.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        // Initialize with both console and file layers
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false) // Disable ANSI colors in file output
            )
            .init();

        // Leak the guard to keep file writer alive for the application lifetime
        std::mem::forget(_guard);

        eprintln!("Debug logging enabled. Log file: {}/debug.log", log_dir.display());
    } else {
        // Console-only logging
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    tracing::info!("Starting AAEQ - Adaptive Audio Equalizer");
    tracing::info!("Database path: {}", db_path.display());

    // Ensure only one instance is running
    let _instance = SingleInstance::new("aaeq-app-instance")?;
    if !_instance.is_single() {
        eprintln!("Another instance of AAEQ is already running.");
        tracing::error!("Another instance of AAEQ is already running");
        std::process::exit(1);
    }
    tracing::info!("Single instance check passed");

    // Keep the instance lock in a static location so it persists for the app lifetime
    // We leak it intentionally to keep the lock held until process exit
    let _instance_guard = Box::leak(Box::new(_instance));

    // Create app
    let mut app = AaeqApp::new(pool, db_path.clone());

    // Initialize app (load mappings, etc.)
    app.initialize().await?;

    tracing::info!("Launching UI...");

    // Initialize GTK on Linux (required for tray-icon)
    #[cfg(target_os = "linux")]
    {
        tracing::info!("Initializing GTK...");
        gtk::init().expect("Failed to initialize GTK");
        tracing::info!("GTK initialized successfully");
    }

    // Create tray icon
    tracing::info!("Creating tray icon menu...");
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

    tracing::info!("Loading tray icon image...");
    let icon = load_icon();
    tracing::info!("Building tray icon...");
    let _tray_icon = match TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("AAEQ - Adaptive Audio Equalizer")
        .with_icon(icon)
        .build() {
            Ok(icon) => {
                tracing::info!("Tray icon created successfully");
                icon
            }
            Err(e) => {
                tracing::warn!("Failed to create tray icon: {}", e);
                #[cfg(target_os = "linux")]
                {
                    // Check if we're on XFCE
                    if std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase().contains("xfce") {
                        tracing::warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        tracing::warn!("XFCE detected: Tray icon requires panel plugin!");
                        tracing::warn!("Add 'Indicator Plugin' or 'Status Notifier Plugin':");
                        tracing::warn!("  1. Right-click XFCE panel → Panel → Add New Items");
                        tracing::warn!("  2. Select 'Indicator Plugin' (recommended)");
                        tracing::warn!("  3. Restart AAEQ to see the tray icon");
                        tracing::warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    } else {
                        tracing::warn!("Make sure libappindicator3 is installed:");
                        tracing::warn!("  Ubuntu/Debian: sudo apt install libappindicator3-1");
                        tracing::warn!("  Arch: sudo pacman -S libappindicator-gtk3");
                    }
                }
                return Err(e.into());
            }
        };

    // Track window visibility
    let window_visible = Arc::new(Mutex::new(true));
    let window_visible_clone = window_visible.clone();

    // Handle tray icon events
    let tray_channel = MenuEvent::receiver();

    // Load window icon
    let window_icon = load_window_icon();

    // Run UI
    // Note: Window behavior across platforms:
    // - Minimize button: window minimizes to taskbar/dock (normal system behavior)
    // - Close button (X): window hides to tray (handled in app.rs close_requested)
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("AAEQ - Adaptive Audio Equalizer")
            .with_icon(window_icon)
            .with_taskbar(true)          // Show in taskbar/dock on all platforms
            .with_decorations(true)      // Enable window controls (minimize, maximize, close)
            .with_resizable(true)        // Allow window resizing
            .with_maximized(false),      // Start not maximized
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

                                // On Windows, we need to be more aggressive to restore the window
                                #[cfg(target_os = "windows")]
                                {
                                    // First, ensure window is not minimized
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));

                                    // Make sure window is visible
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

                                    // Bring window to front
                                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));

                                    // Give Windows a moment to process the commands
                                    std::thread::sleep(std::time::Duration::from_millis(10));

                                    // Remove always-on-top so window behaves normally
                                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));

                                    // Focus the window
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

                                    // Request repaint to ensure UI updates
                                    ctx.request_repaint();
                                }

                                // On other platforms, simpler approach works
                                #[cfg(not(target_os = "windows"))]
                                {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                                }
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

/// Load icon from embedded PNG file
fn load_icon() -> tray_icon::Icon {
    // Embed the icon at compile time
    let icon_bytes = include_bytes!("../../../aaeq-icon.png");

    // Load and decode the PNG
    let img = image::load_from_memory(icon_bytes)
        .expect("Failed to load icon image");

    // Resize to appropriate size for tray icon (32x32 for most systems)
    let img = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);

    // Convert to RGBA
    let rgba_img = img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    let rgba_data = rgba_img.into_raw();

    tray_icon::Icon::from_rgba(rgba_data, width, height)
        .expect("Failed to create tray icon")
}

/// Load application window icon
fn load_window_icon() -> egui::IconData {
    // Embed the icon at compile time
    let icon_bytes = include_bytes!("../../../aaeq-icon.png");

    // Load and decode the PNG
    let img = image::load_from_memory(icon_bytes)
        .expect("Failed to load window icon");

    // Convert to RGBA
    let rgba_img = img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    let rgba_data = rgba_img.into_raw();

    egui::IconData {
        rgba: rgba_data,
        width,
        height,
    }
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
