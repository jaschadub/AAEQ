use aaeq_core::{resolve_preset, DeviceController, Mapping, RulesIndex, Scope, TrackMeta};
use aaeq_device_wiim::WiimController;
use aaeq_persistence::{LastAppliedRepository, MappingRepository};
use crate::views::*;
use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Main application state
pub struct AaeqApp {
    /// Database connection pool
    pool: SqlitePool,

    /// Current device controller
    device: Option<Arc<dyn DeviceController>>,
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
    last_track_key: Option<String>,

    /// UI state
    show_eq_editor: bool,
    status_message: Option<String>,
}

impl AaeqApp {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            device: None,
            device_id: None,
            device_host: "192.168.1.100".to_string(), // Default, user can change
            now_playing_view: NowPlayingView::default(),
            presets_view: PresetsView::default(),
            eq_editor_view: None,
            current_track: None,
            current_preset: None,
            available_presets: vec![],
            rules_index: Arc::new(RwLock::new(RulesIndex::default())),
            last_poll: Instant::now(),
            poll_interval: Duration::from_millis(1000),
            last_track_key: None,
            show_eq_editor: false,
            status_message: None,
        }
    }

    /// Initialize the app (load mappings, connect to device)
    pub async fn initialize(&mut self) -> Result<()> {
        // Load mappings from database
        self.reload_mappings().await?;

        // Try to connect to device if we have one saved
        self.try_connect_device().await;

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
}

impl eframe::App for AaeqApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll device periodically
        if self.last_poll.elapsed() >= self.poll_interval {
            self.last_poll = Instant::now();

            let _pool = self.pool.clone();
            let _device = self.device.clone();
            let _rules_index = self.rules_index.clone();

            // Spawn polling task (non-blocking)
            tokio::spawn(async move {
                // This is a simplified version - in production you'd want better error handling
            });

            // For now, we'll do a blocking poll (not ideal, but works for MVP)
            if let Some(_device) = &self.device {
                let rt = tokio::runtime::Handle::current();
                if let Err(e) = rt.block_on(self.poll_device()) {
                    tracing::error!("Poll error: {}", e);
                }
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
                    let rt = tokio::runtime::Handle::current();
                    rt.block_on(self.try_connect_device());
                }

                if self.device.is_some() {
                    ui.label("✓ Connected");
                } else {
                    ui.label("⚠ Disconnected");
                }
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
                        EqEditorAction::Apply(preset) => {
                            if let Some(device) = &self.device {
                                let rt = tokio::runtime::Handle::current();
                                if let Err(e) = rt.block_on(device.set_custom_eq(&preset)) {
                                    tracing::error!("Failed to apply EQ: {}", e);
                                } else {
                                    self.current_preset = Some(preset.name.clone());
                                }
                            }
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
                            let rt = tokio::runtime::Handle::current();
                            if let Err(e) = rt.block_on(self.refresh_presets()) {
                                tracing::error!("Failed to refresh presets: {}", e);
                            }
                        }
                        PresetAction::Select(preset) => {
                            tracing::info!("Selected preset: {}", preset);
                        }
                        PresetAction::Apply(preset) => {
                            if let Some(device) = &self.device {
                                let rt = tokio::runtime::Handle::current();
                                if let Err(e) = rt.block_on(device.apply_preset(&preset)) {
                                    tracing::error!("Failed to apply preset: {}", e);
                                } else {
                                    self.current_preset = Some(preset.clone());
                                    self.status_message = Some(format!("Applied: {}", preset));
                                }
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
                            let rt = tokio::runtime::Handle::current();
                            if let Err(e) = rt.block_on(self.save_mapping(scope)) {
                                tracing::error!("Failed to save mapping: {}", e);
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
