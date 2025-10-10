use aaeq_core::{resolve_preset, DeviceController, Mapping, RulesIndex, Scope, TrackMeta};
use aaeq_device_wiim::WiimController;
use aaeq_persistence::{AppSettingsRepository, GenreOverrideRepository, LastAppliedRepository, MappingRepository};
use crate::views::*;
use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};

/// Commands that can be sent from UI to async worker
enum AppCommand {
    ConnectDevice(String),
    RefreshPresets,
    ApplyPreset(String),
    SaveMapping(Scope, TrackMeta, String),
    UpdateGenre(String, String), // (track_key, genre)
    BackupDatabase(String), // (db_path)
    Poll,
}

/// Responses from async worker to UI
enum AppResponse {
    Connected(String),
    ConnectionFailed(String),
    PresetsLoaded(Vec<String>),
    PresetApplied(String),
    MappingSaved(String),
    TrackUpdated(TrackMeta, Option<String>),
    BackupCreated(String), // (backup_path)
    Error(String),
}

/// Main application state
pub struct AaeqApp {
    /// Database connection pool
    pool: SqlitePool,
    db_path: std::path::PathBuf,

    /// Current device controller
    device: Option<Arc<dyn DeviceController>>,
    #[allow(dead_code)]
    device_id: Option<i64>,
    device_host: String,

    /// UI Views
    now_playing_view: NowPlayingView,
    presets_view: PresetsView,
    eq_editor_view: Option<EqEditorView>,

    /// Current state
    current_track: Option<TrackMeta>,
    current_preset: Option<String>,
    available_presets: Vec<String>,

    /// Mapping rules cache
    rules_index: Arc<RwLock<RulesIndex>>,

    /// Polling state
    last_poll: Instant,
    poll_interval: Duration,
    #[allow(dead_code)]
    last_track_key: Option<String>,

    /// UI state
    show_eq_editor: bool,
    status_message: Option<String>,

    /// Async communication
    command_tx: mpsc::UnboundedSender<AppCommand>,
    response_rx: mpsc::UnboundedReceiver<AppResponse>,
}

impl AaeqApp {
    pub fn new(pool: SqlitePool, db_path: std::path::PathBuf) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        let rules_index = Arc::new(RwLock::new(RulesIndex::default()));

        // Spawn async worker task
        let worker_pool = pool.clone();
        let worker_rules = rules_index.clone();
        tokio::spawn(async move {
            Self::async_worker(worker_pool, worker_rules, command_rx, response_tx).await;
        });

        Self {
            pool,
            db_path,
            device: None,
            device_id: None,
            device_host: "192.168.1.100".to_string(), // Default, user can change
            now_playing_view: NowPlayingView::default(),
            presets_view: PresetsView::default(),
            eq_editor_view: None,
            current_track: None,
            current_preset: None,
            available_presets: vec![],
            rules_index,
            last_poll: Instant::now(),
            poll_interval: Duration::from_millis(1000),
            last_track_key: None,
            show_eq_editor: false,
            status_message: None,
            command_tx,
            response_rx,
        }
    }

    /// Initialize the app (load mappings, connect to device)
    pub async fn initialize(&mut self) -> Result<()> {
        // Load mappings from database
        self.reload_mappings().await?;

        // Load last connected host from settings
        let settings_repo = AppSettingsRepository::new(self.pool.clone());
        if let Ok(Some(last_host)) = settings_repo.get_last_connected_host().await {
            tracing::info!("Loading last connected host: {}", last_host);
            self.device_host = last_host.clone();
            // Try to connect to the last device
            let _ = self.command_tx.send(AppCommand::ConnectDevice(last_host));
        }

        Ok(())
    }

    /// Reload mapping rules from database
    async fn reload_mappings(&mut self) -> Result<()> {
        let repo = MappingRepository::new(self.pool.clone());
        let mappings = repo.list_all().await?;

        let mut rules = self.rules_index.write().await;
        *rules = RulesIndex::from_mappings(mappings);

        tracing::info!("Loaded {} song rules, {} album rules, {} genre rules",
            rules.song_rules.len(), rules.album_rules.len(), rules.genre_rules.len());

        Ok(())
    }

    /// Try to connect to the WiiM device
    #[allow(dead_code)]
    async fn try_connect_device(&mut self) {
        let controller = WiimController::new("WiiM Device", self.device_host.clone());

        if controller.is_online().await {
            tracing::info!("Connected to device at {}", self.device_host);
            self.device = Some(Arc::new(controller));
            self.status_message = Some(format!("Connected to {}", self.device_host));

            // Load presets
            if let Err(e) = self.refresh_presets().await {
                tracing::error!("Failed to load presets: {}", e);
            }
        } else {
            tracing::warn!("Device at {} is offline", self.device_host);
            self.status_message = Some(format!("Device {} offline", self.device_host));
        }
    }

    /// Refresh preset list from device
    #[allow(dead_code)]
    async fn refresh_presets(&mut self) -> Result<()> {
        if let Some(device) = &self.device {
            let presets = device.list_presets().await?;
            self.available_presets = presets.clone();
            self.presets_view.presets = presets;
            tracing::info!("Loaded {} presets from device", self.available_presets.len());
        }
        Ok(())
    }

    /// Poll device for now playing and apply EQ if needed
    #[allow(dead_code)]
    async fn poll_device(&mut self) -> Result<()> {
        let device = match &self.device {
            Some(d) => d,
            None => return Ok(()),
        };

        // Get current track
        let track = device.get_now_playing().await?;
        let track_key = track.track_key();

        // Check if track changed
        if self.last_track_key.as_deref() != Some(&track_key) {
            tracing::info!("Track changed: {} - {}", track.artist, track.title);

            // Resolve preset
            let rules = self.rules_index.read().await;
            let desired_preset = resolve_preset(&track, &rules, "Flat");
            drop(rules);

            // Apply if different from current
            if self.current_preset.as_deref() != Some(&desired_preset) {
                tracing::info!("Applying preset: {}", desired_preset);
                device.apply_preset(&desired_preset).await?;

                self.current_preset = Some(desired_preset.clone());
                self.status_message = Some(format!("Applied preset: {}", desired_preset));

                // Save to database
                if let Some(device_id) = self.device_id {
                    let repo = LastAppliedRepository::new(self.pool.clone());
                    repo.update(device_id, &track_key, &desired_preset).await?;
                }
            }

            self.current_track = Some(track.clone());
            self.last_track_key = Some(track_key);
        }

        // Update UI
        self.now_playing_view.track = self.current_track.clone();
        self.now_playing_view.current_preset = self.current_preset.clone();

        Ok(())
    }

    /// Save a mapping for the current track
    #[allow(dead_code)]
    async fn save_mapping(&mut self, scope: Scope) -> Result<()> {
        let track = match &self.current_track {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        let preset = match &self.current_preset {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let key_normalized = match scope {
            Scope::Song => Some(track.song_key()),
            Scope::Album => Some(track.album_key()),
            Scope::Genre => Some(track.genre_key()),
            Scope::Default => None,
        };

        let mapping = Mapping {
            id: None,
            scope: scope.clone(),
            key_normalized,
            preset_name: preset.clone(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        let repo = MappingRepository::new(self.pool.clone());
        repo.upsert(&mapping).await?;

        // Reload rules
        self.reload_mappings().await?;

        let scope_name = match scope {
            Scope::Song => "song",
            Scope::Album => "album",
            Scope::Genre => "genre",
            Scope::Default => "default",
        };

        self.status_message = Some(format!("Saved {} mapping for {}", scope_name, preset));
        tracing::info!("Saved mapping: {:?}", mapping);

        Ok(())
    }

    /// Async worker task that handles all async operations
    async fn async_worker(
        pool: SqlitePool,
        rules_index: Arc<RwLock<RulesIndex>>,
        mut command_rx: mpsc::UnboundedReceiver<AppCommand>,
        response_tx: mpsc::UnboundedSender<AppResponse>,
    ) {
        let mut device: Option<Arc<dyn DeviceController>> = None;
        let device_id: Option<i64> = None;
        let mut last_track_key: Option<String> = None;
        let mut current_preset: Option<String> = None;

        while let Some(cmd) = command_rx.recv().await {
            match cmd {
                AppCommand::ConnectDevice(host) => {
                    let controller = WiimController::new("WiiM Device", host.clone());
                    if controller.is_online().await {
                        tracing::info!("Connected to device at {}", host);
                        device = Some(Arc::new(controller));

                        // Save the successful connection to settings
                        let settings_repo = AppSettingsRepository::new(pool.clone());
                        if let Err(e) = settings_repo.set_last_connected_host(&host).await {
                            tracing::error!("Failed to save last connected host: {}", e);
                        }

                        let _ = response_tx.send(AppResponse::Connected(host));
                    } else {
                        tracing::warn!("Device at {} is offline", host);
                        let _ = response_tx.send(AppResponse::ConnectionFailed(host));
                    }
                }

                AppCommand::RefreshPresets => {
                    if let Some(dev) = &device {
                        match dev.list_presets().await {
                            Ok(presets) => {
                                tracing::info!("Loaded {} presets from device", presets.len());
                                let _ = response_tx.send(AppResponse::PresetsLoaded(presets));
                            }
                            Err(e) => {
                                tracing::error!("Failed to load presets: {}", e);
                                let _ = response_tx.send(AppResponse::Error(format!("Failed to load presets: {}", e)));
                            }
                        }
                    }
                }

                AppCommand::ApplyPreset(preset) => {
                    if let Some(dev) = &device {
                        match dev.apply_preset(&preset).await {
                            Ok(_) => {
                                current_preset = Some(preset.clone());
                                let _ = response_tx.send(AppResponse::PresetApplied(preset));
                            }
                            Err(e) => {
                                tracing::error!("Failed to apply preset: {}", e);
                                let _ = response_tx.send(AppResponse::Error(format!("Failed to apply preset: {}", e)));
                            }
                        }
                    }
                }

                AppCommand::UpdateGenre(track_key, genre) => {
                    let repo = GenreOverrideRepository::new(pool.clone());
                    match repo.upsert(&track_key, &genre).await {
                        Ok(_) => {
                            tracing::info!("Updated genre for track: {} -> {}", track_key, genre);
                        }
                        Err(e) => {
                            tracing::error!("Failed to update genre: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to update genre: {}", e)));
                        }
                    }
                }

                AppCommand::SaveMapping(scope, track, preset) => {
                    let key_normalized = match scope {
                        Scope::Song => Some(track.song_key()),
                        Scope::Album => Some(track.album_key()),
                        Scope::Genre => Some(track.genre_key()),
                        Scope::Default => None,
                    };

                    let mapping = Mapping {
                        id: None,
                        scope: scope.clone(),
                        key_normalized,
                        preset_name: preset.clone(),
                        created_at: chrono::Utc::now().timestamp(),
                        updated_at: chrono::Utc::now().timestamp(),
                    };

                    let repo = MappingRepository::new(pool.clone());
                    match repo.upsert(&mapping).await {
                        Ok(_) => {
                            // Reload rules index
                            match repo.list_all().await {
                                Ok(mappings) => {
                                    let mut rules = rules_index.write().await;
                                    *rules = RulesIndex::from_mappings(mappings);
                                    tracing::info!("Reloaded rules: {} song rules, {} album rules, {} genre rules",
                                        rules.song_rules.len(), rules.album_rules.len(), rules.genre_rules.len());
                                }
                                Err(e) => {
                                    tracing::error!("Failed to reload mappings: {}", e);
                                }
                            }

                            let scope_name = match scope {
                                Scope::Song => "song",
                                Scope::Album => "album",
                                Scope::Genre => "genre",
                                Scope::Default => "default",
                            };
                            let msg = format!("Saved {} mapping for {}", scope_name, preset);
                            tracing::info!("{}", msg);
                            let _ = response_tx.send(AppResponse::MappingSaved(msg));
                        }
                        Err(e) => {
                            tracing::error!("Failed to save mapping: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to save mapping: {}", e)));
                        }
                    }
                }

                AppCommand::BackupDatabase(db_path) => {
                    use std::fs;
                    use std::io::Write;

                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    let backup_name = format!("aaeq-bkup_{}.zip", timestamp);
                    let backup_path = std::path::Path::new(&db_path)
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join(&backup_name);

                    match fs::copy(&db_path, backup_path.with_extension("db.tmp")) {
                        Ok(_) => {
                            // Create zip archive
                            let zip_file = match fs::File::create(&backup_path) {
                                Ok(f) => f,
                                Err(e) => {
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to create backup file: {}", e)));
                                    continue;
                                }
                            };

                            let mut zip = zip::ZipWriter::new(zip_file);
                            let options = zip::write::FileOptions::<()>::default()
                                .compression_method(zip::CompressionMethod::Deflated)
                                .compression_level(Some(6));

                            match zip.start_file("aaeq.db", options) {
                                Ok(_) => {
                                    let db_content = match fs::read(backup_path.with_extension("db.tmp")) {
                                        Ok(content) => content,
                                        Err(e) => {
                                            let _ = response_tx.send(AppResponse::Error(format!("Failed to read database: {}", e)));
                                            continue;
                                        }
                                    };

                                    if let Err(e) = zip.write_all(&db_content) {
                                        let _ = response_tx.send(AppResponse::Error(format!("Failed to write to zip: {}", e)));
                                        continue;
                                    }

                                    if let Err(e) = zip.finish() {
                                        let _ = response_tx.send(AppResponse::Error(format!("Failed to finalize zip: {}", e)));
                                        continue;
                                    }

                                    // Clean up temp file
                                    let _ = fs::remove_file(backup_path.with_extension("db.tmp"));

                                    tracing::info!("Database backup created: {}", backup_path.display());
                                    let _ = response_tx.send(AppResponse::BackupCreated(backup_path.display().to_string()));
                                }
                                Err(e) => {
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to create zip file: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to copy database: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to backup database: {}", e)));
                        }
                    }
                }

                AppCommand::Poll => {
                    if let Some(dev) = &device {
                        match dev.get_now_playing().await {
                            Ok(mut track) => {
                                let track_key = track.track_key();

                                // Check if track changed
                                if last_track_key.as_deref() != Some(&track_key) {
                                    tracing::info!("Track changed: {} - {}", track.artist, track.title);

                                    // Load genre override if exists
                                    let genre_repo = GenreOverrideRepository::new(pool.clone());
                                    if let Ok(Some(genre_override)) = genre_repo.get(&track_key).await {
                                        tracing::info!("Using genre override: {}", genre_override);
                                        track.genre = genre_override;
                                    }

                                    // Resolve preset
                                    let rules = rules_index.read().await;
                                    let desired_preset = resolve_preset(&track, &rules, "Flat");
                                    drop(rules);

                                    // Apply if different from current
                                    if current_preset.as_deref() != Some(&desired_preset) {
                                        tracing::info!("Applying preset: {}", desired_preset);
                                        if let Err(e) = dev.apply_preset(&desired_preset).await {
                                            tracing::error!("Failed to apply preset: {}", e);
                                        } else {
                                            current_preset = Some(desired_preset.clone());

                                            // Save to database
                                            if let Some(dev_id) = device_id {
                                                let repo = LastAppliedRepository::new(pool.clone());
                                                let _ = repo.update(dev_id, &track_key, &desired_preset).await;
                                            }
                                        }
                                    }

                                    last_track_key = Some(track_key);
                                }

                                let _ = response_tx.send(AppResponse::TrackUpdated(track, current_preset.clone()));
                            }
                            Err(e) => {
                                tracing::error!("Poll error: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl eframe::App for AaeqApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process responses from async worker
        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                AppResponse::Connected(host) => {
                    self.status_message = Some(format!("Connected to {}", host));
                    // Request preset refresh after connection
                    let _ = self.command_tx.send(AppCommand::RefreshPresets);
                }
                AppResponse::ConnectionFailed(host) => {
                    self.status_message = Some(format!("Device {} offline", host));
                    self.device = None;
                }
                AppResponse::PresetsLoaded(presets) => {
                    self.available_presets = presets.clone();
                    self.presets_view.presets = presets;
                }
                AppResponse::PresetApplied(preset) => {
                    self.current_preset = Some(preset.clone());
                    self.status_message = Some(format!("Applied: {}", preset));
                }
                AppResponse::MappingSaved(msg) => {
                    self.status_message = Some(msg);
                }
                AppResponse::TrackUpdated(track, preset) => {
                    // Check if track actually changed
                    let track_changed = self.current_track.as_ref()
                        .map(|t| t.track_key() != track.track_key())
                        .unwrap_or(true);

                    self.current_track = Some(track.clone());
                    self.current_preset = preset;
                    self.now_playing_view.track = Some(track.clone());
                    self.now_playing_view.current_preset = self.current_preset.clone();

                    // Only update genre_edit if track changed (not on every poll)
                    if track_changed {
                        self.now_playing_view.genre_edit = track.genre.clone();
                    }
                }
                AppResponse::BackupCreated(path) => {
                    self.status_message = Some(format!("Backup created: {}", path));
                }
                AppResponse::Error(msg) => {
                    self.status_message = Some(format!("Error: {}", msg));
                }
            }
        }

        // Poll device periodically
        if self.last_poll.elapsed() >= self.poll_interval {
            self.last_poll = Instant::now();
            if self.device.is_some() {
                let _ = self.command_tx.send(AppCommand::Poll);
            }
        }

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("AAEQ - Adaptive Audio Equalizer");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(msg) = &self.status_message {
                        ui.label(msg);
                    }
                });
            });
        });

        // Device connection panel
        egui::TopBottomPanel::top("device_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Device IP:");
                ui.text_edit_singleline(&mut self.device_host);

                if ui.button("Connect").clicked() {
                    let _ = self.command_tx.send(AppCommand::ConnectDevice(self.device_host.clone()));
                    // Optimistically set device as connected (will be updated by response)
                    self.device = Some(Arc::new(WiimController::new("WiiM Device", self.device_host.clone())));
                }

                if self.device.is_some() {
                    ui.label("✓ Connected");
                } else {
                    ui.label("⚠ Disconnected");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📦 Backup Database").clicked() {
                        let db_path = self.db_path.display().to_string();
                        let _ = self.command_tx.send(AppCommand::BackupDatabase(db_path));
                    }
                });
            });
        });

        // Main content
        if self.show_eq_editor {
            // Show EQ editor
            if let Some(editor) = &mut self.eq_editor_view {
                if let Some(action) = editor.show(ctx) {
                    match action {
                        EqEditorAction::Save(preset) => {
                            tracing::info!("Saving preset: {}", preset.name);
                            // TODO: Save to device
                            self.show_eq_editor = false;
                        }
                        EqEditorAction::Apply(_preset) => {
                            // Note: Custom EQ not supported by WiiM API yet
                            // For now just close the editor
                            tracing::warn!("Custom EQ application not yet implemented");
                            self.status_message = Some("Custom EQ not supported by device API".to_string());
                            self.show_eq_editor = false;
                        }
                        EqEditorAction::Modified => {
                            // Just redraw
                        }
                    }
                }
            }

            // Close button
            egui::TopBottomPanel::bottom("close_editor").show(ctx, |ui| {
                if ui.button("Close Editor").clicked() {
                    self.show_eq_editor = false;
                }
            });
        } else {
            // Show main view
            egui::SidePanel::left("presets_panel").show(ctx, |ui| {
                if let Some(action) = self.presets_view.show(ui) {
                    match action {
                        PresetAction::Refresh => {
                            let _ = self.command_tx.send(AppCommand::RefreshPresets);
                        }
                        PresetAction::Select(preset) => {
                            tracing::info!("Selected preset: {}", preset);
                        }
                        PresetAction::Apply(preset) => {
                            if self.device.is_some() {
                                let _ = self.command_tx.send(AppCommand::ApplyPreset(preset));
                            }
                        }
                        PresetAction::CreateCustom => {
                            self.eq_editor_view = Some(EqEditorView::default());
                            self.show_eq_editor = true;
                        }
                    }
                }
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(action) = self.now_playing_view.show(ui) {
                    match action {
                        NowPlayingAction::SaveMapping(scope) => {
                            // Pass track and preset to the async worker for saving
                            if let (Some(track), Some(preset)) = (&self.current_track, &self.current_preset) {
                                let _ = self.command_tx.send(AppCommand::SaveMapping(scope, track.clone(), preset.clone()));
                            } else {
                                self.status_message = Some("No track or preset to save".to_string());
                            }
                        }
                        NowPlayingAction::UpdateGenre(genre) => {
                            // Update genre for current track
                            if let Some(track) = &self.current_track {
                                let track_key = track.track_key();
                                let _ = self.command_tx.send(AppCommand::UpdateGenre(track_key, genre.clone()));

                                // Update the current track's genre locally
                                if let Some(track) = &mut self.current_track {
                                    track.genre = genre;
                                    self.now_playing_view.track = Some(track.clone());
                                }
                            }
                        }
                    }
                }
            });
        }

        // Request continuous repaint for polling
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
