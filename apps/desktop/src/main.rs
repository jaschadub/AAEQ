#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod single_instance;

use aaeq_ui_egui::AaeqApp;
use anyhow::Result;
use clap::Parser;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use single_instance::SingleInstanceGuard;
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

    // Load hotkey settings from database
    let hotkey_enabled = settings_repo.get_hotkey_enabled().await.unwrap_or(true);
    let hotkey_modifiers = settings_repo.get_hotkey_modifiers().await.unwrap_or_else(|_| "Ctrl+Shift".to_string());
    let hotkey_key = settings_repo.get_hotkey_key().await.unwrap_or_else(|_| "A".to_string());

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
    // The guard will automatically clean up the lock file when dropped (on normal exit)
    // or be cleaned up on next start if the process crashed
    let _instance_guard = match SingleInstanceGuard::acquire("aaeq-app-instance") {
        Ok(guard) => {
            tracing::info!("Single instance check passed");
            guard
        }
        Err(e) => {
            eprintln!("Another instance of AAEQ is already running.");
            tracing::error!("Another instance of AAEQ is already running: {}", e);
            std::process::exit(1);
        }
    };

    // Keep the instance lock alive for the entire application lifetime
    // We leak it intentionally to keep the lock held until process exit
    let _instance_guard = Box::leak(Box::new(_instance_guard));

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

    // Set up global hotkey to show window (if enabled)
    let hotkey_manager = GlobalHotKeyManager::new().expect("Failed to create hotkey manager");
    let mut show_hotkey_id: Option<u32> = None;

    if hotkey_enabled {
        // Parse modifiers and key from settings
        let modifiers = parse_modifiers(&hotkey_modifiers);
        let key_code = parse_key_code(&hotkey_key);

        if let (Some(mods), Some(code)) = (modifiers, key_code) {
            let show_hotkey = HotKey::new(Some(mods), code);
            tracing::info!("Registering global hotkey: {} + {}...", hotkey_modifiers, hotkey_key);

            if let Err(e) = hotkey_manager.register(show_hotkey) {
                tracing::warn!("Failed to register global hotkey: {}", e);
                tracing::warn!("You can still show the window using the system tray icon");
            } else {
                show_hotkey_id = Some(show_hotkey.id());
                tracing::info!("Global hotkey registered successfully: {} + {}", hotkey_modifiers, hotkey_key);
            }
        } else {
            tracing::warn!("Invalid hotkey configuration: {} + {}", hotkey_modifiers, hotkey_key);
            tracing::warn!("Global hotkey disabled. You can still show the window using the system tray icon");
        }
    } else {
        tracing::info!("Global hotkey disabled in settings");
    }

    // Keep the hotkey manager alive for the lifetime of the application
    let _hotkey_manager = Box::leak(Box::new(hotkey_manager));

    let hotkey_receiver = GlobalHotKeyEvent::receiver();

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
            // Handle tray events and hotkey events in the app
            let ctx = cc.egui_ctx.clone();
            let ctx_hotkey = ctx.clone();
            let window_visible_hotkey = window_visible_clone.clone();

            // Spawn thread for tray icon events
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

            // Spawn thread for global hotkey events
            std::thread::spawn(move || {
                loop {
                    if let Ok(event) = hotkey_receiver.try_recv() {
                        if let Some(expected_id) = show_hotkey_id {
                            if event.id == expected_id {
                                tracing::info!("Global hotkey pressed - showing window");
                                *window_visible_hotkey.lock().unwrap() = true;

                            // On Windows, we need to be more aggressive to restore the window
                            #[cfg(target_os = "windows")]
                            {
                                ctx_hotkey.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                                ctx_hotkey.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                ctx_hotkey.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
                                std::thread::sleep(std::time::Duration::from_millis(10));
                                ctx_hotkey.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                                ctx_hotkey.send_viewport_cmd(egui::ViewportCommand::Focus);
                                ctx_hotkey.request_repaint();
                            }

                            // On other platforms, simpler approach works
                            #[cfg(not(target_os = "windows"))]
                            {
                                ctx_hotkey.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                ctx_hotkey.send_viewport_cmd(egui::ViewportCommand::Focus);
                            }
                            }
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

/// Parse modifiers string into global-hotkey Modifiers
fn parse_modifiers(modifiers_str: &str) -> Option<Modifiers> {
    match modifiers_str {
        "Ctrl" => Some(Modifiers::CONTROL),
        "Alt" => Some(Modifiers::ALT),
        "Shift" => Some(Modifiers::SHIFT),
        "Ctrl+Shift" => Some(Modifiers::CONTROL | Modifiers::SHIFT),
        "Ctrl+Alt" => Some(Modifiers::CONTROL | Modifiers::ALT),
        "Alt+Shift" => Some(Modifiers::ALT | Modifiers::SHIFT),
        _ => None,
    }
}

/// Parse key string into global-hotkey Code
fn parse_key_code(key_str: &str) -> Option<Code> {
    match key_str {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "Space" => Some(Code::Space),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        _ => None,
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
