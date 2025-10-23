use aaeq_core::{resolve_preset, DeviceController, Mapping, RulesIndex, Scope, TrackMeta};
use aaeq_device_wiim::{WiimController, discover_devices_quick};
use aaeq_persistence::{AppSettingsRepository, CustomEqPresetRepository, GenreOverrideRepository, LastAppliedRepository, MappingRepository, ProfileRepository};
use crate::views::*;
use crate::album_art::AlbumArtCache;
use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use stream_server::{OutputConfig, SampleFormat, LocalDacSink, DlnaSink, OutputManager, AudioBlock, SinkStats, EqProcessor};
use stream_server::dsp::{Dither, DitherMode, NoiseShaping, Resampler, ResamplerQuality};

/// Application mode tabs
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppMode {
    EqManagement,
    DspServer,
    Settings,
}

/// Operating mode selected at startup - determines how the app functions
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OperatingMode {
    NotSelected,      // User hasn't chosen yet
    WiimDevice,       // Use WiiM API for EQ control
    DspProcessor,     // Use DSP mode for EQ processing
}

/// Profile dialog mode
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProfileDialogMode {
    Create,
    Duplicate,
    Rename,
    Delete,
}

/// DSP runtime configuration
#[derive(Clone, Debug)]
struct DspRuntimeConfig {
    dither_enabled: bool,
    dither_mode: DitherMode,
    noise_shaping: NoiseShaping,
    target_bits: u8,
    resample_enabled: bool,
    resample_quality: ResamplerQuality,
    target_sample_rate: u32,
}

/// Error categories for user-friendly error handling
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // Not all error categories are used yet
enum ErrorCategory {
    Connection,     // Network/device connection issues
    Discovery,      // Device discovery failures
    Audio,          // Audio streaming/output issues
    Database,       // Database/persistence errors
    Preset,         // EQ preset related errors
    General,        // Other errors
}

/// Structured error information with helpful context
#[derive(Clone, Debug)]
struct ErrorInfo {
    category: ErrorCategory,
    message: String,
    help_text: String,
    can_retry: bool,
    retry_action: Option<AppCommand>,
}

#[allow(dead_code)] // Not all error helper functions are used yet
impl ErrorInfo {
    /// Create a connection error with retry option
    fn connection_error(host: String) -> Self {
        Self {
            category: ErrorCategory::Connection,
            message: format!("Could not connect to device at {}", host),
            help_text: "Make sure the device is powered on and connected to the same network. Check your router settings to ensure devices can communicate.".to_string(),
            can_retry: true,
            retry_action: Some(AppCommand::ConnectDevice(host)),
        }
    }

    /// Create a discovery error with retry option
    fn discovery_error(error_msg: String) -> Self {
        Self {
            category: ErrorCategory::Discovery,
            message: "Device discovery failed".to_string(),
            help_text: format!("Could not discover devices on the network. Check firewall settings and ensure devices are powered on.\n\nDetails: {}", error_msg),
            can_retry: true,
            retry_action: Some(AppCommand::DiscoverDevices),
        }
    }

    /// Create an audio streaming error
    fn audio_error(device: String, error_msg: String) -> Self {
        Self {
            category: ErrorCategory::Audio,
            message: format!("Could not start audio output to '{}'", device),
            help_text: format!("The audio device may be in use by another application or disconnected.\n\nDetails: {}", error_msg),
            can_retry: false,
            retry_action: None,
        }
    }

    /// Create a database error
    fn database_error(operation: &str, error_msg: String) -> Self {
        Self {
            category: ErrorCategory::Database,
            message: format!("Failed to {}", operation),
            help_text: format!("Could not access the database. The database file may be corrupted or locked by another process.\n\nDetails: {}", error_msg),
            can_retry: true,
            retry_action: None, // Will be set by caller if retry makes sense
        }
    }

    /// Create a preset error
    fn preset_error(preset_name: String, is_wiim_mode: bool) -> Self {
        let help_text = if is_wiim_mode {
            format!("The preset '{}' is not available on your WiiM device. You can create this preset on your device, or map this song to a different preset.", preset_name)
        } else {
            format!("The preset '{}' could not be loaded. It may have been deleted or corrupted.", preset_name)
        };

        Self {
            category: ErrorCategory::Preset,
            message: format!("Preset '{}' not available", preset_name),
            help_text,
            can_retry: false,
            retry_action: None,
        }
    }

    /// Create a general error
    fn general_error(message: String) -> Self {
        Self {
            category: ErrorCategory::General,
            message: message.clone(),
            help_text: "An unexpected error occurred.".to_string(),
            can_retry: false,
            retry_action: None,
        }
    }
}

/// Commands that can be sent from UI to async worker
#[derive(Clone, Debug)]
enum AppCommand {
    ConnectDevice(String),
    DiscoverDevices,
    RefreshPresets,
    ApplyPreset(String),
    SaveMapping(Scope, TrackMeta, String, i64), // (scope, track, preset, profile_id)
    UpdateGenre(String, String), // (track_key, genre)
    BackupDatabase(String), // (db_path)
    RestoreDatabase(String, String), // (backup_zip_path, db_path)
    Poll,
    SaveInputDevice(String), // Save last input device to settings
    SaveOutputDevice(String), // Save last output device to settings
    LoadCustomPresets, // Load custom EQ presets from database
    SaveCustomPreset(aaeq_core::EqPreset), // Save custom EQ preset to database
    EditCustomPreset(String), // Load existing custom preset for editing by name
    DeleteCustomPreset(String), // Delete custom EQ preset by name
    LoadPresetCurve(String), // Load EQ curve for a preset (for display)
    ReloadProfiles, // Reload profiles from database
    ReapplyPresetForCurrentTrack, // Re-resolve and apply preset for current track (for profile switches)
    SaveTheme(String), // Save theme preference to database
    SaveEnableDebugLogging(bool), // Save debug logging preference to database
    SaveDspSettings(aaeq_core::DspSettings), // Save DSP settings for a profile
    SaveDspSinkSettings(aaeq_core::DspSinkSettings), // Save DSP settings for a specific sink type
    // DSP Commands
    DspDiscoverDevices(SinkType, Option<String>), // (sink_type, fallback_ip)
    DspStartStreaming(SinkType, String, OutputConfig, bool, Option<String>, Option<String>, DspRuntimeConfig), // (sink_type, output_device, config, use_test_tone, input_device, preset_name, dsp_config)
    DspStopStreaming,
    DspChangePreset(String), // Change EQ preset during active streaming (loads from library/database)
    DspApplyPresetData(aaeq_core::EqPreset), // Apply preset data directly (for live preview)
    DspUpdateResamplerConfig(bool, ResamplerQuality, u32), // Update resampler during streaming (enabled, quality, target_rate)
}

/// Responses from async worker to UI
enum AppResponse {
    Connected(String, Arc<WiimController>), // (host, device)
    #[allow(dead_code)] // Replaced by ErrorDialog
    ConnectionFailed(String),
    Disconnected(String), // Device went offline during operation
    DevicesDiscovered(Vec<(String, String)>), // Vec<(name, host)>
    PresetsLoaded(Vec<String>),
    PresetApplied(String),
    MappingSaved(String),
    TrackUpdated(TrackMeta, Option<String>),
    BackupCreated(String), // (backup_path)
    DatabaseRestored(String), // (backup_path_used)
    Error(String), // Legacy simple error message
    ErrorDialog(ErrorInfo), // New structured error with help and retry
    DeviceNotFoundAutoDiscover(SinkType, String), // (sink_type, device_name) - Device not in cache, auto-trigger discovery
    // DSP Responses
    DspDevicesDiscovered(SinkType, Vec<String>),
    DspStreamingStarted,
    DspStreamingStopped,
    DspStreamStatus(StreamStatus),
    DspAudioSamples(Vec<f64>), // For visualization
    DspAudioMetrics {
        pre_eq_rms_l: f32,
        pre_eq_rms_r: f32,
        pre_eq_peak_l: f32,
        pre_eq_peak_r: f32,
        post_eq_rms_l: f32,
        post_eq_rms_r: f32,
        post_eq_peak_l: f32,
        post_eq_peak_r: f32,
    },
    CustomPresetsLoaded(Vec<String>),
    CustomPresetSaved(String),
    CustomPresetLoaded(aaeq_core::EqPreset), // Loaded preset for editing
    CustomPresetDeleted(String), // Preset name that was deleted
    PresetCurveLoaded(Option<aaeq_core::EqPreset>), // EQ curve for display
    ProfilesLoaded(Vec<aaeq_core::Profile>), // Reloaded profiles from database
    DspPresetChanged(String), // Preset changed during streaming
    ThemeSaved, // Theme saved to database
    DspSettingsSaved, // DSP settings saved successfully
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
    current_preset_curve: Option<aaeq_core::EqPreset>, // Cached EQ curve for display
    available_presets: Vec<String>,

    /// Mapping rules cache
    rules_index: Arc<RwLock<RulesIndex>>,

    /// Active profile
    active_profile_id: i64,
    available_profiles: Vec<aaeq_core::Profile>,

    /// Polling state
    last_poll: Instant,
    poll_interval: Duration,
    #[allow(dead_code)]
    last_track_key: Option<String>,

    /// UI state
    current_mode: AppMode,
    current_theme: crate::theme::Theme,
    enable_debug_logging: bool,
    show_eq_editor: bool,
    show_delete_confirmation: bool, // Show delete preset confirmation dialog
    preset_to_delete: Option<String>, // Preset name pending deletion
    preset_before_editor: Option<String>, // Track preset before opening editor (for restoration on cancel)
    status_message: Option<String>,
    auto_reconnect: bool,
    connection_lost_time: Option<Instant>,
    reconnect_interval: Duration,
    discovered_devices: Vec<(String, String)>, // Vec<(name, host)>
    show_discovery: bool,
    last_viz_state: bool, // Track previous visualization state for window resizing
    last_viz_mode: crate::views::VisualizationMode, // Track previous visualization mode for window resizing
    last_meters_state: bool, // Track previous meters state for window resizing
    last_collapsed_state: bool, // Track previous collapsed state for window resizing
    show_profile_dialog: bool,
    profile_dialog_mode: ProfileDialogMode,
    profile_name_input: String,
    profile_icon_input: String,
    profile_color_input: String,
    profile_to_duplicate: Option<i64>,
    profile_to_rename: Option<i64>,
    profile_to_delete: Option<i64>,

    /// Error dialog state
    show_error_dialog: bool,
    current_error: Option<ErrorInfo>,

    /// Async communication
    command_tx: mpsc::UnboundedSender<AppCommand>,
    response_rx: mpsc::UnboundedReceiver<AppResponse>,

    /// Album art cache
    album_art_cache: Arc<AlbumArtCache>,
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
            current_preset_curve: None,
            available_presets: vec![],
            rules_index,
            active_profile_id: 1, // Default profile
            available_profiles: vec![],
            last_poll: Instant::now(),
            poll_interval: Duration::from_millis(1000),
            last_track_key: None,
            current_mode: AppMode::EqManagement,
            current_theme: crate::theme::Theme::default(), // Will be loaded in initialize()
            enable_debug_logging: false, // Will be loaded in initialize()
            show_eq_editor: false,
            show_delete_confirmation: false,
            preset_to_delete: None,
            preset_before_editor: None,
            status_message: None,
            auto_reconnect: true, // Enable by default
            connection_lost_time: None,
            reconnect_interval: Duration::from_secs(5),
            discovered_devices: vec![],
            show_discovery: false,
            last_viz_state: false,
            last_viz_mode: crate::views::VisualizationMode::Waveform,
            last_meters_state: false,
            last_collapsed_state: false,
            show_profile_dialog: false,
            profile_dialog_mode: ProfileDialogMode::Create,
            profile_name_input: String::new(),
            profile_icon_input: "📁".to_string(), // Default folder icon
            profile_color_input: "#4A90E2".to_string(), // Default blue
            profile_to_duplicate: None,
            profile_to_rename: None,
            profile_to_delete: None,
            show_error_dialog: false,
            current_error: None,
            command_tx,
            response_rx,
            album_art_cache: Arc::new(AlbumArtCache::new()),
        }
    }

    /// Initialize the app (load mappings, connect to device)
    pub async fn initialize(&mut self) -> Result<()> {
        // Load profiles from database
        let profile_repo = ProfileRepository::new(self.pool.clone());
        self.available_profiles = profile_repo.list_all().await.unwrap_or_default();
        tracing::info!("Loaded {} profiles", self.available_profiles.len());

        // Load active profile from settings
        let settings_repo = AppSettingsRepository::new(self.pool.clone());
        if let Ok(Some(active_id)) = settings_repo.get_active_profile_id().await {
            self.active_profile_id = active_id;
            tracing::info!("Loaded active profile ID: {}", active_id);
        }

        // Load DSP settings for the active profile
        use aaeq_persistence::DspSettingsRepository;
        let dsp_repo = DspSettingsRepository::new(self.pool.clone());
        match dsp_repo.get_by_profile(self.active_profile_id).await {
            Ok(Some(settings)) => {
                tracing::info!("Loaded DSP settings for active profile: {:?}", settings);
                self.dsp_view.sample_rate = settings.sample_rate;
                self.dsp_view.buffer_ms = settings.buffer_ms;
                self.dsp_view.headroom_db = settings.headroom_db;
                self.dsp_view.auto_compensate = settings.auto_compensate;
                self.dsp_view.clip_detection = settings.clip_detection;

                // Load dithering settings
                self.dsp_view.dither_enabled = settings.dither_enabled;
                self.dsp_view.target_bits = settings.target_bits;

                // Parse dither mode from string
                use stream_server::dsp::DitherMode;
                self.dsp_view.dither_mode = match settings.dither_mode.as_str() {
                    "None" => DitherMode::None,
                    "Rectangular" => DitherMode::Rectangular,
                    "Triangular" => DitherMode::Triangular,
                    "Gaussian" => DitherMode::Gaussian,
                    _ => DitherMode::Triangular, // Default to TPDF if unknown
                };

                // Parse noise shaping from string
                use stream_server::dsp::NoiseShaping;
                self.dsp_view.noise_shaping = match settings.noise_shaping.as_str() {
                    "None" => NoiseShaping::None,
                    "FirstOrder" => NoiseShaping::FirstOrder,
                    "SecondOrder" => NoiseShaping::SecondOrder,
                    "Gesemann" => NoiseShaping::Gesemann,
                    _ => NoiseShaping::None, // Default to None if unknown
                };

                // Load resampling settings
                self.dsp_view.resample_enabled = settings.resample_enabled;
                self.dsp_view.target_sample_rate = settings.target_sample_rate;

                // Parse resample quality from string
                use stream_server::dsp::ResamplerQuality;
                self.dsp_view.resample_quality = match settings.resample_quality.as_str() {
                    "Fast" => ResamplerQuality::Fast,
                    "Balanced" => ResamplerQuality::Balanced,
                    "High" => ResamplerQuality::High,
                    "Ultra" => ResamplerQuality::Ultra,
                    _ => ResamplerQuality::Balanced, // Default to Balanced if unknown
                };
            }
            Ok(None) => {
                tracing::info!("No DSP settings found for active profile, using defaults");
            }
            Err(e) => {
                tracing::error!("Failed to load DSP settings: {}", e);
            }
        }

        // Load DSP sink settings for the current sink type
        use aaeq_persistence::DspSinkSettingsRepository;
        let sink_repo = DspSinkSettingsRepository::new(self.pool.clone());
        let sink_type_str = self.dsp_view.selected_sink.to_db_string();
        match sink_repo.get_by_sink_type(sink_type_str).await {
            Ok(Some(sink_settings)) => {
                tracing::info!("Loaded DSP sink settings for {}: sample_rate={}, format={}, buffer_ms={}, headroom_db={}",
                    sink_settings.sink_type, sink_settings.sample_rate, sink_settings.format,
                    sink_settings.buffer_ms, sink_settings.headroom_db);

                // Apply sink-specific settings (these override profile settings for sample_rate, buffer_ms, headroom_db)
                self.dsp_view.sample_rate = sink_settings.sample_rate;
                self.dsp_view.buffer_ms = sink_settings.buffer_ms;
                self.dsp_view.headroom_db = sink_settings.headroom_db;

                // Parse format from string
                use crate::views::FormatOption;
                self.dsp_view.format = match sink_settings.format.as_str() {
                    "F32" => FormatOption::F32,
                    "S24LE" => FormatOption::S24LE,
                    "S16LE" => FormatOption::S16LE,
                    _ => FormatOption::F32, // Default to F32 if unknown
                };
            }
            Ok(None) => {
                tracing::info!("No DSP sink settings found for {}, using defaults", sink_type_str);
            }
            Err(e) => {
                tracing::error!("Failed to load DSP sink settings: {}", e);
            }
        }

        // Load mappings from database for the active profile
        self.reload_mappings().await?;

        // Load auto-reconnect setting first (we need this before deciding to connect)
        match settings_repo.get_auto_reconnect().await {
            Ok(Some(auto_reconnect)) => {
                tracing::info!("Loading saved auto-reconnect setting: {}", auto_reconnect);
                self.auto_reconnect = auto_reconnect;
            }
            Ok(None) => {
                // No setting saved yet, use default (true) and save it
                tracing::info!("No auto-reconnect setting found, using default: true");
                self.auto_reconnect = true;
                if let Err(e) = settings_repo.set_auto_reconnect(true).await {
                    tracing::error!("Failed to save default auto-reconnect setting: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to load auto-reconnect setting: {}, using default: true", e);
                self.auto_reconnect = true;
            }
        }

        // Load last connected host from settings
        if let Ok(Some(last_host)) = settings_repo.get_last_connected_host().await {
            tracing::info!("Loading last connected host: {}", last_host);
            self.device_host = last_host.clone();

            // Only try to connect if auto-reconnect is enabled
            if self.auto_reconnect {
                tracing::info!("Auto-reconnect enabled, connecting to last device: {}", last_host);
                let _ = self.command_tx.send(AppCommand::ConnectDevice(last_host));
            } else {
                tracing::info!("Auto-reconnect disabled, not connecting to last device");
            }
        }

        // Load last input device from settings
        if let Ok(Some(last_input)) = settings_repo.get_last_input_device().await {
            tracing::info!("Loading last input device: {}", last_input);
            self.dsp_view.selected_input_device = Some(last_input);
        }

        // Load last output device from settings
        if let Ok(Some(last_output)) = settings_repo.get_last_output_device().await {
            tracing::info!("Loading last output device: {}", last_output);
            self.dsp_view.selected_device = Some(last_output);
        }

        // Load theme from settings
        if let Ok(Some(theme_str)) = settings_repo.get_theme().await {
            if let Some(theme) = crate::theme::Theme::from_str(&theme_str) {
                tracing::info!("Loading saved theme: {}", theme_str);
                self.current_theme = theme;
            }
        }

        // Load debug logging setting
        if let Ok(enabled) = settings_repo.get_enable_debug_logging().await {
            self.enable_debug_logging = enabled;
            tracing::info!("Debug logging enabled: {}", enabled);
        }

        // Trigger device discovery on startup for Local DAC (fast, populates device lists)
        tracing::info!("Triggering automatic device discovery on startup");
        let _ = self.command_tx.send(AppCommand::DspDiscoverDevices(SinkType::LocalDac, None));

        // Load custom EQ presets
        let _ = self.command_tx.send(AppCommand::LoadCustomPresets);

        Ok(())
    }

    /// Reload mapping rules from database
    async fn reload_mappings(&mut self) -> Result<()> {
        let repo = MappingRepository::new(self.pool.clone());
        let mappings = repo.list_by_profile(self.active_profile_id).await?;

        let mut rules = self.rules_index.write().await;
        *rules = RulesIndex::from_mappings(mappings);

        tracing::info!("Loaded {} song rules, {} album rules, {} genre rules for profile {}",
            rules.song_rules.len(), rules.album_rules.len(), rules.genre_rules.len(), self.active_profile_id);

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
                self.dsp_view.current_active_preset = Some(desired_preset.clone()); // Sync to DSP view
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
        self.dsp_view.current_active_preset = self.current_preset.clone(); // Sync to DSP view for pipeline

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
            profile_id: self.active_profile_id,
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
        let mut last_track: Option<aaeq_core::TrackMeta> = None;
        let mut current_preset: Option<String> = None;

        // DSP state
        let mut output_manager = OutputManager::new();
        let mut streaming_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut stream_shutdown_tx: Option<mpsc::Sender<()>> = None;
        let mut stream_preset_change_tx: Option<mpsc::Sender<String>> = None;
        let mut stream_preset_data_tx: Option<mpsc::Sender<aaeq_core::EqPreset>> = None;
        let mut stream_resampler_config_tx: Option<mpsc::Sender<(bool, ResamplerQuality, u32)>> = None;
        let mut dsp_is_streaming = false;

        // DLNA device cache
        let mut discovered_dlna_devices: Vec<stream_server::sinks::dlna::DlnaDevice> = Vec::new();

        // AirPlay device cache
        let mut discovered_airplay_devices: Vec<stream_server::sinks::airplay::AirPlayDevice> = Vec::new();

        while let Some(cmd) = command_rx.recv().await {
            // Log all commands for debugging profile switching
            if matches!(cmd, AppCommand::ReapplyPresetForCurrentTrack) {
                tracing::info!(">>> Received ReapplyPresetForCurrentTrack command in async worker");
            }

            match cmd {
                AppCommand::ConnectDevice(host) => {
                    let controller = WiimController::new("WiiM Device", host.clone());
                    if controller.is_online().await {
                        tracing::info!("Connected to device at {}", host);
                        let device_arc = Arc::new(controller);
                        device = Some(device_arc.clone());

                        // Save the successful connection to settings
                        let settings_repo = AppSettingsRepository::new(pool.clone());
                        if let Err(e) = settings_repo.set_last_connected_host(&host).await {
                            tracing::error!("Failed to save last connected host: {}", e);
                        }

                        let _ = response_tx.send(AppResponse::Connected(host, device_arc));
                    } else {
                        tracing::warn!("Device at {} is offline", host);
                        let error_info = ErrorInfo::connection_error(host.clone());
                        let _ = response_tx.send(AppResponse::ErrorDialog(error_info));
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
                            let error_info = ErrorInfo::discovery_error(e.to_string());
                            let _ = response_tx.send(AppResponse::ErrorDialog(error_info));
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
                                // Preset doesn't exist on WiiM device, try fallback to Flat
                                tracing::warn!("Preset '{}' not available on device: {}. Falling back to 'Flat'", preset, e);

                                match dev.apply_preset("Flat").await {
                                    Ok(_) => {
                                        current_preset = Some("Flat".to_string());
                                        let _ = response_tx.send(AppResponse::Error(
                                            format!("Preset '{}' not available in WiiM mode, using 'Flat'", preset)
                                        ));
                                    }
                                    Err(e2) => {
                                        tracing::error!("Failed to apply fallback 'Flat' preset: {}", e2);
                                        let _ = response_tx.send(AppResponse::Error(
                                            format!("Failed to apply preset: {}", e2)
                                        ));
                                    }
                                }
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

                AppCommand::SaveMapping(scope, track, preset, profile_id) => {
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
                        profile_id,
                        created_at: chrono::Utc::now().timestamp(),
                        updated_at: chrono::Utc::now().timestamp(),
                    };

                    let repo = MappingRepository::new(pool.clone());
                    match repo.upsert(&mapping).await {
                        Ok(_) => {
                            // Reload rules index for the active profile
                            match repo.list_by_profile(profile_id).await {
                                Ok(mappings) => {
                                    let mut rules = rules_index.write().await;
                                    *rules = RulesIndex::from_mappings(mappings);
                                    tracing::info!("Reloaded rules for profile {}: {} song rules, {} album rules, {} genre rules",
                                        profile_id, rules.song_rules.len(), rules.album_rules.len(), rules.genre_rules.len());
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

                AppCommand::SaveOutputDevice(device_name) => {
                    let settings_repo = AppSettingsRepository::new(pool.clone());
                    if let Err(e) = settings_repo.set_last_output_device(&device_name).await {
                        tracing::error!("Failed to save last output device: {}", e);
                    } else {
                        tracing::info!("Saved last output device: {}", device_name);
                    }
                }

                AppCommand::SaveTheme(theme_str) => {
                    let settings_repo = AppSettingsRepository::new(pool.clone());
                    if let Err(e) = settings_repo.set_theme(&theme_str).await {
                        tracing::error!("Failed to save theme: {}", e);
                        let _ = response_tx.send(AppResponse::Error(format!("Failed to save theme: {}", e)));
                    } else {
                        tracing::info!("Saved theme: {}", theme_str);
                        let _ = response_tx.send(AppResponse::ThemeSaved);
                    }
                }
                AppCommand::SaveEnableDebugLogging(enabled) => {
                    let settings_repo = AppSettingsRepository::new(pool.clone());
                    if let Err(e) = settings_repo.set_enable_debug_logging(enabled).await {
                        tracing::error!("Failed to save debug logging setting: {}", e);
                    } else {
                        tracing::info!("Debug logging setting saved: {}", enabled);
                    }
                }

                AppCommand::SaveDspSettings(settings) => {
                    tracing::info!("📥 Received SaveDspSettings command for profile {}: dither_enabled={}, mode={}, shaping={}, bits={}",
                        settings.profile_id, settings.dither_enabled, settings.dither_mode, settings.noise_shaping, settings.target_bits);
                    use aaeq_persistence::DspSettingsRepository;
                    let dsp_repo = DspSettingsRepository::new(pool.clone());
                    match dsp_repo.upsert(&settings).await {
                        Ok(_) => {
                            tracing::info!("✅ Successfully saved DSP settings to database for profile {}", settings.profile_id);
                            let _ = response_tx.send(AppResponse::DspSettingsSaved);
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to save DSP settings: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to save DSP settings: {}", e)));
                        }
                    }
                }

                AppCommand::SaveDspSinkSettings(settings) => {
                    tracing::info!("📥 Saving DSP sink settings for {}: sample_rate={}, format={}, buffer_ms={}, headroom_db={}",
                        settings.sink_type, settings.sample_rate, settings.format, settings.buffer_ms, settings.headroom_db);
                    use aaeq_persistence::DspSinkSettingsRepository;
                    let sink_repo = DspSinkSettingsRepository::new(pool.clone());
                    match sink_repo.upsert(&settings).await {
                        Ok(_) => {
                            tracing::info!("✅ Successfully saved DSP sink settings for {}", settings.sink_type);
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to save DSP sink settings: {}", e);
                        }
                    }
                }

                AppCommand::LoadCustomPresets => {
                    let custom_repo = CustomEqPresetRepository::new(pool.clone());
                    match custom_repo.list_names().await {
                        Ok(presets) => {
                            tracing::info!("Loaded {} custom presets", presets.len());
                            let _ = response_tx.send(AppResponse::CustomPresetsLoaded(presets));
                        }
                        Err(e) => {
                            tracing::error!("Failed to load custom presets: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to load custom presets: {}", e)));
                        }
                    }
                }

                AppCommand::SaveCustomPreset(preset) => {
                    let custom_repo = CustomEqPresetRepository::new(pool.clone());
                    match custom_repo.upsert(&preset).await {
                        Ok(_) => {
                            tracing::info!("Saved custom preset: {}", preset.name);
                            let _ = response_tx.send(AppResponse::CustomPresetSaved(preset.name.clone()));
                            // Reload custom presets list
                            if let Ok(presets) = custom_repo.list_names().await {
                                let _ = response_tx.send(AppResponse::CustomPresetsLoaded(presets));
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to save custom preset: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to save custom preset: {}", e)));
                        }
                    }
                }

                AppCommand::EditCustomPreset(preset_name) => {
                    let custom_repo = CustomEqPresetRepository::new(pool.clone());
                    match custom_repo.get_by_name(&preset_name).await {
                        Ok(Some(preset)) => {
                            tracing::info!("Loaded custom preset for editing: {}", preset_name);
                            let _ = response_tx.send(AppResponse::CustomPresetLoaded(preset));
                        }
                        Ok(None) => {
                            tracing::warn!("Custom preset not found: {}", preset_name);
                            let _ = response_tx.send(AppResponse::Error(format!("Preset '{}' not found", preset_name)));
                        }
                        Err(e) => {
                            tracing::error!("Failed to load custom preset: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to load preset: {}", e)));
                        }
                    }
                }

                AppCommand::DeleteCustomPreset(preset_name) => {
                    let custom_repo = CustomEqPresetRepository::new(pool.clone());
                    let mapping_repo = MappingRepository::new(pool.clone());

                    match custom_repo.delete(&preset_name).await {
                        Ok(_) => {
                            tracing::info!("Deleted custom preset: {}", preset_name);

                            // Update all song mappings referencing this preset to use "Flat"
                            match mapping_repo.update_preset_references(&preset_name, "Flat").await {
                                Ok(count) => {
                                    if count > 0 {
                                        tracing::info!("Updated {} song mapping(s) from '{}' to 'Flat'", count, preset_name);
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to update song mappings after deleting preset '{}': {}", preset_name, e);
                                }
                            }

                            let _ = response_tx.send(AppResponse::CustomPresetDeleted(preset_name.clone()));
                            // Reload custom presets list
                            if let Ok(presets) = custom_repo.list_names().await {
                                let _ = response_tx.send(AppResponse::CustomPresetsLoaded(presets));
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to delete custom preset: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to delete preset: {}", e)));
                        }
                    }
                }

                AppCommand::LoadPresetCurve(preset_name) => {
                    // Load EQ curve for display using the database-aware function
                    let curve = crate::preset_library::get_preset_curve_with_db(&preset_name, &pool).await;
                    let _ = response_tx.send(AppResponse::PresetCurveLoaded(curve));
                }

                AppCommand::ReloadProfiles => {
                    let profile_repo = ProfileRepository::new(pool.clone());
                    match profile_repo.list_all().await {
                        Ok(profiles) => {
                            let _ = response_tx.send(AppResponse::ProfilesLoaded(profiles));
                        }
                        Err(e) => {
                            tracing::error!("Failed to reload profiles: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to reload profiles: {}", e)));
                        }
                    }
                }

                AppCommand::ReapplyPresetForCurrentTrack => {
                    // Re-resolve preset for the current track with the newly loaded rules
                    if let Some(track) = &last_track {
                        tracing::info!("Re-resolving preset for current track after profile switch: {} - {}", track.artist, track.title);
                        tracing::info!("Current preset before switch: {:?}", current_preset);
                        tracing::info!("DSP streaming: {}", dsp_is_streaming);

                        // Resolve preset with the new rules (from the switched profile)
                        let rules = rules_index.read().await;
                        let desired_preset = resolve_preset(track, &rules, "Flat");
                        drop(rules);

                        tracing::info!("Profile switch resolved preset: {} (current: {:?})", desired_preset, current_preset);

                        // Always apply the preset on profile switch (even if it's the same name,
                        // it could be a different profile's mapping)
                        tracing::info!("Applying preset after profile switch: {}", desired_preset);

                        // If DSP is streaming, change preset there
                        if dsp_is_streaming {
                            if let Some(preset_tx) = &stream_preset_change_tx {
                                tracing::info!("Sending preset change to DSP stream: {}", desired_preset);
                                match preset_tx.send(desired_preset.clone()).await {
                                    Ok(_) => {
                                        current_preset = Some(desired_preset.clone());
                                        let _ = response_tx.send(AppResponse::DspPresetChanged(desired_preset.clone()));
                                        tracing::info!("Successfully sent preset change to DSP");
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to send preset change to DSP: {}", e);
                                    }
                                }
                            } else {
                                tracing::warn!("DSP is streaming but no preset_tx available!");
                            }
                        } else if let Some(dev) = &device {
                            // Use WiiM API
                            tracing::info!("Applying preset via WiiM API: {}", desired_preset);
                            match dev.apply_preset(&desired_preset).await {
                                Ok(_) => {
                                    current_preset = Some(desired_preset.clone());
                                    tracing::info!("Successfully applied preset via WiiM API");
                                }
                                Err(e) => {
                                    // Preset doesn't exist on WiiM device, try fallback to Flat
                                    tracing::warn!("Preset '{}' not available on device: {}. Falling back to 'Flat'", desired_preset, e);

                                    match dev.apply_preset("Flat").await {
                                        Ok(_) => {
                                            current_preset = Some("Flat".to_string());
                                            let _ = response_tx.send(AppResponse::Error(
                                                format!("Preset '{}' not available in WiiM mode, using 'Flat'", desired_preset)
                                            ));
                                        }
                                        Err(e2) => {
                                            tracing::error!("Failed to apply fallback 'Flat' preset: {}", e2);
                                            let _ = response_tx.send(AppResponse::Error(
                                                format!("Failed to apply preset: {}", e2)
                                            ));
                                        }
                                    }
                                }
                            }
                        } else {
                            tracing::warn!("No DSP stream or device available to apply preset");
                        }
                    } else {
                        tracing::warn!("No track available to re-apply preset for");
                    }
                }

                AppCommand::RestoreDatabase(backup_zip_path, db_path) => {
                    use std::fs;
                    use std::io::{Read, Write};

                    // Step 1: Create a backup of the current database before restoring
                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    let pre_restore_backup = format!("aaeq-pre-restore_{}.zip", timestamp);
                    let backup_path = std::path::Path::new(&backup_zip_path)
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join(&pre_restore_backup);

                    // Create pre-restore backup
                    match fs::copy(&db_path, backup_path.with_extension("db.tmp")) {
                        Ok(_) => {
                            let zip_file = match fs::File::create(&backup_path) {
                                Ok(f) => f,
                                Err(e) => {
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to create pre-restore backup: {}", e)));
                                    continue;
                                }
                            };

                            let mut zip = zip::ZipWriter::new(zip_file);
                            let options = zip::write::FileOptions::<()>::default()
                                .compression_method(zip::CompressionMethod::Deflated)
                                .compression_level(Some(6));

                            if let Err(e) = zip.start_file("aaeq.db", options) {
                                let _ = response_tx.send(AppResponse::Error(format!("Failed to start zip file: {}", e)));
                                continue;
                            }

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

                            let _ = fs::remove_file(backup_path.with_extension("db.tmp"));
                            tracing::info!("Pre-restore backup created: {}", backup_path.display());

                            // Step 2: Extract and restore from the backup zip
                            let backup_file = match fs::File::open(&backup_zip_path) {
                                Ok(f) => f,
                                Err(e) => {
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to open backup file: {}", e)));
                                    continue;
                                }
                            };

                            let mut zip_archive = match zip::ZipArchive::new(backup_file) {
                                Ok(archive) => archive,
                                Err(e) => {
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to read backup zip: {}", e)));
                                    continue;
                                }
                            };

                            // Find and extract the database file
                            let mut db_file = match zip_archive.by_name("aaeq.db") {
                                Ok(file) => file,
                                Err(e) => {
                                    let _ = response_tx.send(AppResponse::Error(format!("Backup file doesn't contain aaeq.db: {}", e)));
                                    continue;
                                }
                            };

                            let mut db_content = Vec::new();
                            if let Err(e) = db_file.read_to_end(&mut db_content) {
                                let _ = response_tx.send(AppResponse::Error(format!("Failed to read database from backup: {}", e)));
                                continue;
                            }

                            // Write the restored database
                            match fs::write(&db_path, &db_content) {
                                Ok(_) => {
                                    tracing::info!("Database restored from: {}", backup_zip_path);
                                    let _ = response_tx.send(AppResponse::DatabaseRestored(backup_zip_path.clone()));
                                }
                                Err(e) => {
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to restore database: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to create pre-restore backup: {}", e);
                            let _ = response_tx.send(AppResponse::Error(format!("Failed to create pre-restore backup: {}", e)));
                        }
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
                                    // This is our DSP stream, get real metadata from media session
                                    tracing::debug!("Detected DSP stream, checking media session for real track info");
                                    match crate::media::get_now_playing() {
                                        Ok(media_track) => {
                                            track = media_track;
                                            tracing::debug!("Using media session track: {} - {}", track.artist, track.title);
                                        }
                                        Err(e) => {
                                            tracing::debug!("Media session not available: {}", e);
                                            // Keep the WiiM track (will show "AAEQ Stream")
                                        }
                                    }
                                }

                                let track_key = track.track_key();

                                // Store device genre before applying override (always do this on every poll)
                                track.device_genre = track.genre.clone();

                                // Load genre override if exists (check on every poll, not just on track change)
                                let genre_repo = GenreOverrideRepository::new(pool.clone());
                                if let Ok(Some(genre_override)) = genre_repo.get(&track_key).await {
                                    track.genre = genre_override;
                                }

                                // Check if track changed
                                if last_track_key.as_deref() != Some(&track_key) {
                                    tracing::info!("Track changed: {} - {}", track.artist, track.title);

                                    // Resolve preset
                                    let rules = rules_index.read().await;
                                    let desired_preset = resolve_preset(&track, &rules, "Flat");
                                    drop(rules);

                                    // Apply if different from current
                                    if current_preset.as_deref() != Some(&desired_preset) {
                                        tracing::info!("Applying preset: {}", desired_preset);

                                        // If DSP is streaming, change preset there instead of via WiiM API
                                        if dsp_is_streaming {
                                            if let Some(preset_tx) = &stream_preset_change_tx {
                                                match preset_tx.send(desired_preset.clone()).await {
                                                    Ok(_) => {
                                                        current_preset = Some(desired_preset.clone());
                                                        let _ = response_tx.send(AppResponse::DspPresetChanged(desired_preset.clone()));
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to send preset change to DSP: {}", e);
                                                    }
                                                }
                                            }
                                        } else {
                                            // Use WiiM API
                                            match dev.apply_preset(&desired_preset).await {
                                                Ok(_) => {
                                                    current_preset = Some(desired_preset.clone());

                                                    // Save to database
                                                    if let Some(dev_id) = device_id {
                                                        let repo = LastAppliedRepository::new(pool.clone());
                                                        let _ = repo.update(dev_id, &track_key, &desired_preset).await;
                                                    }
                                                }
                                                Err(e) => {
                                                    // Preset doesn't exist on WiiM device, try fallback to Flat
                                                    tracing::warn!("Preset '{}' not available on device: {}. Falling back to 'Flat'", desired_preset, e);

                                                    match dev.apply_preset("Flat").await {
                                                        Ok(_) => {
                                                            current_preset = Some("Flat".to_string());

                                                            // Save Flat to database
                                                            if let Some(dev_id) = device_id {
                                                                let repo = LastAppliedRepository::new(pool.clone());
                                                                let _ = repo.update(dev_id, &track_key, "Flat").await;
                                                            }

                                                            let _ = response_tx.send(AppResponse::Error(
                                                                format!("Preset '{}' not available in WiiM mode, using 'Flat'", desired_preset)
                                                            ));
                                                        }
                                                        Err(e2) => {
                                                            tracing::error!("Failed to apply fallback 'Flat' preset: {}", e2);
                                                            let _ = response_tx.send(AppResponse::Error(
                                                                format!("Failed to apply preset: {}", e2)
                                                            ));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    last_track_key = Some(track_key.clone());
                                }

                                // Always update last_track, even if the track hasn't changed
                                // This ensures we have track metadata available for profile switches
                                last_track = Some(track.clone());

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
                        // No WiiM device connected, try media session for DSP mode
                        match crate::media::get_now_playing() {
                            Ok(mut track) => {
                                let track_key = track.track_key();

                                // Store device genre before applying override (always do this on every poll)
                                track.device_genre = track.genre.clone();

                                // Load genre override if exists (check on every poll, not just on track change)
                                let genre_repo = GenreOverrideRepository::new(pool.clone());
                                if let Ok(Some(genre_override)) = genre_repo.get(&track_key).await {
                                    track.genre = genre_override;
                                }

                                // Check if track changed
                                if last_track_key.as_deref() != Some(&track_key) {
                                    tracing::info!("Track changed (MPRIS): {} - {}", track.artist, track.title);

                                    // Resolve preset based on rules
                                    let rules = rules_index.read().await;
                                    let desired_preset = resolve_preset(&track, &rules, "Flat");
                                    drop(rules);

                                    // If DSP is streaming and preset changed, apply it automatically
                                    if dsp_is_streaming && current_preset.as_deref() != Some(&desired_preset) {
                                        tracing::info!("Auto-applying preset in DSP mode: {}", desired_preset);

                                        if let Some(preset_tx) = &stream_preset_change_tx {
                                            match preset_tx.send(desired_preset.clone()).await {
                                                Ok(_) => {
                                                    current_preset = Some(desired_preset.clone());
                                                    let _ = response_tx.send(AppResponse::DspPresetChanged(desired_preset.clone()));
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to send preset change: {}", e);
                                                }
                                            }
                                        }
                                    } else {
                                        // Not streaming, just update the current preset for display
                                        current_preset = Some(desired_preset.clone());
                                        tracing::info!("Preset for track (DSP mode): {}", desired_preset);
                                    }

                                    last_track_key = Some(track_key.clone());
                                }

                                // Always update last_track, even if the track hasn't changed
                                // This ensures we have track metadata available for profile switches
                                last_track = Some(track.clone());

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
                                    let _ = response_tx.send(AppResponse::DspDevicesDiscovered(SinkType::LocalDac, devices));
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
                            let _ = response_tx.send(AppResponse::DspDevicesDiscovered(SinkType::Dlna, device_names));
                        }
                        SinkType::AirPlay => {
                            // Discover AirPlay devices
                            use stream_server::AirPlaySink;

                            match AirPlaySink::discover(10).await { // 10 second timeout
                                Ok(devices) => {
                                    // Cache the discovered AirPlay devices for later use
                                    discovered_airplay_devices = devices.clone();
                                    tracing::info!("Cached {} AirPlay device(s)", discovered_airplay_devices.len());

                                    let device_names: Vec<String> = devices.iter().map(|d| d.name.clone()).collect();
                                    tracing::info!("Found {} AirPlay devices", device_names.len());
                                    let _ = response_tx.send(AppResponse::DspDevicesDiscovered(SinkType::AirPlay, device_names));
                                }
                                Err(e) => {
                                    tracing::error!("Failed to discover AirPlay devices: {}", e);
                                    let _ = response_tx.send(AppResponse::Error(format!("Failed to discover AirPlay devices: {}", e)));
                                }
                            }
                        }
                    }
                }

                AppCommand::DspStartStreaming(sink_type, device_name, config, use_test_tone, input_device, preset_name, mut dsp_config) => {
                    tracing::info!("Starting DSP streaming: {:?} to device '{}' (test_tone: {}, input: {:?}, preset: {:?}, dither: {})",
                        sink_type, device_name, use_test_tone, input_device, preset_name, dsp_config.dither_enabled);

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

                    // Wait a moment for port to be released
                    tokio::time::sleep(Duration::from_millis(100)).await;

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
                                // Auto-trigger discovery instead of just showing error
                                let _ = response_tx.send(AppResponse::DeviceNotFoundAutoDiscover(
                                    SinkType::Dlna,
                                    device_name.clone()
                                ));
                                Err(format!("Device '{}' not found in cache. Starting auto-discovery...", device_name))
                            }
                        }
                        SinkType::AirPlay => {
                            use stream_server::AirPlaySink;

                            // Find the AirPlay device by name from cache
                            if let Some(device) = discovered_airplay_devices.iter().find(|d| d.name == device_name) {
                                let mut sink = AirPlaySink::new();
                                sink.set_device(device.clone());
                                output_manager.register_sink(Box::new(sink));
                                output_manager.select_sink(0, config.clone()).await
                                    .map_err(|e| format!("Failed to open AirPlay sink: {}", e))
                            } else {
                                tracing::error!("AirPlay device '{}' not found in cache. Available devices: {:?}",
                                    device_name,
                                    discovered_airplay_devices.iter().map(|d| &d.name).collect::<Vec<_>>());
                                // Auto-trigger discovery instead of just showing error
                                let _ = response_tx.send(AppResponse::DeviceNotFoundAutoDiscover(
                                    SinkType::AirPlay,
                                    device_name.clone()
                                ));
                                Err(format!("Device '{}' not found in cache. Starting auto-discovery...", device_name))
                            }
                        }
                    };

                    match sink_result {
                        Ok(_) => {
                            tracing::info!("Sink opened successfully, starting streaming task");

                            // Create shutdown channel for the streaming task
                            let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
                            stream_shutdown_tx = Some(shutdown_tx);

                            // Create preset change channel for the streaming task
                            let (preset_change_tx, mut preset_change_rx) = mpsc::channel::<String>(8);
                            stream_preset_change_tx = Some(preset_change_tx);

                            // Create preset data channel for live preview (sends full preset, not just name)
                            let (preset_data_tx, mut preset_data_rx) = mpsc::channel::<aaeq_core::EqPreset>(8);
                            stream_preset_data_tx = Some(preset_data_tx);

                            // Create resampler config channel for live updates
                            let (resampler_config_tx, mut resampler_config_rx) = mpsc::channel::<(bool, ResamplerQuality, u32)>(8);
                            stream_resampler_config_tx = Some(resampler_config_tx);

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

                                // Helper function to calculate RMS and peak from interleaved stereo samples
                                let calculate_metrics = |samples: &[f64]| -> (f32, f32, f32, f32) {
                                    let mut sum_sq_l = 0.0f64;
                                    let mut sum_sq_r = 0.0f64;
                                    let mut peak_l = 0.0f32;
                                    let mut peak_r = 0.0f32;
                                    let frame_count = samples.len() / channels;

                                    for i in 0..frame_count {
                                        let l = samples[i * channels];
                                        let r = samples[i * channels + 1];

                                        sum_sq_l += l * l;
                                        sum_sq_r += r * r;

                                        peak_l = peak_l.max(l.abs() as f32);
                                        peak_r = peak_r.max(r.abs() as f32);
                                    }

                                    let rms_l = (sum_sq_l / frame_count as f64).sqrt() as f32;
                                    let rms_r = (sum_sq_r / frame_count as f64).sqrt() as f32;

                                    // Convert to dBFS
                                    let rms_dbfs_l = if rms_l > 0.0 { 20.0 * rms_l.log10() } else { -120.0 };
                                    let rms_dbfs_r = if rms_r > 0.0 { 20.0 * rms_r.log10() } else { -120.0 };
                                    let peak_dbfs_l = if peak_l > 0.0 { 20.0 * peak_l.log10() } else { -120.0 };
                                    let peak_dbfs_r = if peak_r > 0.0 { 20.0 * peak_r.log10() } else { -120.0 };

                                    (rms_dbfs_l, rms_dbfs_r, peak_dbfs_l, peak_dbfs_r)
                                };

                                // Helper function to load a preset from library or database
                                let load_preset_curve = |preset_name: &str| -> Option<aaeq_core::EqPreset> {
                                    crate::preset_library::get_preset_curve(preset_name)
                                        .or_else(|| {
                                            let custom_repo = CustomEqPresetRepository::new(_pool_for_task.clone());
                                            let rt = tokio::runtime::Handle::current();
                                            rt.block_on(async {
                                                match custom_repo.get_by_name(preset_name).await {
                                                    Ok(Some(custom_preset)) => {
                                                        tracing::info!("Loaded custom preset: {}", preset_name);
                                                        Some(custom_preset)
                                                    }
                                                    Ok(None) => {
                                                        tracing::warn!("Preset '{}' not found", preset_name);
                                                        None
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to load preset: {}", e);
                                                        None
                                                    }
                                                }
                                            })
                                        })
                                };

                                // Initialize EQ processor if preset is provided
                                let mut eq_processor = EqProcessor::new(sample_rate, channels);
                                if let Some(ref preset_name) = preset_name {
                                    tracing::info!("Loading EQ preset: {}", preset_name);
                                    if let Some(preset) = load_preset_curve(preset_name) {
                                        eq_processor.load_preset(&preset);
                                        tracing::info!("EQ preset loaded: {} ({} bands)", preset_name, eq_processor.band_count());
                                    }
                                }

                                // Initialize Dither processor (currently unused - see NOTE below)
                                let _dither = Dither::new(
                                    dsp_config.dither_mode,
                                    dsp_config.noise_shaping,
                                    dsp_config.target_bits,
                                );
                                tracing::info!("Dither processor initialized but disabled (dithering happens in format conversion): mode={:?}, shaping={:?}, bits={}",
                                    dsp_config.dither_mode, dsp_config.noise_shaping, dsp_config.target_bits);

                                // Initialize Resampler processor
                                let mut resampler = Resampler::new(
                                    dsp_config.resample_quality,
                                    sample_rate,
                                    dsp_config.target_sample_rate,
                                    channels,
                                ).expect("Failed to create resampler");
                                tracing::info!("Resampler initialized: enabled={}, quality={:?}, {} Hz -> {} Hz",
                                    dsp_config.resample_enabled, dsp_config.resample_quality, sample_rate, dsp_config.target_sample_rate);

                                // CPU usage tracking - average over last 10 samples
                                // TODO: Currently disabled, will revisit later
                                let _cpu_samples: std::collections::VecDeque<f32> = std::collections::VecDeque::with_capacity(10);

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

                                // Send initial status update immediately for auto-delay detection
                                {
                                    let mgr = manager.read().await;
                                    let latency = mgr.active_sink_latency().unwrap_or(0);
                                    let stats = mgr.active_sink_stats()
                                        .unwrap_or(SinkStats::default());

                                    // Initial CPU is 0 (no samples processed yet)
                                    let initial_cpu = 0.0;

                                    // Get DSP latency from resampler
                                    let dsp_latency = if dsp_config.resample_enabled {
                                        resampler.latency_ms()
                                    } else {
                                        0.0
                                    };

                                    let status = StreamStatus {
                                        latency_ms: latency,
                                        frames_written: frame_count,
                                        underruns: stats.underruns,
                                        buffer_fill: stats.buffer_fill,
                                        cpu_usage: initial_cpu,
                                        dsp_latency_ms: dsp_latency,
                                    };

                                    tracing::info!("Sending initial stream status: latency={} ms", latency);
                                    let _ = tx.send(AppResponse::DspStreamStatus(status));
                                }

                                loop {
                                    tokio::select! {
                                        _ = shutdown_rx.recv() => {
                                            tracing::info!("Streaming task received shutdown signal");
                                            break;
                                        }
                                        Some(new_preset_name) = preset_change_rx.recv() => {
                                            tracing::info!("Preset change requested: {}", new_preset_name);
                                            if let Some(preset) = load_preset_curve(&new_preset_name) {
                                                eq_processor.load_preset(&preset);
                                                tracing::info!("EQ preset changed to: {} ({} bands)", new_preset_name, eq_processor.band_count());
                                            } else {
                                                tracing::warn!("Failed to load preset: {}", new_preset_name);
                                            }
                                        }
                                        Some(preset_data) = preset_data_rx.recv() => {
                                            tracing::info!("Direct preset data received: {} ({} bands)", preset_data.name, preset_data.bands.len());
                                            eq_processor.load_preset(&preset_data);
                                            tracing::info!("Live EQ preview applied");
                                        }
                                        Some((enabled, quality, target_rate)) = resampler_config_rx.recv() => {
                                            tracing::info!("Resampler config update: enabled={}, quality={:?}, target_rate={}", enabled, quality, target_rate);

                                            // Update dsp_config
                                            dsp_config.resample_enabled = enabled;
                                            dsp_config.resample_quality = quality;
                                            dsp_config.target_sample_rate = target_rate;

                                            // Recreate resampler with new settings
                                            match Resampler::new(quality, sample_rate, target_rate, channels) {
                                                Ok(new_resampler) => {
                                                    resampler = new_resampler;
                                                    tracing::info!("Resampler updated successfully: {} Hz -> {} Hz, quality={:?}",
                                                        sample_rate, target_rate, quality);
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to update resampler: {}", e);
                                                }
                                            }
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

                                            // TODO: CPU usage calculation - timing disabled for now
                                            // let dsp_start = std::time::Instant::now();
                                            // let _original_sample_count = captured_samples.len() / channels;

                                            // Calculate pre-EQ metrics
                                            let (pre_rms_l, pre_rms_r, pre_peak_l, pre_peak_r) = calculate_metrics(&captured_samples);

                                            // Apply EQ processing
                                            eq_processor.process(&mut captured_samples);

                                            // Apply resampling (after EQ, before dither)
                                            if dsp_config.resample_enabled {
                                                captured_samples = resampler.process(&captured_samples)
                                                    .expect("Failed to resample audio");
                                            }

                                            // NOTE: Dithering is applied during format conversion in convert_format()
                                            // Applying it here in the float domain creates audible artifacts
                                            // The DSP dither processor is kept for potential future use with
                                            // proper integration, but disabled for now
                                            // if dsp_config.dither_enabled {
                                            //     dither.process(&mut captured_samples);
                                            // }

                                            // Calculate post-EQ metrics
                                            let (post_rms_l, post_rms_r, post_peak_l, post_peak_r) = calculate_metrics(&captured_samples);

                                            // Send samples for visualization (need 2048 for FFT)
                                            let viz_samples: Vec<f64> = captured_samples.iter()
                                                .step_by(channels) // Take every Nth sample (left channel)
                                                .take(2048) // Need enough samples for spectrum analyzer FFT
                                                .copied()
                                                .collect();
                                            let _ = tx.send(AppResponse::DspAudioSamples(viz_samples));

                                            // Create audio block from captured samples
                                            // Use target sample rate if resampling is enabled, otherwise use original rate
                                            let output_sample_rate = if dsp_config.resample_enabled {
                                                dsp_config.target_sample_rate
                                            } else {
                                                sample_rate
                                            };
                                            let block = AudioBlock::new(&captured_samples, output_sample_rate, channels as u16);

                                            // Write to sink
                                            let mut mgr = manager.write().await;
                                            if let Err(e) = mgr.write(block).await {
                                                tracing::error!("Failed to write audio block: {}", e);
                                                break;
                                            }

                                            frame_count += (captured_samples.len() / channels) as u64;

                                            // TODO: CPU usage calculation - revisit later
                                            // Currently shows 0%, needs investigation
                                            // let dsp_elapsed = dsp_start.elapsed();
                                            // let audio_duration_secs = original_sample_count as f64 / sample_rate as f64;
                                            // let dsp_usage = (dsp_elapsed.as_secs_f64() / audio_duration_secs * 100.0) as f32;
                                            // cpu_samples.push_back(dsp_usage);
                                            // if cpu_samples.len() > 10 {
                                            //     cpu_samples.pop_front();
                                            // }

                                            // Send audio metrics for every audio block (for meters)
                                            let _ = tx.send(AppResponse::DspAudioMetrics {
                                                pre_eq_rms_l: pre_rms_l,
                                                pre_eq_rms_r: pre_rms_r,
                                                pre_eq_peak_l: pre_peak_l,
                                                pre_eq_peak_r: pre_peak_r,
                                                post_eq_rms_l: post_rms_l,
                                                post_eq_rms_r: post_rms_r,
                                                post_eq_peak_l: post_peak_l,
                                                post_eq_peak_r: post_peak_r,
                                            });

                                            // Send status update periodically (every ~100ms)
                                            if frame_count % (sample_rate as u64 / 10) == 0 {
                                                let latency = mgr.active_sink_latency().unwrap_or(0);
                                                let stats = mgr.active_sink_stats()
                                                    .unwrap_or(SinkStats::default());

                                                // Calculate average CPU usage
                                                // TODO: CPU calculation commented out for now
                                                let avg_cpu = 0.0;
                                                // let avg_cpu = if !cpu_samples.is_empty() {
                                                //     cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32
                                                // } else {
                                                //     0.0
                                                // };

                                                // Get DSP latency from resampler
                                                let dsp_latency = if dsp_config.resample_enabled {
                                                    resampler.latency_ms()
                                                } else {
                                                    0.0
                                                };

                                                let status = StreamStatus {
                                                    latency_ms: latency,
                                                    frames_written: frame_count,
                                                    underruns: stats.underruns,
                                                    buffer_fill: stats.buffer_fill,
                                                    cpu_usage: avg_cpu,
                                                    dsp_latency_ms: dsp_latency,
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

                                            // TODO: CPU usage calculation - timing disabled for now
                                            // let dsp_start = std::time::Instant::now();

                                            // Calculate pre-EQ metrics
                                            let (pre_rms_l, pre_rms_r, pre_peak_l, pre_peak_r) = calculate_metrics(&audio_data);

                                            // Apply EQ processing
                                            eq_processor.process(&mut audio_data);

                                            // Apply resampling (after EQ, before dither)
                                            if dsp_config.resample_enabled {
                                                audio_data = resampler.process(&audio_data)
                                                    .expect("Failed to resample audio");
                                            }

                                            // NOTE: Dithering is applied during format conversion in convert_format()
                                            // Applying it here in the float domain creates audible artifacts
                                            // if dsp_config.dither_enabled {
                                            //     dither.process(&mut audio_data);
                                            // }

                                            // Calculate post-EQ metrics
                                            let (post_rms_l, post_rms_r, post_peak_l, post_peak_r) = calculate_metrics(&audio_data);

                                            // Send samples for visualization (need 2048 for FFT)
                                            let viz_samples: Vec<f64> = audio_data.iter()
                                                .step_by(channels)
                                                .take(2048) // Need enough samples for spectrum analyzer FFT
                                                .copied()
                                                .collect();
                                            let _ = tx.send(AppResponse::DspAudioSamples(viz_samples));

                                            // Use target sample rate if resampling is enabled, otherwise use original rate
                                            let output_sample_rate = if dsp_config.resample_enabled {
                                                dsp_config.target_sample_rate
                                            } else {
                                                sample_rate
                                            };
                                            let block = AudioBlock::new(&audio_data, output_sample_rate, channels as u16);

                                            // Write to sink
                                            let mut mgr = manager.write().await;
                                            if let Err(e) = mgr.write(block).await {
                                                tracing::error!("Failed to write audio block: {}", e);
                                                break;
                                            }

                                            frame_count += frames_per_block as u64;

                                            // TODO: CPU usage calculation - revisit later
                                            // Currently shows 0%, needs investigation
                                            // let dsp_elapsed = dsp_start.elapsed();
                                            // let audio_duration_secs = frames_per_block as f64 / sample_rate as f64;
                                            // let dsp_usage = (dsp_elapsed.as_secs_f64() / audio_duration_secs * 100.0) as f32;
                                            // cpu_samples.push_back(dsp_usage);
                                            // if cpu_samples.len() > 10 {
                                            //     cpu_samples.pop_front();
                                            // }

                                            // Send audio metrics for every audio block (for meters)
                                            let _ = tx.send(AppResponse::DspAudioMetrics {
                                                pre_eq_rms_l: pre_rms_l,
                                                pre_eq_rms_r: pre_rms_r,
                                                pre_eq_peak_l: pre_peak_l,
                                                pre_eq_peak_r: pre_peak_r,
                                                post_eq_rms_l: post_rms_l,
                                                post_eq_rms_r: post_rms_r,
                                                post_eq_peak_l: post_peak_l,
                                                post_eq_peak_r: post_peak_r,
                                            });

                                            // Send status update periodically (every ~100ms)
                                            if frame_count % (sample_rate as u64 / 10) == 0 {
                                                let latency = mgr.active_sink_latency().unwrap_or(0);
                                                let stats = mgr.active_sink_stats()
                                                    .unwrap_or(SinkStats::default());

                                                // Calculate average CPU usage
                                                // TODO: CPU calculation commented out for now
                                                let avg_cpu = 0.0;
                                                // let avg_cpu = if !cpu_samples.is_empty() {
                                                //     cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32
                                                // } else {
                                                //     0.0
                                                // };

                                                // Get DSP latency from resampler
                                                let dsp_latency = if dsp_config.resample_enabled {
                                                    resampler.latency_ms()
                                                } else {
                                                    0.0
                                                };

                                                let status = StreamStatus {
                                                    latency_ms: latency,
                                                    frames_written: frame_count,
                                                    underruns: stats.underruns,
                                                    buffer_fill: stats.buffer_fill,
                                                    cpu_usage: avg_cpu,
                                                    dsp_latency_ms: dsp_latency,
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
                            dsp_is_streaming = true;
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

                    // Clear preset change channels
                    stream_preset_change_tx = None;
                    stream_preset_data_tx = None;
                    stream_resampler_config_tx = None;
                    dsp_is_streaming = false;

                    let _ = response_tx.send(AppResponse::DspStreamingStopped);
                    tracing::info!("DSP streaming stopped successfully");
                }

                AppCommand::DspChangePreset(preset_name) => {
                    tracing::info!("Changing DSP preset to: {}", preset_name);

                    if let Some(preset_tx) = &stream_preset_change_tx {
                        match preset_tx.send(preset_name.clone()).await {
                            Ok(_) => {
                                current_preset = Some(preset_name.clone());
                                let _ = response_tx.send(AppResponse::DspPresetChanged(preset_name));
                            }
                            Err(e) => {
                                tracing::error!("Failed to send preset change to streaming task: {}", e);
                                let _ = response_tx.send(AppResponse::Error(format!("Failed to change preset: {}", e)));
                            }
                        }
                    } else {
                        tracing::warn!("Cannot change preset - no active streaming session");
                        let _ = response_tx.send(AppResponse::Error("No active streaming session".to_string()));
                    }
                }

                AppCommand::DspApplyPresetData(preset) => {
                    tracing::info!("Applying live preset data: {} ({} bands)", preset.name, preset.bands.len());

                    if let Some(preset_data_tx) = &stream_preset_data_tx {
                        match preset_data_tx.send(preset.clone()).await {
                            Ok(_) => {
                                tracing::info!("Live preset data sent to streaming task");
                            }
                            Err(e) => {
                                tracing::error!("Failed to send preset data to streaming task: {}", e);
                            }
                        }
                    } else {
                        tracing::warn!("Cannot apply preset data - no active streaming session");
                    }
                }

                AppCommand::DspUpdateResamplerConfig(enabled, quality, target_rate) => {
                    tracing::info!("Updating resampler config: enabled={}, quality={:?}, target_rate={}", enabled, quality, target_rate);

                    if let Some(resampler_tx) = &stream_resampler_config_tx {
                        match resampler_tx.send((enabled, quality, target_rate)).await {
                            Ok(_) => {
                                tracing::info!("Resampler config update sent to streaming task");
                            }
                            Err(e) => {
                                tracing::error!("Failed to send resampler config to streaming task: {}", e);
                            }
                        }
                    } else {
                        tracing::debug!("Cannot update resampler - no active streaming session");
                    }
                }
            }
        }
    }

    /// Save DSP sink settings to database when user changes them
    fn save_dsp_sink_settings(&self) {
        let format_str = match self.dsp_view.format {
            FormatOption::F32 => "F32",
            FormatOption::S24LE => "S24LE",
            FormatOption::S16LE => "S16LE",
        };

        let settings = aaeq_core::DspSinkSettings {
            id: None,
            sink_type: self.dsp_view.selected_sink.to_db_string().to_string(),
            sample_rate: self.dsp_view.sample_rate,
            format: format_str.to_string(),
            buffer_ms: self.dsp_view.buffer_ms,
            headroom_db: self.dsp_view.headroom_db,
            created_at: 0,
            updated_at: 0,
        };

        tracing::debug!("Saving DSP sink settings for {}: sample_rate={}, format={}, buffer_ms={}, headroom_db={}",
            settings.sink_type, settings.sample_rate, settings.format, settings.buffer_ms, settings.headroom_db);

        let _ = self.command_tx.send(AppCommand::SaveDspSinkSettings(settings));
    }

    /// Auto-save DSP settings to database when user changes them
    fn auto_save_dsp_settings(&self) {
        let settings = aaeq_core::DspSettings {
            id: None,
            profile_id: self.active_profile_id,
            sample_rate: self.dsp_view.sample_rate,
            buffer_ms: self.dsp_view.buffer_ms,
            headroom_db: self.dsp_view.headroom_db,
            auto_compensate: self.dsp_view.auto_compensate,
            clip_detection: self.dsp_view.clip_detection,
            dither_enabled: self.dsp_view.dither_enabled,
            dither_mode: self.dsp_view.dither_mode.as_str().to_string(),
            noise_shaping: self.dsp_view.noise_shaping.as_str().to_string(),
            target_bits: self.dsp_view.target_bits,
            resample_enabled: self.dsp_view.resample_enabled,
            resample_quality: self.dsp_view.resample_quality.as_str().to_string(),
            target_sample_rate: self.dsp_view.target_sample_rate,
            created_at: 0,
            updated_at: 0,
        };

        tracing::debug!("Auto-saving DSP settings: dither={}, resample={}, target_rate={}",
            settings.dither_enabled, settings.resample_enabled, settings.target_sample_rate);

        let _ = self.command_tx.send(AppCommand::SaveDspSettings(settings));
    }
}

impl eframe::App for AaeqApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply current theme
        ctx.set_visuals(self.current_theme.to_visuals());

        // Handle window close button (X) - hide to tray instead of quitting
        // Note: This only intercepts the close button (X), not the minimize button.
        // Minimize button works normally and sends window to taskbar/dock.
        if ctx.input(|i| i.viewport().close_requested()) {
            tracing::info!("Close button clicked - hiding to tray (minimize button still works normally)");
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        // Process responses from async worker
        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                AppResponse::Connected(host, device_arc) => {
                    self.device = Some(device_arc);
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
                    self.dsp_view.current_active_preset = None; // Sync to DSP view
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
                    self.presets_view.presets = presets.clone();
                }
                AppResponse::PresetApplied(preset) => {
                    self.current_preset = Some(preset.clone());
                    self.dsp_view.current_active_preset = Some(preset.clone()); // Sync to DSP view
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

                    // Check if preset changed
                    let preset_changed = self.current_preset != preset;

                    // Only update genre_edit if track changed (to avoid overwriting user edits)
                    if track_changed {
                        // When track changes, reset genre_edit to match new track
                        self.now_playing_view.genre_edit = track.genre.clone();
                    }
                    // If track didn't change but genre changed (e.g., override was applied),
                    // update genre_edit only if it matches the old genre (preserve user edits in textbox)
                    else if self.current_track.as_ref().map(|t| t.genre.clone()) != Some(track.genre.clone()) {
                        // Genre changed on same track (likely from override being applied)
                        // Only update if genre_edit matches the old genre (not a pending user edit)
                        if let Some(old_track) = &self.current_track {
                            if self.now_playing_view.genre_edit == old_track.genre {
                                self.now_playing_view.genre_edit = track.genre.clone();
                            }
                        }
                    }

                    self.current_track = Some(track.clone());
                    self.current_preset = preset;
                    self.dsp_view.current_active_preset = self.current_preset.clone(); // Sync to DSP view
                    self.now_playing_view.track = Some(track.clone());
                    self.now_playing_view.current_preset = self.current_preset.clone();

                    // Load preset curve if preset changed
                    if preset_changed {
                        if let Some(preset_name) = &self.current_preset {
                            let _ = self.command_tx.send(AppCommand::LoadPresetCurve(preset_name.clone()));
                        } else {
                            self.current_preset_curve = None;
                            self.now_playing_view.current_preset_curve = None;
                        }
                    }
                }
                AppResponse::BackupCreated(path) => {
                    self.status_message = Some(format!("Backup created: {}", path));
                }
                AppResponse::DatabaseRestored(path) => {
                    self.status_message = Some(format!("Database restored from: {}", path));
                }
                AppResponse::Error(msg) => {
                    self.status_message = Some(format!("Error: {}", msg));
                    // Clear any pending "starting" state on error
                    self.dsp_view.is_starting = false;
                }
                AppResponse::ErrorDialog(error_info) => {
                    // Show structured error dialog with help and retry options
                    self.current_error = Some(error_info);
                    self.show_error_dialog = true;
                    // Clear any pending "starting" state on error
                    self.dsp_view.is_starting = false;
                }
                AppResponse::DeviceNotFoundAutoDiscover(sink_type, device_name) => {
                    tracing::info!("Device '{}' not found - auto-triggering discovery for {:?}", device_name, sink_type);
                    self.status_message = Some(format!("Device '{}' not found. Discovering devices...", device_name));
                    // Clear starting state
                    self.dsp_view.is_starting = false;
                    // Trigger discovery automatically
                    self.dsp_view.show_device_discovery = true;
                    self.dsp_view.discovering = true;
                    let fallback_ip = if sink_type == SinkType::Dlna {
                        Some(self.device_host.clone())
                    } else {
                        None
                    };
                    let _ = self.command_tx.send(AppCommand::DspDiscoverDevices(sink_type, fallback_ip));
                }
                // DSP Responses
                AppResponse::DspDevicesDiscovered(sink_type, devices) => {
                    // Store devices in the appropriate list based on sink type
                    match sink_type {
                        SinkType::LocalDac => {
                            self.dsp_view.available_local_devices = devices.clone();
                        }
                        SinkType::Dlna => {
                            self.dsp_view.available_dlna_devices = devices.clone();
                        }
                        SinkType::AirPlay => {
                            self.dsp_view.available_airplay_devices = devices.clone();
                        }
                    }
                    // Also update legacy list for backwards compatibility
                    self.dsp_view.available_devices = devices.clone();
                    self.dsp_view.discovering = false;
                    self.status_message = Some(format!("Found {} device(s)", devices.len()));
                }
                AppResponse::DspStreamingStarted => {
                    self.dsp_view.is_streaming = true;
                    self.dsp_view.is_starting = false; // Hide spinner, streaming is active
                    self.status_message = Some("Streaming started".to_string());
                }
                AppResponse::DspStreamingStopped => {
                    self.dsp_view.is_streaming = false;
                    self.dsp_view.stream_status = None;
                    self.dsp_view.clear_buffers(); // Clear visualization buffers when stopping
                    self.dsp_view.reset_auto_delay(); // Reset auto-detection for next session

                    // Check if we need to auto-restart due to resampling settings change
                    if self.dsp_view.needs_restart {
                        tracing::info!("Auto-restarting stream after resampling settings change");
                        self.dsp_view.needs_restart = false;

                        // Restart streaming with new settings
                        let format = match self.dsp_view.format {
                            FormatOption::F32 => SampleFormat::F32,
                            FormatOption::S24LE => SampleFormat::S24LE,
                            FormatOption::S16LE => SampleFormat::S16LE,
                        };

                        // Use target sample rate if resampling is enabled, otherwise use input rate
                        let output_sample_rate = if self.dsp_view.resample_enabled {
                            self.dsp_view.target_sample_rate
                        } else {
                            self.dsp_view.sample_rate
                        };

                        let config = OutputConfig {
                            sample_rate: output_sample_rate,
                            channels: 2,
                            format,
                            buffer_ms: self.dsp_view.buffer_ms,
                            exclusive: false,
                        };

                        if let Some(device) = &self.dsp_view.selected_device {
                            let dsp_config = DspRuntimeConfig {
                                dither_enabled: self.dsp_view.dither_enabled,
                                dither_mode: self.dsp_view.dither_mode,
                                noise_shaping: self.dsp_view.noise_shaping,
                                target_bits: self.dsp_view.target_bits,
                                resample_enabled: self.dsp_view.resample_enabled,
                                resample_quality: self.dsp_view.resample_quality,
                                target_sample_rate: self.dsp_view.target_sample_rate,
                            };
                            let _ = self.command_tx.send(AppCommand::DspStartStreaming(
                                self.dsp_view.selected_sink,
                                device.clone(),
                                config,
                                self.dsp_view.use_test_tone,
                                self.dsp_view.selected_input_device.clone(),
                                None,
                                dsp_config,
                            ));
                            self.dsp_view.is_starting = true;
                            self.status_message = Some("Restarting stream with new settings...".to_string());
                        }
                    } else {
                        self.status_message = Some("Streaming stopped".to_string());
                    }
                }
                AppResponse::DspStreamStatus(status) => {
                    // Try automatic delay detection on first status update
                    if self.dsp_view.try_auto_detect_delay(&status) {
                        tracing::info!("Automatically set visualization delay to {} ms based on stream latency", self.dsp_view.viz_delay_ms);
                        self.dsp_view.clear_buffers(); // Clear buffers when auto-setting delay
                    }
                    self.dsp_view.stream_status = Some(status);
                }
                AppResponse::DspAudioSamples(samples) => {
                    // Buffer samples for delayed visualization (for network streaming sync)
                    self.dsp_view.buffer_samples(samples);
                }
                AppResponse::DspAudioMetrics {
                    pre_eq_rms_l, pre_eq_rms_r, pre_eq_peak_l, pre_eq_peak_r,
                    post_eq_rms_l, post_eq_rms_r, post_eq_peak_l, post_eq_peak_r
                } => {
                    // Buffer metrics for delayed visualization (for network streaming sync)
                    let metrics = crate::views::VizMetrics {
                        pre_eq_rms_l,
                        pre_eq_rms_r,
                        pre_eq_peak_l,
                        pre_eq_peak_r,
                        post_eq_rms_l,
                        post_eq_rms_r,
                        post_eq_peak_l,
                        post_eq_peak_r,
                    };
                    self.dsp_view.buffer_metrics(metrics);
                }
                AppResponse::CustomPresetsLoaded(presets) => {
                    self.presets_view.custom_presets = presets.clone();
                    self.now_playing_view.custom_presets = presets;
                }
                AppResponse::CustomPresetSaved(preset_name) => {
                    self.status_message = Some(format!("Saved custom preset: {}", preset_name));
                }
                AppResponse::CustomPresetLoaded(preset) => {
                    // Open EQ editor with loaded preset in edit mode
                    let preset_name = preset.name.clone();
                    let mut editor = EqEditorView::new_for_edit(preset);
                    editor.set_existing_presets(self.presets_view.custom_presets.clone());
                    self.eq_editor_view = Some(editor);
                    self.show_eq_editor = true;
                    self.status_message = Some(format!("Editing preset: {}", preset_name));
                }
                AppResponse::CustomPresetDeleted(preset_name) => {
                    tracing::info!("Preset '{}' deleted. Current preset: {:?}, DSP streaming: {}",
                        preset_name, self.current_preset, self.dsp_view.is_streaming);

                    self.status_message = Some(format!("Deleted preset: {}", preset_name));

                    // If this preset is currently active, revert to Flat EQ
                    let is_active = self.current_preset.as_ref() == Some(&preset_name);

                    if is_active {
                        tracing::info!("Deleted preset '{}' is currently active", preset_name);

                        // DSP mode: apply flat preset directly
                        if self.dsp_view.is_streaming {
                            tracing::info!("Deleted preset '{}' was active during DSP streaming - reverting to Flat EQ", preset_name);
                            let flat_preset = aaeq_core::EqPreset::default();
                            let _ = self.command_tx.send(AppCommand::DspApplyPresetData(flat_preset));
                            self.current_preset = None;
                            self.dsp_view.current_active_preset = None; // Sync to DSP view
                            self.current_preset_curve = Some(aaeq_core::EqPreset::default());
                            self.now_playing_view.current_preset = None;
                            self.now_playing_view.current_preset_curve = Some(aaeq_core::EqPreset::default());
                            self.status_message = Some(format!("Deleted preset '{}' - reverted to Flat EQ", preset_name));
                        }
                        // WiiM mode: apply Flat preset via device API
                        else if self.device.is_some() {
                            tracing::info!("Deleted preset '{}' was active on WiiM device - reverting to Flat", preset_name);
                            let _ = self.command_tx.send(AppCommand::ApplyPreset("Flat".to_string()));
                            self.current_preset = Some("Flat".to_string());
                            self.dsp_view.current_active_preset = Some("Flat".to_string()); // Sync to DSP view
                            self.now_playing_view.current_preset = Some("Flat".to_string());
                            // Load Flat curve for display
                            let _ = self.command_tx.send(AppCommand::LoadPresetCurve("Flat".to_string()));
                            self.status_message = Some(format!("Deleted preset '{}' - reverted to Flat", preset_name));
                        } else {
                            tracing::warn!("Deleted preset '{}' was active but neither DSP nor WiiM mode is active", preset_name);
                        }
                    } else {
                        tracing::info!("Deleted preset '{}' was not active, no need to revert", preset_name);
                    }

                    // If deleted preset was selected, clear selection
                    if self.presets_view.selected_preset.as_ref() == Some(&preset_name) {
                        self.presets_view.selected_preset = None;
                    }
                }
                AppResponse::PresetCurveLoaded(curve) => {
                    self.current_preset_curve = curve;
                    self.now_playing_view.current_preset_curve = self.current_preset_curve.clone();
                }
                AppResponse::ProfilesLoaded(profiles) => {
                    self.available_profiles = profiles;
                    // Close the dialog and clear inputs
                    self.show_profile_dialog = false;
                    self.profile_name_input.clear();
                    self.profile_to_rename = None;
                    self.profile_to_delete = None;
                }
                AppResponse::DspPresetChanged(preset) => {
                    self.current_preset = Some(preset.clone());
                    self.dsp_view.current_active_preset = Some(preset.clone()); // Sync to DSP view
                    self.now_playing_view.current_preset = Some(preset.clone());
                    self.status_message = Some(format!("DSP preset changed: {}", preset));
                    // Load the EQ curve for display
                    let _ = self.command_tx.send(AppCommand::LoadPresetCurve(preset));
                }
                AppResponse::ThemeSaved => {
                    self.status_message = Some("Theme saved".to_string());
                }
                AppResponse::DspSettingsSaved => {
                    self.status_message = Some("DSP settings saved successfully".to_string());
                }
            }
        }

        // Process buffered visualization data (for network streaming sync)
        self.dsp_view.process_buffers();

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

                // Show Connect or Disconnect button based on connection state
                if self.device.is_some() {
                    if ui.button("Disconnect").clicked() {
                        tracing::info!("Disconnecting from WiiM device");
                        self.device = None;
                        self.status_message = Some("Disconnected".to_string());
                    }
                } else if ui.button("Connect").clicked() {
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

                // Profile selector
                ui.label("Profile:");
                let current_profile = self.available_profiles
                    .iter()
                    .find(|p| p.id == Some(self.active_profile_id));

                let current_profile_text = if let Some(prof) = current_profile {
                    format!("{} {}", prof.icon, prof.name)
                } else {
                    "Default".to_string()
                };

                egui::ComboBox::from_id_salt("profile_selector")
                    .selected_text(current_profile_text)
                    .show_ui(ui, |ui| {
                        for profile in &self.available_profiles.clone() {
                            if let Some(profile_id) = profile.id {
                                let is_selected = profile_id == self.active_profile_id;

                                // Parse profile color
                                let profile_color = if profile.color.starts_with('#') && profile.color.len() == 7 {
                                    let r = u8::from_str_radix(&profile.color[1..3], 16).unwrap_or(128);
                                    let g = u8::from_str_radix(&profile.color[3..5], 16).unwrap_or(128);
                                    let b = u8::from_str_radix(&profile.color[5..7], 16).unwrap_or(128);
                                    egui::Color32::from_rgb(r, g, b)
                                } else {
                                    egui::Color32::GRAY
                                };

                                ui.horizontal(|ui| {
                                    // Color indicator dot
                                    let size = egui::Vec2::new(12.0, 12.0);
                                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                    if ui.is_rect_visible(rect) {
                                        ui.painter().circle_filled(rect.center(), 6.0, profile_color);
                                    }

                                    // Icon and name
                                    let label_text = format!("{} {}", profile.icon, profile.name);
                                    if ui.selectable_label(is_selected, label_text).clicked() {
                                        self.active_profile_id = profile_id;

                                        // Save active profile to settings
                                        let pool = self.pool.clone();
                                        tokio::spawn(async move {
                                            let settings_repo = AppSettingsRepository::new(pool);
                                            let _ = settings_repo.set_active_profile_id(profile_id).await;
                                        });

                                        // Reload mappings for the new profile
                                        tracing::info!("Profile switched in UI, reloading mappings for profile {}", self.active_profile_id);
                                        let pool = self.pool.clone();
                                        let rules_index = self.rules_index.clone();
                                        let profile_id_for_reload = self.active_profile_id;
                                        let command_tx = self.command_tx.clone();
                                        tokio::spawn(async move {
                                            tracing::info!("Starting async task to reload mappings for profile {}", profile_id_for_reload);
                                            let repo = MappingRepository::new(pool);
                                            match repo.list_by_profile(profile_id_for_reload).await {
                                                Ok(mappings) => {
                                                    tracing::info!("Loaded {} mappings for profile {}", mappings.len(), profile_id_for_reload);
                                                    let mut rules = rules_index.write().await;
                                                    *rules = RulesIndex::from_mappings(mappings);
                                                    tracing::info!("Switched to profile {}, loaded {} song rules, {} album rules, {} genre rules",
                                                        profile_id_for_reload, rules.song_rules.len(), rules.album_rules.len(), rules.genre_rules.len());
                                                    drop(rules); // Release lock before sending command

                                                    // Trigger re-resolution of current track with new rules
                                                    tracing::info!("Sending ReapplyPresetForCurrentTrack command...");
                                                    match command_tx.send(AppCommand::ReapplyPresetForCurrentTrack) {
                                                        Ok(_) => tracing::info!("Successfully sent ReapplyPresetForCurrentTrack command"),
                                                        Err(e) => tracing::error!("Failed to send ReapplyPresetForCurrentTrack command: {}", e),
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to load mappings for profile {}: {}", profile_id_for_reload, e);
                                                }
                                            }
                                        });

                                        self.status_message = Some(format!("Switched to profile: {}", profile.name));
                                    }

                                    // Show duplicate/rename/delete buttons for non-builtin profiles
                                    if !profile.is_builtin {
                                        if ui.small_button("📋").on_hover_text("Duplicate profile").clicked() {
                                            self.show_profile_dialog = true;
                                            self.profile_dialog_mode = ProfileDialogMode::Duplicate;
                                            self.profile_name_input = format!("{} Copy", profile.name);
                                            self.profile_icon_input = profile.icon.clone();
                                            self.profile_color_input = profile.color.clone();
                                            self.profile_to_duplicate = Some(profile_id);
                                        }

                                        if ui.small_button("✏").on_hover_text("Edit profile").clicked() {
                                            self.show_profile_dialog = true;
                                            self.profile_dialog_mode = ProfileDialogMode::Rename;
                                            self.profile_name_input = profile.name.clone();
                                            self.profile_icon_input = profile.icon.clone();
                                            self.profile_color_input = profile.color.clone();
                                            self.profile_to_rename = Some(profile_id);
                                        }

                                        if ui.small_button("🗑").on_hover_text("Delete profile").clicked() {
                                            self.show_profile_dialog = true;
                                            self.profile_dialog_mode = ProfileDialogMode::Delete;
                                            self.profile_to_delete = Some(profile_id);
                                            self.profile_name_input = profile.name.clone();
                                        }
                                    }
                                });
                            }
                        }

                        ui.separator();
                        if ui.button("+ Add Profile").clicked() {
                            self.show_profile_dialog = true;
                            self.profile_dialog_mode = ProfileDialogMode::Create;
                            self.profile_name_input.clear();
                        }
                    });

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

                    // Save auto-reconnect setting to database
                    let pool = self.pool.clone();
                    let auto_reconnect = self.auto_reconnect;
                    tokio::spawn(async move {
                        let settings_repo = AppSettingsRepository::new(pool);
                        if let Err(e) = settings_repo.set_auto_reconnect(auto_reconnect).await {
                            tracing::error!("Failed to save auto-reconnect setting: {}", e);
                        } else {
                            tracing::info!("Saved auto-reconnect setting: {}", auto_reconnect);
                        }
                    });
                }
            });
        });

        // Tab bar for mode selection
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_mode, AppMode::EqManagement, "EQ Management")
                    .on_hover_text("Manage EQ presets and mappings for WiiM device");
                ui.selectable_value(&mut self.current_mode, AppMode::DspServer, "DSP Server")
                    .on_hover_text("Stream audio with DSP processing to various outputs");
                ui.selectable_value(&mut self.current_mode, AppMode::Settings, "Settings")
                    .on_hover_text("Application settings and preferences");
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
                                    ui.label(name);
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

        // Error dialog with retry and help buttons
        if self.show_error_dialog {
            if let Some(error) = &self.current_error.clone() {
                let error_title = match error.category {
                    ErrorCategory::Connection => "⚠ Connection Error",
                    ErrorCategory::Discovery => "⚠ Discovery Error",
                    ErrorCategory::Audio => "⚠ Audio Error",
                    ErrorCategory::Database => "⚠ Database Error",
                    ErrorCategory::Preset => "⚠ Preset Error",
                    ErrorCategory::General => "⚠ Error",
                };

                // Clone data before entering closure
                let error_message = error.message.clone();
                let error_help_text = error.help_text.clone();
                let error_can_retry = error.can_retry;
                let error_retry_action = error.retry_action.clone();

                egui::Window::new(error_title)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.set_min_width(400.0);

                        // Error message
                        ui.label(
                            egui::RichText::new(&error_message)
                                .size(14.0)
                                .strong()
                        );

                        ui.add_space(10.0);

                        // Help text
                        ui.label(
                            egui::RichText::new(&error_help_text)
                                .size(12.0)
                                .color(egui::Color32::GRAY)
                        );

                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(10.0);

                        // Buttons
                        ui.horizontal(|ui| {
                            // Retry button (if applicable)
                            if error_can_retry && ui.button("🔄 Retry").clicked() {
                                if let Some(retry_cmd) = error_retry_action {
                                    let _ = self.command_tx.send(retry_cmd);
                                }
                                self.show_error_dialog = false;
                                self.current_error = None;
                            }

                            // Close button
                            if ui.button("Close").clicked() {
                                self.show_error_dialog = false;
                                self.current_error = None;
                            }
                        });
                    });
            }
        }

        // Main content based on current mode
        match self.current_mode {
            AppMode::EqManagement => {
                // EQ Management Mode: Show presets panel + now playing
                if self.show_eq_editor {
                    // Show EQ editor
                    if let Some(editor) = &mut self.eq_editor_view {
                        if let Some(action) = editor.show(ctx) {
                            match action {
                                EqEditorAction::Save(preset) => {
                                    tracing::info!("Saving custom preset: {}", preset.name);
                                    let preset_name = preset.name.clone();
                                    let _ = self.command_tx.send(AppCommand::SaveCustomPreset(preset));

                                    // Preset is already applied via live preview, just update UI state
                                    if self.dsp_view.is_streaming {
                                        tracing::info!("Preset '{}' saved (already applied via live preview)", preset_name);
                                        self.current_preset = Some(preset_name.clone());
                                        self.dsp_view.current_active_preset = Some(preset_name.clone()); // Sync to DSP view
                                        self.now_playing_view.current_preset = Some(preset_name.clone());
                                        self.status_message = Some(format!("Saved preset: {}", preset_name));
                                        let _ = self.command_tx.send(AppCommand::LoadPresetCurve(preset_name));
                                    }

                                    self.show_eq_editor = false;
                                    self.preset_before_editor = None; // Clear saved preset on successful save
                                }
                                EqEditorAction::Modified => {
                                    // Just redraw
                                }
                                EqEditorAction::LiveUpdate(preset) => {
                                    // Apply EQ changes in real-time if streaming
                                    if self.dsp_view.is_streaming {
                                        tracing::debug!("Live preview: applying EQ changes for {}", preset.name);
                                        let _ = self.command_tx.send(AppCommand::DspApplyPresetData(preset));
                                    }
                                }
                            }
                        }
                    }

                    // Close button
                    egui::TopBottomPanel::bottom("close_editor").show(ctx, |ui| {
                        if ui.button("Close Editor").clicked() {
                            self.show_eq_editor = false;

                            // Restore previous preset if we were streaming and had one
                            if self.dsp_view.is_streaming {
                                if let Some(prev_preset) = self.preset_before_editor.take() {
                                    tracing::info!("Closing EQ editor without saving - restoring previous preset: {}", prev_preset);
                                    let _ = self.command_tx.send(AppCommand::DspChangePreset(prev_preset));
                                } else {
                                    tracing::info!("Closing EQ editor - no previous preset to restore");
                                }
                            }
                        }
                    });
                } else {
                    // Show presets panel on left
                    egui::SidePanel::left("presets_panel").show(ctx, |ui| {
                // Show custom EQ option if DSP is streaming (custom EQ works in DSP mode)
                // Hide it if using WiiM API only (device doesn't support custom EQ)
                let show_custom_eq = self.dsp_view.is_streaming;
                let device_connected = self.device.is_some();
                if let Some(action) = self.presets_view.show(ui, show_custom_eq, device_connected) {
                    match action {
                        PresetAction::Refresh => {
                            let _ = self.command_tx.send(AppCommand::RefreshPresets);
                        }
                        PresetAction::Select(preset) => {
                            tracing::info!("Selected preset: {}", preset);
                        }
                        PresetAction::Apply(preset) => {
                            // If DSP is streaming, use DspChangePreset; otherwise use WiiM API
                            if self.dsp_view.is_streaming {
                                tracing::info!("Applying preset to DSP: {}", preset);
                                let _ = self.command_tx.send(AppCommand::DspChangePreset(preset.clone()));
                                self.status_message = Some(format!("Applying DSP preset: {}", preset));

                                // Immediately update UI state for instant feedback
                                self.current_preset = Some(preset.clone());
                                self.dsp_view.current_active_preset = Some(preset.clone()); // Sync to DSP view
                                self.now_playing_view.current_preset = Some(preset.clone());

                                // Clear the curve immediately to avoid showing stale data
                                // It will be repopulated when LoadPresetCurve response arrives
                                self.current_preset_curve = None;
                                self.now_playing_view.current_preset_curve = None;

                                // Load the EQ curve for display
                                let _ = self.command_tx.send(AppCommand::LoadPresetCurve(preset));
                            } else if self.device.is_some() {
                                tracing::info!("Applying preset to WiiM device: {}", preset);
                                let _ = self.command_tx.send(AppCommand::ApplyPreset(preset.clone()));
                                self.status_message = Some(format!("Applying preset: {}", preset));

                                // Immediately update UI state for instant feedback
                                self.current_preset = Some(preset.clone());
                                self.dsp_view.current_active_preset = Some(preset.clone()); // Sync to DSP view
                                self.now_playing_view.current_preset = Some(preset.clone());

                                // Clear the curve immediately to avoid showing stale data
                                // It will be repopulated when LoadPresetCurve response arrives
                                self.current_preset_curve = None;
                                self.now_playing_view.current_preset_curve = None;

                                // Load the EQ curve for display
                                let _ = self.command_tx.send(AppCommand::LoadPresetCurve(preset));
                            } else {
                                self.status_message = Some("Not connected (no WiiM device or DSP streaming)".to_string());
                            }
                        }
                        PresetAction::CreateCustom => {
                            let mut editor = EqEditorView::default();
                            // Populate existing presets list and auto-fix name conflicts
                            editor.set_existing_presets(self.presets_view.custom_presets.clone());
                            self.eq_editor_view = Some(editor);
                            self.show_eq_editor = true;

                            // If streaming, save current preset and immediately apply Flat for accurate live preview
                            // This ensures what you see (Flat sliders) matches what you hear
                            if self.dsp_view.is_streaming {
                                // Save current preset for restoration on cancel
                                self.preset_before_editor = self.current_preset.clone();
                                tracing::info!("Opening EQ editor - saving current preset '{}' and applying Flat for live preview",
                                    self.preset_before_editor.as_deref().unwrap_or("none"));

                                let flat_preset = aaeq_core::EqPreset::default();
                                let _ = self.command_tx.send(AppCommand::DspApplyPresetData(flat_preset));
                            }
                        }
                        PresetAction::EditCustom(preset_name) => {
                            tracing::info!("Loading preset for editing: {}", preset_name);
                            let _ = self.command_tx.send(AppCommand::EditCustomPreset(preset_name));
                        }
                        PresetAction::DeleteCustom(preset_name) => {
                            tracing::info!("Requesting delete confirmation for preset: {}", preset_name);
                            // Show confirmation dialog
                            self.show_delete_confirmation = true;
                            self.preset_to_delete = Some(preset_name);
                        }
                    }
                }
            });

                    // Central panel for now playing
                    egui::CentralPanel::default().show(ctx, |ui| {
                        if let Some(action) = self.now_playing_view.show(ui, self.album_art_cache.clone()) {
                            match action {
                                NowPlayingAction::SaveMapping(scope) => {
                                    // Pass track and preset to the async worker for saving
                                    if let (Some(track), Some(preset)) = (&self.current_track, &self.current_preset) {
                                        let _ = self.command_tx.send(AppCommand::SaveMapping(scope, track.clone(), preset.clone(), self.active_profile_id));
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
            }

            AppMode::DspServer => {
                // DSP Server Mode: Show DSP controls in main area
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(action) = self.dsp_view.show(ui, &self.current_theme) {
                        match action {
                            DspAction::SinkTypeChanged(sink_type) => {
                                tracing::info!("DSP sink type changed: {:?}", sink_type);

                                // Save current device to appropriate field before switching
                                if let Some(device) = &self.dsp_view.selected_device {
                                    match self.dsp_view.selected_sink {
                                        SinkType::LocalDac => {
                                            self.dsp_view.last_local_dac_device = Some(device.clone());
                                            tracing::info!("Saved Local DAC device: {}", device);
                                        }
                                        SinkType::Dlna => {
                                            self.dsp_view.last_dlna_device = Some(device.clone());
                                            tracing::info!("Saved DLNA device: {}", device);
                                        }
                                        SinkType::AirPlay => {
                                            self.dsp_view.last_airplay_device = Some(device.clone());
                                            tracing::info!("Saved AirPlay device: {}", device);
                                        }
                                    }
                                }

                                // Clear devices for new sink type
                                self.dsp_view.available_devices.clear();

                                // Restore saved device for new sink type
                                self.dsp_view.selected_device = match sink_type {
                                    SinkType::LocalDac => {
                                        let saved = self.dsp_view.last_local_dac_device.clone();
                                        if let Some(ref device) = saved {
                                            tracing::info!("Restored Local DAC device: {}", device);
                                        }
                                        saved
                                    }
                                    SinkType::Dlna => {
                                        let saved = self.dsp_view.last_dlna_device.clone();
                                        if let Some(ref device) = saved {
                                            tracing::info!("Restored DLNA device: {}", device);
                                        }
                                        saved
                                    }
                                    SinkType::AirPlay => {
                                        let saved = self.dsp_view.last_airplay_device.clone();
                                        if let Some(ref device) = saved {
                                            tracing::info!("Restored AirPlay device: {}", device);
                                        }
                                        saved
                                    }
                                };

                                // Load DSP sink settings for the new sink type
                                let pool = self.pool.clone();
                                let sink_type_str = sink_type.to_db_string().to_string();
                                tokio::spawn(async move {
                                    use aaeq_persistence::DspSinkSettingsRepository;
                                    let sink_repo = DspSinkSettingsRepository::new(pool);
                                    if let Ok(Some(settings)) = sink_repo.get_by_sink_type(&sink_type_str).await {
                                        tracing::info!("Loaded DSP sink settings for {}: sample_rate={}, format={}, buffer_ms={}, headroom_db={}",
                                            settings.sink_type, settings.sample_rate, settings.format, settings.buffer_ms, settings.headroom_db);
                                        // TODO: Apply settings to UI (requires sending response back)
                                    } else {
                                        tracing::info!("No DSP sink settings found for {}, using current values", sink_type_str);
                                    }
                                });

                                // For now, adjust format based on sink type compatibility as fallback
                                match sink_type {
                                    SinkType::LocalDac => {
                                        // Local DAC only supports F32 and S16LE
                                        if self.dsp_view.format == FormatOption::S24LE {
                                            self.dsp_view.format = FormatOption::F32;
                                            tracing::info!("Switched to F32 format for Local DAC compatibility");
                                        }
                                    }
                                    SinkType::Dlna => {
                                        // DLNA requires PCM format (S16LE or S24LE)
                                        if self.dsp_view.format == FormatOption::F32 {
                                            self.dsp_view.format = FormatOption::S24LE;
                                            tracing::info!("Switched to S24LE format for DLNA compatibility");
                                        }
                                    }
                                    SinkType::AirPlay => {
                                        // AirPlay supports all formats
                                    }
                                }

                                // Clear visualization buffers when switching sink types
                                self.dsp_view.clear_buffers();
                                self.dsp_view.reset_auto_delay(); // Reset auto-detection for new sink type

                                // Reset delay to 0 when switching to Local DAC
                                if matches!(sink_type, SinkType::LocalDac) {
                                    self.dsp_view.viz_delay_ms = 0;
                                }

                                // Automatically discover devices for the new sink type
                                self.dsp_view.discovering = true;
                                let _ = self.command_tx.send(AppCommand::DspDiscoverDevices(sink_type, Some(self.device_host.clone())));
                            }
                            DspAction::DeviceSelected(device) => {
                                tracing::info!("DSP device selected: {}", device);

                                // Save device to appropriate per-sink field
                                match self.dsp_view.selected_sink {
                                    SinkType::LocalDac => {
                                        self.dsp_view.last_local_dac_device = Some(device.clone());
                                    }
                                    SinkType::Dlna => {
                                        self.dsp_view.last_dlna_device = Some(device.clone());
                                    }
                                    SinkType::AirPlay => {
                                        self.dsp_view.last_airplay_device = Some(device.clone());
                                    }
                                }

                                // Also save the selected output device to settings (for initial restore on startup)
                                let _ = self.command_tx.send(AppCommand::SaveOutputDevice(device));
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

                                // Use target sample rate if resampling is enabled, otherwise use input rate
                                let output_sample_rate = if self.dsp_view.resample_enabled {
                                    self.dsp_view.target_sample_rate
                                } else {
                                    self.dsp_view.sample_rate
                                };

                                let config = OutputConfig {
                                    sample_rate: output_sample_rate,
                                    channels: 2,
                                    format,
                                    buffer_ms: self.dsp_view.buffer_ms,
                                    exclusive: false,
                                };

                                if let Some(device) = &self.dsp_view.selected_device {
                                    let dsp_config = DspRuntimeConfig {
                                        dither_enabled: self.dsp_view.dither_enabled,
                                        dither_mode: self.dsp_view.dither_mode,
                                        noise_shaping: self.dsp_view.noise_shaping,
                                        target_bits: self.dsp_view.target_bits,
                                        resample_enabled: self.dsp_view.resample_enabled,
                                        resample_quality: self.dsp_view.resample_quality,
                                        target_sample_rate: self.dsp_view.target_sample_rate,
                                    };
                                    let _ = self.command_tx.send(AppCommand::DspStartStreaming(
                                        self.dsp_view.selected_sink,
                                        device.clone(),
                                        config,
                                        self.dsp_view.use_test_tone,
                                        self.dsp_view.selected_input_device.clone(),
                                        None, // No manual preset override - use EQ Management
                                        dsp_config,
                                    ));
                                    self.dsp_view.is_starting = true; // Show spinner while connecting
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

                                    // Use target sample rate if resampling is enabled, otherwise use input rate
                                    let output_sample_rate = if self.dsp_view.resample_enabled {
                                        self.dsp_view.target_sample_rate
                                    } else {
                                        self.dsp_view.sample_rate
                                    };

                                    let config = OutputConfig {
                                        sample_rate: output_sample_rate,
                                        channels: 2,
                                        format,
                                        buffer_ms: self.dsp_view.buffer_ms,
                                        exclusive: false,
                                    };

                                    if let Some(device) = &self.dsp_view.selected_device {
                                        let dsp_config = DspRuntimeConfig {
                                            dither_enabled: self.dsp_view.dither_enabled,
                                            dither_mode: self.dsp_view.dither_mode,
                                            noise_shaping: self.dsp_view.noise_shaping,
                                            target_bits: self.dsp_view.target_bits,
                                            resample_enabled: self.dsp_view.resample_enabled,
                                            resample_quality: self.dsp_view.resample_quality,
                                            target_sample_rate: self.dsp_view.target_sample_rate,
                                        };
                                        let _ = self.command_tx.send(AppCommand::DspStartStreaming(
                                            self.dsp_view.selected_sink,
                                            device.clone(),
                                            config,
                                            true, // use_test_tone
                                            None, // input_device
                                            None, // No manual preset override - use EQ Management // Use selected EQ preset
                                            dsp_config,
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
                            DspAction::ToggleMeters => {
                                tracing::info!("Meters toggled: {}", self.dsp_view.show_meters);
                            }
                            DspAction::SaveCustomPreset(preset) => {
                                tracing::info!("Saving custom preset: {}", preset.name);
                                let _ = self.command_tx.send(AppCommand::SaveCustomPreset(preset));
                            }
                            DspAction::HeadroomChanged => {
                                tracing::info!("Headroom changed to {} dB - saving to database", self.dsp_view.headroom_db);
                                self.save_dsp_sink_settings();
                                // Headroom will be applied on next stream start or restart
                            }
                            DspAction::ClipDetectionChanged => {
                                tracing::info!("Clip detection changed to: {}", self.dsp_view.clip_detection);
                                // Clip detection setting will be applied on next stream start
                            }
                            DspAction::ResetClipCount => {
                                tracing::info!("Clip counter reset");
                                self.dsp_view.clip_count = 0;
                                // TODO: Also reset the counter in the actual HeadroomControl instance
                                // This will be implemented when integrating with the DSP pipeline
                            }
                            DspAction::DitherToggled => {
                                tracing::info!("Dithering toggled: {} - auto-saving", self.dsp_view.dither_enabled);
                                // Auto-save DSP settings
                                self.auto_save_dsp_settings();
                            }
                            DspAction::DitherModeChanged => {
                                tracing::info!("Dither mode changed to: {:?} - auto-saving", self.dsp_view.dither_mode);
                                // Auto-save DSP settings
                                self.auto_save_dsp_settings();
                            }
                            DspAction::NoiseShapingChanged => {
                                tracing::info!("Noise shaping changed to: {:?} - auto-saving", self.dsp_view.noise_shaping);
                                // Auto-save DSP settings
                                self.auto_save_dsp_settings();
                            }
                            DspAction::TargetBitsChanged => {
                                tracing::info!("Target bit depth changed to: {} bits - auto-saving", self.dsp_view.target_bits);
                                // Auto-save DSP settings
                                self.auto_save_dsp_settings();
                            }
                            DspAction::ResampleToggled => {
                                tracing::info!("Resampling toggled: {} - auto-saving and restarting stream", self.dsp_view.resample_enabled);
                                // Auto-save DSP settings
                                self.auto_save_dsp_settings();
                                // Toggling resampling changes output rate - must restart stream
                                if self.dsp_view.is_streaming {
                                    tracing::info!("Resampling toggled during streaming - restarting stream to apply new output rate");
                                    // Stop and restart stream with new settings
                                    let _ = self.command_tx.send(AppCommand::DspStopStreaming);
                                    // Queue restart with new settings (will happen after stop completes)
                                    self.dsp_view.needs_restart = true;
                                    self.status_message = Some("Restarting stream with new resampling settings...".to_string());
                                }
                            }
                            DspAction::ResampleQualityChanged => {
                                tracing::info!("Resample quality changed to: {:?} - auto-saving and updating live", self.dsp_view.resample_quality);
                                // Auto-save DSP settings
                                self.auto_save_dsp_settings();
                                // Quality change doesn't affect output rate - can update live
                                if self.dsp_view.is_streaming && self.dsp_view.resample_enabled {
                                    let _ = self.command_tx.send(AppCommand::DspUpdateResamplerConfig(
                                        self.dsp_view.resample_enabled,
                                        self.dsp_view.resample_quality,
                                        self.dsp_view.target_sample_rate,
                                    ));
                                }
                            }
                            DspAction::TargetSampleRateChanged => {
                                tracing::info!("Target sample rate changed to: {} Hz - auto-saving and restarting stream", self.dsp_view.target_sample_rate);
                                // Auto-save DSP settings
                                self.auto_save_dsp_settings();
                                // Changing target rate changes output rate - must restart stream
                                if self.dsp_view.is_streaming && self.dsp_view.resample_enabled {
                                    tracing::info!("Target sample rate changed during streaming - restarting stream to apply new output rate");
                                    // Stop and restart stream with new settings
                                    let _ = self.command_tx.send(AppCommand::DspStopStreaming);
                                    // Queue restart with new settings (will happen after stop completes)
                                    self.dsp_view.needs_restart = true;
                                    self.status_message = Some("Restarting stream with new sample rate...".to_string());
                                }
                            }
                            DspAction::SampleRateChanged => {
                                tracing::info!("Sample rate changed to: {} Hz - saving to database", self.dsp_view.sample_rate);
                                self.save_dsp_sink_settings();
                                // If streaming, need to restart for sample rate change
                                if self.dsp_view.is_streaming {
                                    self.dsp_view.needs_restart = true;
                                }
                            }
                            DspAction::FormatChanged => {
                                tracing::info!("Format changed to: {:?} - saving to database", self.dsp_view.format);
                                self.save_dsp_sink_settings();
                                // If streaming, need to restart for format change
                                if self.dsp_view.is_streaming {
                                    self.dsp_view.needs_restart = true;
                                }
                            }
                            DspAction::BufferChanged => {
                                tracing::info!("Buffer changed to: {} ms - saving to database", self.dsp_view.buffer_ms);
                                self.save_dsp_sink_settings();
                                // Buffer change can be applied without restart for some sinks
                            }
                        }
                    }
                });
            }
            AppMode::Settings => {
                // Settings Mode: Show application settings
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Settings");
                    ui.add_space(20.0);

                    // Theme selection
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Theme").strong().size(16.0));
                        ui.add_space(10.0);

                        egui::ComboBox::new("theme_selector", "")
                            .selected_text(self.current_theme.display_name())
                            .show_ui(ui, |ui| {
                                for theme in crate::theme::Theme::all() {
                                    if ui.selectable_value(&mut self.current_theme, *theme, theme.display_name()).clicked() {
                                        tracing::info!("Theme changed to: {:?}", theme);
                                        // Save theme to database
                                        let _ = self.command_tx.send(AppCommand::SaveTheme(theme.as_str().to_string()));
                                    }
                                }
                            });
                    });

                    ui.add_space(20.0);

                    // Debug logging option
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Debug Logging").strong().size(16.0));
                        ui.add_space(10.0);

                        let prev_debug_logging = self.enable_debug_logging;
                        ui.checkbox(&mut self.enable_debug_logging, "Enable debug logging to file")
                            .on_hover_text("Save all debug logs to a file for troubleshooting and bug reporting");

                        if self.enable_debug_logging != prev_debug_logging {
                            tracing::info!("Debug logging changed to: {}", self.enable_debug_logging);
                            // Save setting to database
                            let _ = self.command_tx.send(AppCommand::SaveEnableDebugLogging(self.enable_debug_logging));
                        }

                        ui.add_space(5.0);

                        // Show log file location
                        if self.enable_debug_logging {
                            let log_dir = self.db_path.parent()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "unknown".to_string());

                            ui.label(
                                egui::RichText::new(format!("📁 Log file: {}/debug.log", log_dir))
                                    .color(egui::Color32::GRAY)
                                    .size(10.0)
                            );
                            ui.label(
                                egui::RichText::new("⚠️ Changes take effect after restarting the application")
                                    .color(egui::Color32::from_rgb(255, 140, 0))
                                    .italics()
                                    .size(10.0)
                            );
                        }
                    });


                    // Database management
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Database Management").strong().size(16.0));
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            if ui.button("📦 Backup Database").clicked() {
                                let db_path = self.db_path.display().to_string();
                                let _ = self.command_tx.send(AppCommand::BackupDatabase(db_path));
                            }

                            if ui.button("📥 Restore Database").clicked() {
                                // Open file picker for .zip files
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("AAEQ Backup", &["zip"])
                                    .pick_file()
                                {
                                    let backup_path = path.display().to_string();
                                    let db_path = self.db_path.display().to_string();
                                    let _ = self.command_tx.send(AppCommand::RestoreDatabase(backup_path, db_path));
                                }
                            }
                        });

                        ui.add_space(5.0);
                        ui.label("Backup creates a timestamped .zip file.");
                        ui.label("Restore automatically backs up current database before restoring.");
                    });

                    ui.add_space(20.0);

                    // About section
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("About").strong().size(16.0));
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.label("Version:");
                            ui.label(egui::RichText::new(env!("CARGO_PKG_VERSION")).strong());
                        });

                        ui.add_space(5.0);

                        ui.horizontal(|ui| {
                            ui.label("Author:");
                            ui.hyperlink_to("Jascha Wanger", "https://jascha.me");
                            ui.label("/");
                            ui.hyperlink_to("Tarnover, LLC", "https://tarnover.com");
                        });

                        ui.add_space(5.0);

                        ui.horizontal(|ui| {
                            ui.label("License:");
                            ui.label("MIT");
                        });

                        ui.add_space(5.0);

                        ui.horizontal(|ui| {
                            ui.label("Project:");
                            ui.hyperlink_to("https://github.com/jaschadub/AAEQ", "https://github.com/jaschadub/AAEQ");
                        });
                    });
                    }); // End of ScrollArea
                });
            }
        }

        // Window resize logic for visualization elements and collapsed state
        let viz_enabled = self.dsp_view.audio_viz.enabled || self.dsp_view.spectrum_analyzer.enabled;
        let viz_mode = self.dsp_view.viz_mode;
        let show_meters = self.dsp_view.show_meters;
        let is_collapsed = self.dsp_view.audio_output_collapsed;

        // Check if visualization, meters, or collapsed state changed
        if viz_enabled != self.last_viz_state || viz_mode != self.last_viz_mode || show_meters != self.last_meters_state || is_collapsed != self.last_collapsed_state {
            tracing::info!("Window resize needed - viz: {} -> {}, mode: {:?} -> {:?}, meters: {} -> {}, collapsed: {} -> {}",
                self.last_viz_state, viz_enabled, self.last_viz_mode, viz_mode, self.last_meters_state, show_meters, self.last_collapsed_state, is_collapsed);

            // Calculate new window height based on visible elements
            // More accurate calculation based on actual UI layout
            let base_height = 180.0; // Title bar, menu bar, Device section minimal
            let dsp_pipeline_height = 80.0; // DSP Pipeline visualization
            let audio_output_header_height = 120.0; // Audio Output header + controls (always visible)
            let audio_output_body_height = 420.0; // Full configuration section (when expanded)

            let mut new_height = base_height + dsp_pipeline_height;

            // Audio Output section - always has header, adds body if not collapsed
            new_height += audio_output_header_height;
            if !is_collapsed {
                new_height += audio_output_body_height;
            }

            // Add height for visualization based on mode
            if viz_enabled {
                match viz_mode {
                    crate::views::VisualizationMode::Waveform => {
                        // Waveform visualization (~250px including spacing)
                        new_height += 250.0;
                    }
                    crate::views::VisualizationMode::Spectrum => {
                        // Spectrum analyzer needs more space (~380px for display + labels + spacing)
                        new_height += 380.0;
                    }
                }
            }

            // Add height for audio meters (380px: includes spacing, separator, labels, meters with padding)
            if show_meters {
                new_height += 380.0;
            }

            // Apply window resize
            let current_size = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
            if let Some(size) = current_size {
                let new_size = egui::vec2(size.x, new_height);
                tracing::info!("Resizing window from {}x{} to {}x{}", size.x, size.y, new_size.x, new_size.y);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));

                // Request another repaint to ensure layout is complete before resize takes full effect
                ctx.request_repaint();
            }

            // Update tracking state
            self.last_viz_state = viz_enabled;
            self.last_viz_mode = viz_mode;
            self.last_meters_state = show_meters;
            self.last_collapsed_state = is_collapsed;
        }

        // Show delete preset confirmation dialog
        if self.show_delete_confirmation {
            egui::Window::new("Delete Preset")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some(preset_name) = &self.preset_to_delete {
                        ui.label(format!("Delete preset '{}'?", preset_name));
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("This cannot be undone.")
                                .color(egui::Color32::from_rgb(255, 100, 100))
                                .italics()
                        );
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.show_delete_confirmation = false;
                                self.preset_to_delete = None;
                            }

                            if ui.button("Delete").clicked() {
                                if let Some(name) = self.preset_to_delete.take() {
                                    let _ = self.command_tx.send(AppCommand::DeleteCustomPreset(name));
                                }
                                self.show_delete_confirmation = false;
                            }
                        });
                    }
                });
        }

        // Show profile management dialog
        if self.show_profile_dialog {
            egui::Window::new(match self.profile_dialog_mode {
                ProfileDialogMode::Create => "Create Profile",
                ProfileDialogMode::Duplicate => "Duplicate Profile",
                ProfileDialogMode::Rename => "Edit Profile",
                ProfileDialogMode::Delete => "Delete Profile",
            })
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                match self.profile_dialog_mode {
                    ProfileDialogMode::Create | ProfileDialogMode::Duplicate | ProfileDialogMode::Rename => {
                        ui.label("Profile name:");
                        ui.text_edit_singleline(&mut self.profile_name_input);

                        ui.add_space(5.0);

                        // Icon picker (for Create, Duplicate, and Rename modes)
                        if self.profile_dialog_mode != ProfileDialogMode::Delete {
                            ui.horizontal(|ui| {
                                ui.label("Icon:");

                                // Icon selection buttons (desktop-appropriate icons, no car)
                                let icons = ["🏠", "🎧", "🔊", "🎵", "🎼", "🎹", "📻", "💿", "📁", "⭐"];
                                for icon in icons {
                                    if ui.selectable_label(self.profile_icon_input == icon, icon).clicked() {
                                        self.profile_icon_input = icon.to_string();
                                    }
                                }
                            });

                            ui.add_space(5.0);

                            // Color picker
                            ui.horizontal(|ui| {
                                ui.label("Color:");

                                // Parse current color
                                let mut color = egui::Color32::from_rgb(74, 144, 226); // Default blue
                                if self.profile_color_input.starts_with('#') && self.profile_color_input.len() == 7 {
                                    if let Ok(r) = u8::from_str_radix(&self.profile_color_input[1..3], 16) {
                                        if let Ok(g) = u8::from_str_radix(&self.profile_color_input[3..5], 16) {
                                            if let Ok(b) = u8::from_str_radix(&self.profile_color_input[5..7], 16) {
                                                color = egui::Color32::from_rgb(r, g, b);
                                            }
                                        }
                                    }
                                }

                                // Color picker
                                if ui.color_edit_button_srgba(&mut color).changed() {
                                    self.profile_color_input = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
                                }

                                // Preset color swatches (theme-aware colors)
                                let preset_colors = [
                                    ("#4A90E2", "Blue"),
                                    ("#9B59B6", "Purple"),
                                    ("#E74C3C", "Red"),
                                    ("#F39C12", "Orange"),
                                    ("#2ECC71", "Green"),
                                    ("#1ABC9C", "Teal"),
                                    ("#34495E", "Gray"),
                                ];

                                for (hex, _name) in preset_colors {
                                    let preset_color = if hex.starts_with('#') && hex.len() == 7 {
                                        let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(128);
                                        let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(128);
                                        let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(128);
                                        egui::Color32::from_rgb(r, g, b)
                                    } else {
                                        egui::Color32::GRAY
                                    };

                                    let size = egui::Vec2::new(20.0, 20.0);
                                    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

                                    if ui.is_rect_visible(rect) {
                                        ui.painter().circle_filled(rect.center(), 10.0, preset_color);
                                        if self.profile_color_input == hex {
                                            ui.painter().circle_stroke(rect.center(), 11.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                                        }
                                    }

                                    if response.clicked() {
                                        self.profile_color_input = hex.to_string();
                                    }

                                    response.on_hover_text(_name);
                                }
                            });

                            ui.add_space(5.0);
                        }

                        ui.add_space(5.0);

                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.show_profile_dialog = false;
                                self.profile_name_input.clear();
                                self.profile_icon_input = "📁".to_string();
                                self.profile_color_input = "#4A90E2".to_string();
                            }

                            let button_text = match self.profile_dialog_mode {
                                ProfileDialogMode::Create => "Create",
                                ProfileDialogMode::Duplicate => "Duplicate",
                                ProfileDialogMode::Rename => "Save",
                                _ => "OK",
                            };

                            if ui.button(button_text).clicked() && !self.profile_name_input.trim().is_empty() {
                                let profile_name = self.profile_name_input.trim().to_string();

                                if self.profile_dialog_mode == ProfileDialogMode::Create || self.profile_dialog_mode == ProfileDialogMode::Duplicate {
                                    // Create new profile
                                    let pool = self.pool.clone();
                                    let profile_name_clone = profile_name.clone();
                                    let profile_icon = self.profile_icon_input.clone();
                                    let profile_color = self.profile_color_input.clone();
                                    let command_tx = self.command_tx.clone();
                                    tokio::spawn(async move {
                                        let profile_repo = ProfileRepository::new(pool);
                                        let profile = aaeq_core::Profile {
                                            id: None,
                                            name: profile_name_clone.clone(),
                                            is_builtin: false,
                                            icon: profile_icon,
                                            color: profile_color,
                                            created_at: chrono::Utc::now().timestamp(),
                                            updated_at: chrono::Utc::now().timestamp(),
                                        };
                                        match profile_repo.create(&profile).await {
                                            Ok(_) => {
                                                tracing::info!("Created profile: {}", profile_name_clone);
                                                let _ = command_tx.send(AppCommand::ReloadProfiles);
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to create profile: {}", e);
                                            }
                                        }
                                    });

                                    let action = if self.profile_dialog_mode == ProfileDialogMode::Duplicate {
                                        "Duplicating"
                                    } else {
                                        "Creating"
                                    };
                                    self.status_message = Some(format!("{} profile: {}", action, profile_name));
                                    // Reset inputs
                                    self.profile_icon_input = "📁".to_string();
                                    self.profile_color_input = "#4A90E2".to_string();
                                    self.profile_to_duplicate = None;
                                } else {
                                    // Edit existing profile (name, icon, color)
                                    if let Some(profile_id) = self.profile_to_rename {
                                        let pool = self.pool.clone();
                                        let profile_name_clone = profile_name.clone();
                                        let profile_icon = self.profile_icon_input.clone();
                                        let profile_color = self.profile_color_input.clone();
                                        let command_tx = self.command_tx.clone();
                                        tokio::spawn(async move {
                                            let profile_repo = ProfileRepository::new(pool);
                                            match profile_repo.update(profile_id, &profile_name_clone, &profile_icon, &profile_color).await {
                                                Ok(_) => {
                                                    tracing::info!("Updated profile: {}", profile_name_clone);
                                                    let _ = command_tx.send(AppCommand::ReloadProfiles);
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to update profile: {}", e);
                                                }
                                            }
                                        });

                                        self.status_message = Some(format!("Updating profile: {}", profile_name));
                                    }
                                }

                                // Don't clear dialog state here - will be cleared when ProfilesLoaded response arrives
                            }
                        });
                    }
                    ProfileDialogMode::Delete => {
                        ui.label(format!("Are you sure you want to delete the profile '{}'?", self.profile_name_input));
                        ui.label("All EQ mappings for this profile will be lost.");

                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.show_profile_dialog = false;
                                self.profile_to_delete = None;
                            }

                            if ui.button("Delete").clicked() {
                                if let Some(profile_id) = self.profile_to_delete {
                                    let pool = self.pool.clone();
                                    let profile_name = self.profile_name_input.clone();
                                    let profile_name_clone = profile_name.clone();
                                    let command_tx = self.command_tx.clone();
                                    let is_active = profile_id == self.active_profile_id;

                                    tokio::spawn(async move {
                                        // Switch to Default if deleting active profile
                                        if is_active {
                                            let settings_repo = AppSettingsRepository::new(pool.clone());
                                            if let Err(e) = settings_repo.set_active_profile_id(1).await {
                                                tracing::error!("Failed to switch to Default profile: {}", e);
                                            }
                                        }

                                        // Delete profile
                                        let profile_repo = ProfileRepository::new(pool);
                                        match profile_repo.delete(profile_id).await {
                                            Ok(_) => {
                                                tracing::info!("Deleted profile: {}", profile_name_clone);
                                                let _ = command_tx.send(AppCommand::ReloadProfiles);
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to delete profile: {}", e);
                                            }
                                        }
                                    });

                                    if is_active {
                                        self.active_profile_id = 1; // Default profile

                                        // Reload mappings for Default profile
                                        let pool = self.pool.clone();
                                        let rules_index = self.rules_index.clone();
                                        tokio::spawn(async move {
                                            let repo = MappingRepository::new(pool);
                                            if let Ok(mappings) = repo.list_by_profile(1).await {
                                                let mut rules = rules_index.write().await;
                                                *rules = RulesIndex::from_mappings(mappings);
                                                tracing::info!("Switched to Default profile, loaded {} song rules, {} album rules, {} genre rules",
                                                    rules.song_rules.len(), rules.album_rules.len(), rules.genre_rules.len());
                                            }
                                        });
                                    }

                                    self.status_message = Some(format!("Deleting profile: {}", profile_name));
                                }

                                // Don't clear dialog state here - will be cleared when ProfilesLoaded response arrives
                            }
                        });
                    }
                }
            });
        }

        // Request continuous repaint for polling
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
