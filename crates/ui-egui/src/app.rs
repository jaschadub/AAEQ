use aaeq_core::{resolve_preset, DeviceController, Mapping, RulesIndex, Scope, TrackMeta};
use aaeq_device_wiim::{WiimController, discover_devices_quick};
use aaeq_persistence::{AppSettingsRepository, GenreOverrideRepository, LastAppliedRepository, MappingRepository};
use crate::views::*;
use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use stream_server::{OutputConfig, SampleFormat, LocalDacSink, DlnaSink, OutputManager, AudioBlock, SinkStats, EqProcessor};

/// Commands that can be sent from UI to async worker
enum AppCommand {
    ConnectDevice(String),
    DiscoverDevices,
    RefreshPresets,
    ApplyPreset(String),
    SaveMapping(Scope, TrackMeta, String),
    UpdateGenre(String, String), // (track_key, genre)
    BackupDatabase(String), // (db_path)
    Poll,
    SaveInputDevice(String), // Save last input device to settings
    // DSP Commands
    DspDiscoverDevices(SinkType, Option<String>), // (sink_type, fallback_ip)
    DspStartStreaming(SinkType, String, OutputConfig, bool, Option<String>, Option<String>), // (sink_type, output_device, config, use_test_tone, input_device, preset_name)
    DspStopStreaming,
}

/// Responses from async worker to UI
enum AppResponse {
    Connected(String),
    ConnectionFailed(String),
    Disconnected(String), // Device went offline during operation
    DevicesDiscovered(Vec<(String, String)>), // Vec<(name, host)>
    PresetsLoaded(Vec<String>),
    PresetApplied(String),
    MappingSaved(String),
    TrackUpdated(TrackMeta, Option<String>),
    BackupCreated(String), // (backup_path)
    Error(String),
    // DSP Responses
    DspDevicesDiscovered(Vec<String>),
    DspStreamingStarted,
    DspStreamingStopped,
    DspStreamStatus(StreamStatus),
    DspAudioSamples(Vec<f64>), // For visualization
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
    dsp_view: DspView,

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
    show_dsp_panel: bool,
    status_message: Option<String>,
    auto_reconnect: bool,
    connection_lost_time: Option<Instant>,
    reconnect_interval: Duration,
    discovered_devices: Vec<(String, String)>, // Vec<(name, host)>
    show_discovery: bool,

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
            dsp_view: DspView::default(),
            current_track: None,
            current_preset: None,
            available_presets: vec![],
            rules_index,
            last_poll: Instant::now(),
            poll_interval: Duration::from_millis(1000),
            last_track_key: None,
            show_eq_editor: false,
            show_dsp_panel: false,
            status_message: None,
            auto_reconnect: true, // Enable by default
            connection_lost_time: None,
            reconnect_interval: Duration::from_secs(5),
            discovered_devices: vec![],
            show_discovery: false,
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

        // Load last input device from settings
        if let Ok(Some(last_input)) = settings_repo.get_last_input_device().await {
            tracing::info!("Loading last input device: {}", last_input);
            self.dsp_view.selected_input_device = Some(last_input);
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

        // DSP state
        let mut output_manager = OutputManager::new();
        let mut streaming_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut stream_shutdown_tx: Option<mpsc::Sender<()>> = None;

        // DLNA device cache
        let mut discovered_dlna_devices: Vec<stream_server::sinks::dlna::DlnaDevice> = Vec::new();

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

                AppCommand::DiscoverDevices => {
                    tracing::info!("Starting device discovery...");
                    match discover_devices_quick().await {
                        Ok(devices) => {
                            let device_list: Vec<(String, String)> = devices
                                .into_iter()
                                .map(|d| (d.name, d.host))
                                .collect();
                            tracing::info!("Discovered {} devices", device_list.len());
                            let _ = response_tx.send(AppResponse::DevicesDiscovered(device_list));
                        }
                        Err(e) => {
                            tracing::error!("Device discovery failed: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Discovery failed: {}", e)));
                        }
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

                AppCommand::SaveInputDevice(device_name) => {
                    let settings_repo = AppSettingsRepository::new(pool.clone());
                    if let Err(e) = settings_repo.set_last_input_device(&device_name).await {
                        tracing::error!("Failed to save last input device: {}", e);
                    } else {
                        tracing::info!("Saved last input device: {}", device_name);
                    }
                }

                AppCommand::Poll => {
                    // Poll from WiiM device if connected, otherwise try MPRIS for DSP mode
                    if let Some(dev) = &device {
                        match dev.get_now_playing().await {
                            Ok(mut track) => {
                                // Check if this is just the DSP stream (not real media)
                                // If title is "AAEQ Stream", we're streaming via DSP, so check MPRIS instead
                                let is_dsp_stream = track.title == "AAEQ Stream" || track.title == "414145512053747265616D";

                                if is_dsp_stream {
                                    // This is our DSP stream, get real metadata from MPRIS
                                    tracing::debug!("Detected DSP stream, checking MPRIS for real track info");
                                    match crate::mpris::get_now_playing_mpris() {
                                        Ok(mpris_track) => {
                                            track = mpris_track;
                                            tracing::debug!("Using MPRIS track: {} - {}", track.artist, track.title);
                                        }
                                        Err(e) => {
                                            tracing::debug!("MPRIS not available: {}", e);
                                            // Keep the WiiM track (will show "AAEQ Stream")
                                        }
                                    }
                                }

                                let track_key = track.track_key();

                                // Check if track changed
                                if last_track_key.as_deref() != Some(&track_key) {
                                    tracing::info!("Track changed: {} - {}", track.artist, track.title);

                                    // Store device genre before applying override
                                    track.device_genre = track.genre.clone();

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
                                // Device appears to be disconnected
                                if device.is_some() {
                                    tracing::warn!("Device connection lost - marking as disconnected");
                                    device = None;
                                    current_preset = None;
                                    last_track_key = None;
                                    let _ = response_tx.send(AppResponse::Disconnected("Connection lost during polling".to_string()));
                                }
                            }
                        }
                    } else {
                        // No WiiM device connected, try MPRIS for DSP mode
                        match crate::mpris::get_now_playing_mpris() {
                            Ok(mut track) => {
                                let track_key = track.track_key();

                                // Check if track changed
                                if last_track_key.as_deref() != Some(&track_key) {
                                    tracing::info!("Track changed (MPRIS): {} - {}", track.artist, track.title);

                                    // Store device genre before applying override
                                    track.device_genre = track.genre.clone();

                                    // Load genre override if exists
                                    let genre_repo = GenreOverrideRepository::new(pool.clone());
                                    if let Ok(Some(genre_override)) = genre_repo.get(&track_key).await {
                                        tracing::info!("Using genre override: {}", genre_override);
                                        track.genre = genre_override;
                                    }

                                    // Note: In DSP mode without WiiM API, we can't apply presets to the device
                                    // But we can still show the track info and what preset would be used
                                    let rules = rules_index.read().await;
                                    let desired_preset = resolve_preset(&track, &rules, "Flat");
                                    drop(rules);

                                    current_preset = Some(desired_preset.clone());
                                    tracing::info!("Preset for track (DSP mode): {}", desired_preset);

                                    last_track_key = Some(track_key);
                                }

                                let _ = response_tx.send(AppResponse::TrackUpdated(track, current_preset.clone()));
                            }
                            Err(e) => {
                                // MPRIS polling failed - this is OK if no media is playing
                                tracing::debug!("MPRIS poll error (no media playing?): {}", e);
                            }
                        }
                    }
                }

                // DSP Commands
                AppCommand::DspDiscoverDevices(sink_type, fallback_ip) => {
                    tracing::info!("Discovering DSP devices for sink type: {:?}", sink_type);

                    match sink_type {
                        SinkType::LocalDac => {
                            // List local DAC devices
                            match LocalDacSink::list_devices() {
                                Ok(devices) => {
                                    tracing::info!("Found {} local DAC devices", devices.len());
                                    let _ = response_tx.send(AppResponse::DspDevicesDiscovered(devices));
                                }
                                Err(e) => {
                                    tracing::error!("Failed to list DAC devices: {}", e);
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to list DAC devices: {}", e)));
                                }
                            }
                        }
                        SinkType::Dlna => {
                            // Discover DLNA devices
                            use stream_server::sinks::dlna::discovery::{discover_devices, create_device_from_ip};

                            let mut discovered_devices = match discover_devices(10).await { // Increased timeout to 10 seconds
                                Ok(devices) => devices,
                                Err(e) => {
                                    tracing::warn!("DLNA multicast discovery failed: {}", e);
                                    Vec::new()
                                }
                            };

                            // If multicast discovery found no devices, try the fallback IP
                            if discovered_devices.is_empty() {
                                if let Some(ip) = fallback_ip {
                                    tracing::info!("Multicast discovery failed, trying manual device creation with IP: {}", ip);
                                    match create_device_from_ip(&ip, None).await {
                                        Ok(dlna_device) => {
                                            tracing::info!("Successfully created DLNA device from known IP!");
                                            discovered_devices.push(dlna_device);
                                        }
                                        Err(e) => {
                                            tracing::warn!("Failed to create DLNA device from IP {}: {}", ip, e);
                                        }
                                    }
                                }
                            }

                            // Cache the discovered DLNA devices for later use
                            discovered_dlna_devices = discovered_devices.clone();
                            tracing::info!("Cached {} DLNA device(s)", discovered_dlna_devices.len());

                            let device_names: Vec<String> = discovered_devices.iter().map(|d| d.name.clone()).collect();
                            tracing::info!("Found {} DLNA device(s) total", device_names.len());
                            let _ = response_tx.send(AppResponse::DspDevicesDiscovered(device_names));
                        }
                        SinkType::AirPlay => {
                            // Discover AirPlay devices
                            use stream_server::AirPlaySink;

                            match AirPlaySink::discover(10).await { // 10 second timeout
                                Ok(devices) => {
                                    let device_names: Vec<String> = devices.iter().map(|d| d.name.clone()).collect();
                                    tracing::info!("Found {} AirPlay devices", device_names.len());
                                    let _ = response_tx.send(AppResponse::DspDevicesDiscovered(device_names));
                                }
                                Err(e) => {
                                    tracing::error!("Failed to discover AirPlay devices: {}", e);
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to discover AirPlay devices: {}", e)));
                                }
                            }
                        }
                    }
                }

                AppCommand::DspStartStreaming(sink_type, device_name, config, use_test_tone, input_device, preset_name) => {
                    tracing::info!("Starting DSP streaming: {:?} to device '{}' (test_tone: {}, input: {:?}, preset: {:?})",
                        sink_type, device_name, use_test_tone, input_device, preset_name);

                    // Stop any existing stream first
                    if let Some(task) = streaming_task.take() {
                        if let Some(shutdown) = stream_shutdown_tx.take() {
                            let _ = shutdown.send(()).await;
                        }
                        task.abort();
                    }

                    // Close any active sink
                    if let Err(e) = output_manager.close_active().await {
                        tracing::warn!("Failed to close active sink: {}", e);
                    }

                    // Create and register the appropriate sink
                    let sink_result: Result<(), String> = match sink_type {
                        SinkType::LocalDac => {
                            let sink = LocalDacSink::new(Some(device_name.clone()));
                            output_manager.register_sink(Box::new(sink));
                            output_manager.select_sink(0, config.clone()).await
                                .map_err(|e| format!("Failed to open local DAC: {}", e))
                        }
                        SinkType::Dlna => {
                            // Use cached DLNA devices instead of re-discovering
                            tracing::info!("Looking for DLNA device '{}' in cache ({} devices)", device_name, discovered_dlna_devices.len());

                            if let Some(dlna_device) = discovered_dlna_devices.iter().find(|d| d.name == device_name) {
                                tracing::info!("Found device '{}' in cache", device_name);
                                let bind_addr = "0.0.0.0:8090".parse().unwrap();
                                // Use Push mode to automatically start playback on the device
                                let sink = DlnaSink::with_device(
                                    dlna_device.clone(),
                                    bind_addr,
                                    stream_server::DlnaMode::Push
                                );
                                output_manager.register_sink(Box::new(sink));
                                output_manager.select_sink(0, config.clone()).await
                                    .map_err(|e| format!("Failed to open DLNA sink: {}", e))
                            } else {
                                tracing::error!("DLNA device '{}' not found in cache. Available devices: {:?}",
                                    device_name,
                                    discovered_dlna_devices.iter().map(|d| &d.name).collect::<Vec<_>>());
                                Err(format!("DLNA device '{}' not found in cache. Please run device discovery first.", device_name))
                            }
                        }
                        SinkType::AirPlay => {
                            use stream_server::AirPlaySink;

                            // Find the AirPlay device by name
                            let mut sink = AirPlaySink::new();
                            match sink.set_device_by_name(&device_name, 10).await {
                                Ok(_) => {
                                    output_manager.register_sink(Box::new(sink));
                                    output_manager.select_sink(0, config.clone()).await
                                        .map_err(|e| format!("Failed to open AirPlay sink: {}", e))
                                }
                                Err(e) => {
                                    Err(format!("Failed to find AirPlay device '{}': {}", device_name, e))
                                }
                            }
                        }
                    };

                    match sink_result {
                        Ok(_) => {
                            tracing::info!("Sink opened successfully, starting streaming task");

                            // Create shutdown channel for the streaming task
                            let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
                            stream_shutdown_tx = Some(shutdown_tx);

                            // Setup audio capture if not using test tone
                            let audio_capture_for_task: Option<(mpsc::Receiver<Vec<f64>>, mpsc::Sender<()>)> =
                                if !use_test_tone {
                                    tracing::info!("Starting audio capture from input device");
                                    let (capture_tx, capture_rx) = mpsc::channel::<Vec<f64>>(32);

                                    match stream_server::LocalDacInput::start_capture(
                                        input_device.clone(),
                                        config.clone(),
                                        capture_tx,
                                    ) {
                                        Ok(stop_tx) => {
                                            tracing::info!("Audio capture started successfully");
                                            Some((capture_rx, stop_tx))
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to start audio capture: {}", e);
                                            let _ = response_tx.send(AppResponse::Error(format!("Failed to start audio capture: {}", e)));
                                            None
                                        }
                                    }
                                } else {
                                    None
                                };

                            // Spawn streaming task
                            let manager = Arc::new(RwLock::new(output_manager));
                            output_manager = OutputManager::new(); // Create a new one for future use
                            let tx = response_tx.clone();
                            let sample_rate = config.sample_rate;
                            let _pool_for_task = pool.clone();

                            let task = tokio::spawn(async move {
                                tracing::info!("Streaming task started (test_tone: {})", use_test_tone);
                                let mut frame_count: u64 = 0;
                                let mut phase: f64 = 0.0;
                                let frequency = 1000.0; // 1kHz tone
                                let channels = 2;
                                let frames_per_block = (sample_rate / 100) as usize; // 10ms worth of samples

                                let mut audio_capture = audio_capture_for_task;

                                // Initialize EQ processor if preset is provided
                                let mut eq_processor = EqProcessor::new(sample_rate, channels);
                                if let Some(preset_name) = preset_name {
                                    tracing::info!("Loading EQ preset: {}", preset_name);
                                    if let Some(preset) = crate::preset_library::get_preset_curve(&preset_name) {
                                        eq_processor.load_preset(&preset);
                                        tracing::info!("EQ preset loaded: {} ({} bands)", preset_name, eq_processor.band_count());
                                    } else {
                                        tracing::warn!("Preset '{}' not found in library", preset_name);
                                    }
                                }

                                // Calculate precise interval for audio blocks
                                let block_duration_ms = (frames_per_block as f64 / sample_rate as f64 * 1000.0) as u64;
                                let mut interval = tokio::time::interval(Duration::from_millis(block_duration_ms));
                                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                                // Pre-fill buffer with a few blocks to prevent initial underruns
                                tracing::info!("Pre-filling audio buffer...");
                                for _ in 0..3 {
                                    let mut audio_data = vec![0.0f64; frames_per_block * channels];
                                    for i in 0..frames_per_block {
                                        let sample = (phase * 2.0 * std::f64::consts::PI).sin() * 0.1;
                                        audio_data[i * channels] = sample;
                                        audio_data[i * channels + 1] = sample;
                                        phase += frequency / sample_rate as f64;
                                        if phase >= 1.0 {
                                            phase -= 1.0;
                                        }
                                    }
                                    let block = AudioBlock::new(&audio_data, sample_rate, channels as u16);
                                    let mut mgr = manager.write().await;
                                    if let Err(e) = mgr.write(block).await {
                                        tracing::error!("Failed to pre-fill buffer: {}", e);
                                        break;
                                    }
                                    frame_count += frames_per_block as u64;
                                }
                                tracing::info!("Buffer pre-filled, starting playback");

                                loop {
                                    tokio::select! {
                                        _ = shutdown_rx.recv() => {
                                            tracing::info!("Streaming task received shutdown signal");
                                            break;
                                        }
                                        // Audio capture mode - wait for samples
                                        Some(mut captured_samples) = async {
                                            if let Some((rx, _)) = audio_capture.as_mut() {
                                                rx.recv().await
                                            } else {
                                                None
                                            }
                                        }, if audio_capture.is_some() => {
                                            // Process captured audio samples
                                            // The captured samples are already interleaved stereo f64

                                            // Apply EQ processing
                                            eq_processor.process(&mut captured_samples);

                                            // Send samples for visualization
                                            let viz_samples: Vec<f64> = captured_samples.iter()
                                                .step_by(channels) // Take every Nth sample (left channel)
                                                .take(256) // Limit visualization data
                                                .copied()
                                                .collect();
                                            let _ = tx.send(AppResponse::DspAudioSamples(viz_samples));

                                            // Create audio block from captured samples
                                            let block = AudioBlock::new(&captured_samples, sample_rate, channels as u16);

                                            // Write to sink
                                            let mut mgr = manager.write().await;
                                            if let Err(e) = mgr.write(block).await {
                                                tracing::error!("Failed to write audio block: {}", e);
                                                break;
                                            }

                                            frame_count += (captured_samples.len() / channels) as u64;

                                            // Send status update periodically
                                            if frame_count % (sample_rate as u64 / 10) == 0 {
                                                let latency = mgr.active_sink_latency().unwrap_or(0);
                                                let stats = mgr.active_sink_stats()
                                                    .cloned()
                                                    .unwrap_or(SinkStats::default());

                                                let status = StreamStatus {
                                                    latency_ms: latency,
                                                    frames_written: frame_count,
                                                    underruns: stats.underruns,
                                                    buffer_fill: stats.buffer_fill,
                                                };

                                                let _ = tx.send(AppResponse::DspStreamStatus(status));
                                            }
                                        }
                                        // Test tone mode - generate sine wave on interval
                                        _ = interval.tick(), if use_test_tone => {
                                            // Generate audio block with sine wave
                                            let mut audio_data = vec![0.0f64; frames_per_block * channels];

                                            for i in 0..frames_per_block {
                                                let sample = (phase * 2.0 * std::f64::consts::PI).sin() * 0.1; // 10% amplitude
                                                audio_data[i * channels] = sample; // Left
                                                audio_data[i * channels + 1] = sample; // Right

                                                phase += frequency / sample_rate as f64;
                                                if phase >= 1.0 {
                                                    phase -= 1.0;
                                                }
                                            }

                                            // Apply EQ processing
                                            eq_processor.process(&mut audio_data);

                                            // Send samples for visualization
                                            let viz_samples: Vec<f64> = audio_data.iter()
                                                .step_by(channels)
                                                .take(256)
                                                .copied()
                                                .collect();
                                            let _ = tx.send(AppResponse::DspAudioSamples(viz_samples));

                                            let block = AudioBlock::new(&audio_data, sample_rate, channels as u16);

                                            // Write to sink
                                            let mut mgr = manager.write().await;
                                            if let Err(e) = mgr.write(block).await {
                                                tracing::error!("Failed to write audio block: {}", e);
                                                break;
                                            }

                                            frame_count += frames_per_block as u64;

                                            // Send status update every 100ms
                                            if frame_count % (sample_rate as u64 / 10) == 0 {
                                                let latency = mgr.active_sink_latency().unwrap_or(0);
                                                let stats = mgr.active_sink_stats()
                                                    .cloned()
                                                    .unwrap_or(SinkStats::default());

                                                let status = StreamStatus {
                                                    latency_ms: latency,
                                                    frames_written: frame_count,
                                                    underruns: stats.underruns,
                                                    buffer_fill: stats.buffer_fill,
                                                };

                                                let _ = tx.send(AppResponse::DspStreamStatus(status));
                                            }
                                        }
                                    }
                                }

                                // Clean up audio capture if active
                                if let Some((_rx, stop_tx)) = audio_capture.take() {
                                    tracing::info!("Stopping audio capture");
                                    let _ = stop_tx.send(()).await;
                                }
                                // Stream handle will be dropped when holder thread exits

                                // Clean up output sink
                                let mut mgr = manager.write().await;
                                if let Err(e) = mgr.drain().await {
                                    tracing::warn!("Failed to drain sink: {}", e);
                                }
                                if let Err(e) = mgr.close_active().await {
                                    tracing::warn!("Failed to close sink: {}", e);
                                }

                                tracing::info!("Streaming task finished");
                            });

                            streaming_task = Some(task);
                            let _ = response_tx.send(AppResponse::DspStreamingStarted);
                        }
                        Err(e) => {
                            tracing::error!("Failed to start streaming: {}", e);
                            let _ = response_tx.send(AppResponse::Error(e));
                        }
                    }
                }

                AppCommand::DspStopStreaming => {
                    tracing::info!("Stopping DSP streaming");

                    // Stop the streaming task
                    if let Some(task) = streaming_task.take() {
                        if let Some(shutdown) = stream_shutdown_tx.take() {
                            let _ = shutdown.send(()).await;
                        }
                        // Wait for task to complete (with timeout)
                        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
                    }

                    // Close the active sink
                    if let Err(e) = output_manager.close_active().await {
                        tracing::warn!("Failed to close active sink: {}", e);
                    }

                    let _ = response_tx.send(AppResponse::DspStreamingStopped);
                    tracing::info!("DSP streaming stopped successfully");
                }
            }
        }
    }
}

impl eframe::App for AaeqApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle window close event - minimize to tray instead of quitting
        if ctx.input(|i| i.viewport().close_requested()) {
            tracing::info!("Close requested - minimizing to tray");
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        // Process responses from async worker
        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                AppResponse::Connected(host) => {
                    self.status_message = Some(format!("Connected to {}", host));
                    self.connection_lost_time = None; // Clear reconnect timer
                    self.show_discovery = false; // Close discovery dialog
                    // Request preset refresh after connection
                    let _ = self.command_tx.send(AppCommand::RefreshPresets);
                }
                AppResponse::ConnectionFailed(host) => {
                    self.status_message = Some(format!("Device {} offline", host));
                    self.device = None;
                }
                AppResponse::DevicesDiscovered(devices) => {
                    self.discovered_devices = devices;
                    if self.discovered_devices.is_empty() {
                        self.status_message = Some("No devices found".to_string());
                    } else {
                        self.status_message = Some(format!("Found {} device(s)", self.discovered_devices.len()));
                    }
                }
                AppResponse::Disconnected(msg) => {
                    tracing::warn!("Device disconnected: {}", msg);
                    self.status_message = Some(format!("Disconnected: {}", msg));
                    self.device = None;
                    self.current_track = None;
                    self.current_preset = None;
                    self.now_playing_view.track = None;
                    self.now_playing_view.current_preset = None;
                    // Mark connection lost time for auto-reconnect
                    if self.auto_reconnect && self.connection_lost_time.is_none() {
                        self.connection_lost_time = Some(Instant::now());
                        tracing::info!("Auto-reconnect enabled - will retry in {:?}", self.reconnect_interval);
                    }
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
                // DSP Responses
                AppResponse::DspDevicesDiscovered(devices) => {
                    self.dsp_view.available_devices = devices;
                    self.dsp_view.discovering = false;
                    self.status_message = Some(format!("Found {} device(s)", self.dsp_view.available_devices.len()));
                }
                AppResponse::DspStreamingStarted => {
                    self.dsp_view.is_streaming = true;
                    self.status_message = Some("Streaming started".to_string());
                }
                AppResponse::DspStreamingStopped => {
                    self.dsp_view.is_streaming = false;
                    self.dsp_view.stream_status = None;
                    self.status_message = Some("Streaming stopped".to_string());
                }
                AppResponse::DspStreamStatus(status) => {
                    self.dsp_view.stream_status = Some(status);
                }
                AppResponse::DspAudioSamples(samples) => {
                    // Update visualization with audio samples
                    self.dsp_view.audio_viz.push_samples(&samples);
                }
            }
        }

        // Poll device periodically (both for WiiM API and DSP streaming)
        if self.last_poll.elapsed() >= self.poll_interval {
            self.last_poll = Instant::now();
            // Poll if either:
            // 1. Device is connected (WiiM API mode)
            // 2. DSP streaming is active (to get now playing from the device while streaming)
            if self.device.is_some() || self.dsp_view.is_streaming {
                let _ = self.command_tx.send(AppCommand::Poll);
            }
        }

        // Auto-reconnect logic
        if self.auto_reconnect && self.device.is_none() {
            if let Some(lost_time) = self.connection_lost_time {
                if lost_time.elapsed() >= self.reconnect_interval {
                    tracing::info!("Attempting auto-reconnect to {}", self.device_host);
                    self.status_message = Some(format!("Reconnecting to {}...", self.device_host));
                    let _ = self.command_tx.send(AppCommand::ConnectDevice(self.device_host.clone()));
                    // Reset timer - will be set again if connection fails
                    self.connection_lost_time = Some(Instant::now());
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
                    let _ = self.command_tx.send(AppCommand::ConnectDevice(self.device_host.clone()));
                    // Optimistically set device as connected (will be updated by response)
                    self.device = Some(Arc::new(WiimController::new("WiiM Device", self.device_host.clone())));
                }

                if ui.button("🔍 Discover").on_hover_text("Discover WiiM devices on local network").clicked() {
                    let _ = self.command_tx.send(AppCommand::DiscoverDevices);
                    self.show_discovery = true;
                    self.status_message = Some("Scanning for devices...".to_string());
                }

                if self.device.is_some() {
                    ui.label("✓ Connected");
                } else {
                    ui.label("⚠ Disconnected");
                }

                ui.separator();

                if ui.checkbox(&mut self.auto_reconnect, "Auto-reconnect").on_hover_text("Automatically reconnect when device goes offline").changed() {
                    if self.auto_reconnect && self.device.is_none() {
                        // Enable auto-reconnect for disconnected device
                        self.connection_lost_time = Some(Instant::now());
                        tracing::info!("Auto-reconnect enabled");
                    } else if !self.auto_reconnect {
                        // Disable auto-reconnect
                        self.connection_lost_time = None;
                        tracing::info!("Auto-reconnect disabled");
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📦 Backup Database").clicked() {
                        let db_path = self.db_path.display().to_string();
                        let _ = self.command_tx.send(AppCommand::BackupDatabase(db_path));
                    }

                    if ui.button(if self.show_dsp_panel { "⏺ Hide DSP" } else { "⏺ Show DSP" })
                        .on_hover_text("Show/hide DSP audio output panel").clicked() {
                        self.show_dsp_panel = !self.show_dsp_panel;
                    }
                });
            });
        });

        // Device discovery dialog
        if self.show_discovery {
            egui::Window::new("Discovered Devices")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if self.discovered_devices.is_empty() {
                        ui.label("Scanning for devices...");
                        ui.label("This may take a few seconds.");
                    } else {
                        ui.label(format!("Found {} device(s):", self.discovered_devices.len()));
                        ui.separator();

                        // List discovered devices
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for (name, host) in &self.discovered_devices.clone() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}", name));
                                    ui.label(format!("({})", host));
                                    if ui.button("Connect").clicked() {
                                        self.device_host = host.clone();
                                        let _ = self.command_tx.send(AppCommand::ConnectDevice(host.clone()));
                                        self.device = Some(Arc::new(WiimController::new("WiiM Device", host.clone())));
                                        self.show_discovery = false;
                                    }
                                });
                            }
                        });
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Scan Again").clicked() {
                            self.discovered_devices.clear();
                            let _ = self.command_tx.send(AppCommand::DiscoverDevices);
                            self.status_message = Some("Scanning for devices...".to_string());
                        }

                        if ui.button("Close").clicked() {
                            self.show_discovery = false;
                        }
                    });
                });
        }

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

            // Show DSP panel if enabled
            if self.show_dsp_panel {
                egui::SidePanel::right("dsp_panel").min_width(300.0).show(ctx, |ui| {
                    if let Some(action) = self.dsp_view.show(ui) {
                        match action {
                            DspAction::SinkTypeChanged(sink_type) => {
                                tracing::info!("DSP sink type changed: {:?}", sink_type);
                                // Clear current device selection when switching sink types
                                self.dsp_view.available_devices.clear();
                                self.dsp_view.selected_device = None;

                                // Adjust format based on sink type
                                match sink_type {
                                    SinkType::Dlna => {
                                        // DLNA requires PCM format (S16LE or S24LE)
                                        if self.dsp_view.format == FormatOption::F32 {
                                            self.dsp_view.format = FormatOption::S24LE;
                                            tracing::info!("Switched to S24LE format for DLNA compatibility");
                                        }
                                    }
                                    SinkType::LocalDac | SinkType::AirPlay => {
                                        // Other sinks support all formats
                                    }
                                }

                                // Automatically discover devices for the new sink type
                                self.dsp_view.discovering = true;
                                let _ = self.command_tx.send(AppCommand::DspDiscoverDevices(sink_type, Some(self.device_host.clone())));
                            }
                            DspAction::DeviceSelected(device) => {
                                tracing::info!("DSP device selected: {}", device);
                            }
                            DspAction::DiscoverDevices => {
                                let _ = self.command_tx.send(AppCommand::DspDiscoverDevices(self.dsp_view.selected_sink, Some(self.device_host.clone())));
                            }
                            DspAction::StartStreaming => {
                                // Convert FormatOption to SampleFormat
                                let format = match self.dsp_view.format {
                                    FormatOption::F32 => SampleFormat::F32,
                                    FormatOption::S24LE => SampleFormat::S24LE,
                                    FormatOption::S16LE => SampleFormat::S16LE,
                                };

                                let config = OutputConfig {
                                    sample_rate: self.dsp_view.sample_rate,
                                    channels: 2,
                                    format,
                                    buffer_ms: self.dsp_view.buffer_ms,
                                    exclusive: false,
                                };

                                if let Some(device) = &self.dsp_view.selected_device {
                                    let _ = self.command_tx.send(AppCommand::DspStartStreaming(
                                        self.dsp_view.selected_sink,
                                        device.clone(),
                                        config,
                                        self.dsp_view.use_test_tone,
                                        self.dsp_view.selected_input_device.clone(),
                                        self.dsp_view.selected_preset.clone(),
                                    ));
                                } else {
                                    self.status_message = Some("No device selected".to_string());
                                }
                            }
                            DspAction::StopStreaming => {
                                let _ = self.command_tx.send(AppCommand::DspStopStreaming);
                            }
                            DspAction::PlayTestTone => {
                                // If not streaming, automatically start streaming to play the test tone
                                if !self.dsp_view.is_streaming {
                                    // Convert FormatOption to SampleFormat
                                    let format = match self.dsp_view.format {
                                        FormatOption::F32 => SampleFormat::F32,
                                        FormatOption::S24LE => SampleFormat::S24LE,
                                        FormatOption::S16LE => SampleFormat::S16LE,
                                    };

                                    let config = OutputConfig {
                                        sample_rate: self.dsp_view.sample_rate,
                                        channels: 2,
                                        format,
                                        buffer_ms: self.dsp_view.buffer_ms,
                                        exclusive: false,
                                    };

                                    if let Some(device) = &self.dsp_view.selected_device {
                                        let _ = self.command_tx.send(AppCommand::DspStartStreaming(
                                            self.dsp_view.selected_sink,
                                            device.clone(),
                                            config,
                                            true, // use_test_tone
                                            None, // input_device
                                            self.dsp_view.selected_preset.clone(), // Use selected EQ preset
                                        ));
                                        self.status_message = Some("Starting test tone...".to_string());
                                    } else {
                                        self.status_message = Some("No device selected".to_string());
                                    }
                                } else {
                                    // Already streaming, the tone is already playing
                                    self.status_message = Some("Test tone is already playing (1kHz)".to_string());
                                }
                            }
                            DspAction::ToggleTestTone => {
                                tracing::info!("Test tone toggled: {}", self.dsp_view.use_test_tone);
                                // If streaming is active and we toggled, restart streaming with new mode
                                if self.dsp_view.is_streaming {
                                    let _ = self.command_tx.send(AppCommand::DspStopStreaming);
                                    self.status_message = Some("Restart streaming to apply changes".to_string());
                                }
                            }
                            DspAction::InputDeviceSelected(device) => {
                                tracing::info!("Input device selected: {}", device);
                                // Save the selected input device to settings
                                let _ = self.command_tx.send(AppCommand::SaveInputDevice(device));
                            }
                            DspAction::DiscoverInputDevices => {
                                use stream_server::LocalDacInput;
                                match LocalDacInput::list_devices() {
                                    Ok(devices) => {
                                        self.dsp_view.available_input_devices = devices;
                                        self.status_message = Some(format!("Found {} input device(s)", self.dsp_view.available_input_devices.len()));
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to list input devices: {}", e);
                                        self.status_message = Some(format!("Failed to list input devices: {}", e));
                                    }
                                }
                            }
                            DspAction::ToggleVisualization => {
                                tracing::info!("Visualization toggled: {}", self.dsp_view.audio_viz.enabled);
                            }
                            DspAction::PresetSelected(preset) => {
                                if let Some(ref preset_name) = preset {
                                    tracing::info!("EQ preset selected: {}", preset_name);
                                } else {
                                    tracing::info!("EQ preset cleared");
                                }
                            }
                        }
                    }
                });
            }

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
                                    track.genre = genre.clone();
                                    self.now_playing_view.track = Some(track.clone());
                                    self.now_playing_view.genre_edit = genre; // Keep the edited value
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
