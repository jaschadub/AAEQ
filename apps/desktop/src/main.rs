use aaeq_ui_egui::AaeqApp;
use anyhow::Result;
use std::path::PathBuf;
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
    let mut app = AaeqApp::new(pool);

    // Initialize app (load mappings, etc.)
    app.initialize().await?;

    tracing::info!("Launching UI...");

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
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run app: {}", e))?;

    Ok(())
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
