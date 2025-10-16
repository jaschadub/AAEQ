use aaeq_core::{EqPreset, TrackMeta, Scope};
use crate::audio_viz::AudioVizState;
use crate::widgets::VerticalSlider;
use crate::album_art::{AlbumArtCache, AlbumArtState};
use egui::{Context, ScrollArea, Ui};
use std::sync::Arc;

/// View for creating/editing EQ presets with vertical sliders
pub struct EqEditorView {
    pub preset: EqPreset,
    pub preset_name: String,
    pub existing_presets: Vec<String>, // List of existing preset names for validation
    pub name_error: Option<String>,    // Error message if name is invalid
}

impl Default for EqEditorView {
    fn default() -> Self {
        Self {
            preset: EqPreset::default(),
            preset_name: "Custom".to_string(),
            existing_presets: vec![],
            name_error: None,
        }
    }
}

impl EqEditorView {
    pub fn new(preset: EqPreset) -> Self {
        Self {
            preset_name: preset.name.clone(),
            preset,
            existing_presets: vec![],
            name_error: None,
        }
    }

    fn check_name_conflict(&self) -> bool {
        self.existing_presets.iter().any(|p| p == &self.preset_name)
    }

    pub fn show(&mut self, ctx: &Context) -> Option<EqEditorAction> {
        let mut action = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("EQ Editor");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Preset Name:");
                let response = ui.text_edit_singleline(&mut self.preset_name);

                // Check for name conflict when text changes
                if response.changed() {
                    if self.check_name_conflict() {
                        self.name_error = Some(format!("Preset '{}' already exists! Please choose a different name.", self.preset_name));
                    } else if self.preset_name.trim().is_empty() {
                        self.name_error = Some("Preset name cannot be empty!".to_string());
                    } else {
                        self.name_error = None;
                    }
                }
            });

            // Show error message if name is invalid
            if let Some(error) = &self.name_error {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", error))
                            .color(egui::Color32::from_rgb(255, 100, 100))
                            .strong()
                    );
                });
            }

            ui.add_space(10.0);

            // EQ sliders in a horizontal layout
            ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for band in &mut self.preset.bands {
                        ui.vertical(|ui| {
                            let label = format_frequency(band.frequency);
                            let slider = VerticalSlider::new(
                                &mut band.gain,
                                -12.0..=12.0,
                                label,
                            );
                            ui.add(slider);
                        });
                        ui.add_space(5.0);
                    }
                });
            });

            ui.add_space(20.0);
            ui.separator();

            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("Reset to Flat").clicked() {
                    for band in &mut self.preset.bands {
                        band.gain = 0.0;
                    }
                    action = Some(EqEditorAction::Modified);
                }

                // Disable Save/Apply buttons if name is invalid
                let can_save = self.name_error.is_none();

                ui.add_enabled_ui(can_save, |ui| {
                    if ui.button("Save Preset").on_hover_text_at_pointer(
                        if can_save { "Save preset to database" } else { "Fix name errors before saving" }
                    ).clicked() {
                        self.preset.name = self.preset_name.clone();
                        action = Some(EqEditorAction::Save(self.preset.clone()));
                    }

                    if ui.button("Apply to Device").on_hover_text_at_pointer(
                        if can_save { "Save and apply preset immediately" } else { "Fix name errors before applying" }
                    ).clicked() {
                        self.preset.name = self.preset_name.clone();
                        action = Some(EqEditorAction::Apply(self.preset.clone()));
                    }
                });
            });
        });

        action
    }
}

pub enum EqEditorAction {
    Modified,
    Save(EqPreset),
    Apply(EqPreset),
}

/// Format frequency for display (e.g., 1000 -> "1K", 125 -> "125")
fn format_frequency(hz: u32) -> String {
    if hz >= 1000 {
        format!("{}K", hz / 1000)
    } else {
        hz.to_string()
    }
}

/// View for showing now playing and quick save options
pub struct NowPlayingView {
    pub track: Option<TrackMeta>,
    pub current_preset: Option<String>,
    pub current_preset_curve: Option<EqPreset>, // Cached EQ curve for display
    pub custom_presets: Vec<String>, // List of custom preset names (for determining if curve is exact or estimated)
    pub genre_edit: String,
    album_art_texture: Option<egui::TextureHandle>,
    last_album_art_url: Option<String>,
    default_icon_texture: Option<egui::TextureHandle>, // Default icon when no album art available
}

impl Default for NowPlayingView {
    fn default() -> Self {
        Self {
            track: None,
            current_preset: None,
            current_preset_curve: None,
            custom_presets: vec![],
            genre_edit: String::new(),
            album_art_texture: None,
            last_album_art_url: None,
            default_icon_texture: None,
        }
    }
}

impl NowPlayingView {
    /// Check if track metadata is valid (not all "Unknown")
    fn is_valid_track(track: &TrackMeta) -> bool {
        let is_unknown = track.artist == "Unknown"
            && track.title == "Unknown"
            && track.album == "Unknown";
        !is_unknown
    }

    pub fn show(&mut self, ui: &mut Ui, album_art_cache: Arc<AlbumArtCache>) -> Option<NowPlayingAction> {
        let mut action = None;

        // Load default icon on first run
        if self.default_icon_texture.is_none() {
            // Load the default icon from the project root
            let icon_path = "aaeq-icon.png";
            match image::open(icon_path) {
                Ok(img) => {
                    let size = [img.width() as usize, img.height() as usize];
                    let rgba_image = img.to_rgba8();
                    let pixels = rgba_image.as_flat_samples();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        size,
                        pixels.as_slice(),
                    );
                    let texture = ui.ctx().load_texture(
                        "default_album_art",
                        color_image,
                        Default::default(),
                    );
                    self.default_icon_texture = Some(texture);
                }
                Err(e) => {
                    tracing::warn!("Failed to load default album art icon: {}", e);
                }
            }
        }

        ui.group(|ui| {
            ui.heading("Now Playing");

            if let Some(track) = &self.track {
                // Handle album art loading and display
                if let Some(art_url) = &track.album_art_url {
                    // Check if URL changed - if so, clear cached texture
                    if self.last_album_art_url.as_ref() != Some(art_url) {
                        self.album_art_texture = None;
                        self.last_album_art_url = Some(art_url.clone());
                    }

                    // Try to get loaded image and convert to texture (non-blocking)
                    if self.album_art_texture.is_none() {
                        if let Some(state) = album_art_cache.try_get(art_url) {
                            match state {
                                AlbumArtState::NotLoaded => {
                                    // Start loading
                                    album_art_cache.load(art_url.clone());
                                    ui.ctx().request_repaint();
                                }
                                AlbumArtState::Loading => {
                                    // Still loading, request repaint to check again
                                    ui.ctx().request_repaint();
                                }
                                AlbumArtState::Loaded(color_image) => {
                                    // Convert ColorImage to texture
                                    let texture = ui.ctx().load_texture(
                                        &format!("album_art_{}", art_url),
                                        color_image.as_ref().clone(),
                                        Default::default(),
                                    );
                                    self.album_art_texture = Some(texture);
                                }
                                AlbumArtState::Failed => {
                                    // Failed to load, don't retry
                                }
                            }
                        }
                    }
                }

                // Display layout with album art
                ui.horizontal(|ui| {
                    // Album art on the left - show album art if available, otherwise show default icon
                    if let Some(texture) = &self.album_art_texture {
                        ui.add(egui::Image::new(texture).max_size(egui::vec2(150.0, 150.0)));
                        ui.add_space(10.0);
                    } else if let Some(default_texture) = &self.default_icon_texture {
                        // Show default icon when no album art is loaded
                        ui.add(egui::Image::new(default_texture).max_size(egui::vec2(150.0, 150.0)));
                        ui.add_space(10.0);
                    }

                    // Track info on the right
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Artist:");
                            ui.label(&track.artist);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Title:");
                            ui.label(&track.title);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Album:");
                            ui.label(&track.album);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Genre:");
                            let response = ui.text_edit_singleline(&mut self.genre_edit);
                            // Only update on Enter key, not on every keystroke
                            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                action = Some(NowPlayingAction::UpdateGenre(self.genre_edit.clone()));
                            }
                            if ui.small_button("↻").on_hover_text("Reset to device genre").clicked() {
                                self.genre_edit = track.device_genre.clone();
                                action = Some(NowPlayingAction::UpdateGenre(track.device_genre.clone()));
                            }
                        });
                    });
                });

                if let Some(preset) = &self.current_preset {
                    ui.horizontal(|ui| {
                        ui.label("Current Preset:");
                        ui.strong(preset);
                    });

                    // Visual EQ display - use cached curve if available
                    if let Some(eq_preset) = &self.current_preset_curve {
                        ui.add_space(5.0);
                        ui.separator();

                        // Show label with indicator for estimated curves
                        // Curves are exact (not estimated) if they are:
                        // 1. Known presets from the preset library, OR
                        // 2. Custom presets created and saved by the user
                        let is_known = crate::preset_library::is_known_preset(preset);
                        let is_custom = self.custom_presets.iter().any(|p| p == preset);
                        let is_exact_curve = is_known || is_custom;

                        if is_exact_curve {
                            ui.label("EQ Curve:");
                        } else {
                            ui.horizontal(|ui| {
                                ui.label("EQ Curve:");
                                ui.label(
                                    egui::RichText::new("(estimated)")
                                        .size(10.0)
                                        .color(egui::Color32::GRAY)
                                        .italics()
                                )
                                .on_hover_text("This is an estimated curve based on the preset name. Actual values may differ.");
                            });
                        }
                        ui.add_space(5.0);

                        // Draw EQ bars
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            for band in &eq_preset.bands {
                                ui.vertical(|ui| {
                                    ui.set_width(35.0);

                                    // Draw the bar
                                    let bar_height = 80.0;
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(30.0, bar_height),
                                        egui::Sense::hover()
                                    );

                                    if ui.is_rect_visible(rect) {
                                        let painter = ui.painter();

                                        // Zero line at center
                                        let zero_y = rect.center().y;

                                        // Draw zero line
                                        painter.line_segment(
                                            [
                                                egui::pos2(rect.left(), zero_y),
                                                egui::pos2(rect.right(), zero_y)
                                            ],
                                            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY)
                                        );

                                        // Calculate bar position and height
                                        // Scale: -12dB to +12dB maps to full bar height
                                        let gain_normalized = band.gain / 12.0; // -1.0 to 1.0
                                        let bar_pixel_height = (gain_normalized * bar_height / 2.0).abs();

                                        let bar_rect = if band.gain >= 0.0 {
                                            // Positive gain - bar goes up from zero line
                                            egui::Rect::from_min_max(
                                                egui::pos2(rect.left() + 5.0, zero_y - bar_pixel_height),
                                                egui::pos2(rect.right() - 5.0, zero_y)
                                            )
                                        } else {
                                            // Negative gain - bar goes down from zero line
                                            egui::Rect::from_min_max(
                                                egui::pos2(rect.left() + 5.0, zero_y),
                                                egui::pos2(rect.right() - 5.0, zero_y + bar_pixel_height)
                                            )
                                        };

                                        // Color based on gain
                                        let color = if band.gain > 3.0 {
                                            egui::Color32::from_rgb(100, 200, 100) // Green for boost
                                        } else if band.gain < -3.0 {
                                            egui::Color32::from_rgb(200, 100, 100) // Red for cut
                                        } else if band.gain > 0.0 {
                                            egui::Color32::from_rgb(150, 200, 150) // Light green
                                        } else if band.gain < 0.0 {
                                            egui::Color32::from_rgb(200, 150, 150) // Light red
                                        } else {
                                            egui::Color32::GRAY // Gray for flat
                                        };

                                        painter.rect_filled(bar_rect, 2.0, color);
                                        painter.rect_stroke(bar_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                                    }

                                    // Frequency label below
                                    ui.add_space(2.0);
                                    let freq_label = format_frequency(band.frequency);
                                    ui.label(
                                        egui::RichText::new(freq_label)
                                            .size(9.0)
                                            .color(egui::Color32::GRAY)
                                    );

                                    // Gain value below frequency
                                    let gain_text = if band.gain >= 0.0 {
                                        format!("+{:.1}", band.gain)
                                    } else {
                                        format!("{:.1}", band.gain)
                                    };
                                    ui.label(
                                        egui::RichText::new(gain_text)
                                            .size(8.0)
                                            .color(egui::Color32::LIGHT_GRAY)
                                    );
                                });
                            }
                        });
                    }
                }

                ui.add_space(10.0);
                ui.separator();

                let is_valid = Self::is_valid_track(track);

                if !is_valid {
                    ui.label(
                        egui::RichText::new("⚠ No track playing - cannot save mappings")
                            .color(egui::Color32::YELLOW)
                    );
                } else {
                    ui.label("Save current preset for:");
                }

                ui.add_enabled_ui(is_valid, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("This Song").clicked() {
                            action = Some(NowPlayingAction::SaveMapping(Scope::Song));
                        }
                        if ui.button("This Album").clicked() {
                            action = Some(NowPlayingAction::SaveMapping(Scope::Album));
                        }
                        if ui.button("This Genre").clicked() {
                            action = Some(NowPlayingAction::SaveMapping(Scope::Genre));
                        }
                        if ui.button("Default").clicked() {
                            action = Some(NowPlayingAction::SaveMapping(Scope::Default));
                        }
                    });
                });
            } else {
                // No track playing - show default icon
                ui.horizontal(|ui| {
                    if let Some(default_texture) = &self.default_icon_texture {
                        ui.add(egui::Image::new(default_texture).max_size(egui::vec2(150.0, 150.0)));
                        ui.add_space(10.0);
                    }
                    ui.label("No track playing");
                });
            }
        });

        action
    }
}

pub enum NowPlayingAction {
    SaveMapping(Scope),
    UpdateGenre(String),
}

/// View for listing and managing presets
pub struct PresetsView {
    pub presets: Vec<String>,         // WiiM device presets
    pub custom_presets: Vec<String>,  // Custom EQ presets
    pub selected_preset: Option<String>,
}

impl Default for PresetsView {
    fn default() -> Self {
        Self {
            presets: vec![],
            custom_presets: vec![],
            selected_preset: None,
        }
    }
}

impl PresetsView {
    pub fn show(&mut self, ui: &mut Ui, show_custom_eq: bool, device_connected: bool) -> Option<PresetAction> {
        let mut action = None;

        ui.group(|ui| {
            ui.heading("Presets");

            // Only show "Refresh from Device" button when connected to a WiiM device
            if device_connected {
                if ui.button("Refresh from Device").clicked() {
                    action = Some(PresetAction::Refresh);
                }
                ui.add_space(5.0);
            }


            ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                // Show WiiM device presets
                if !self.presets.is_empty() {
                    ui.label(egui::RichText::new("Device Presets").strong().color(egui::Color32::LIGHT_GREEN));
                    ui.separator();
                    for preset in &self.presets.clone() {
                        let is_selected = self.selected_preset.as_deref() == Some(preset.as_str());
                        if ui.selectable_label(is_selected, preset).clicked() {
                            self.selected_preset = Some(preset.clone());
                            action = Some(PresetAction::Select(preset.clone()));
                        }
                    }
                }

                // Show custom EQ presets
                if !self.custom_presets.is_empty() {
                    if !self.presets.is_empty() {
                        ui.add_space(5.0);
                    }
                    ui.label(egui::RichText::new("Custom Presets").strong().color(egui::Color32::from_rgb(255, 180, 100)));
                    ui.separator();
                    for preset in &self.custom_presets.clone() {
                        let is_selected = self.selected_preset.as_deref() == Some(preset.as_str());
                        if ui.selectable_label(is_selected, preset).clicked() {
                            self.selected_preset = Some(preset.clone());
                            action = Some(PresetAction::Select(preset.clone()));
                        }
                    }
                }
            });

            ui.add_space(5.0);

            if let Some(selected) = &self.selected_preset {
                if ui.button("Apply Selected Preset").clicked() {
                    action = Some(PresetAction::Apply(selected.clone()));
                }
            }

            // Show "Create Custom EQ" only when DSP is active
            if show_custom_eq {
                if ui.button("Create Custom EQ").clicked() {
                    action = Some(PresetAction::CreateCustom);
                }
            } else {
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("💡 Custom EQ available when DSP streaming is active")
                        .color(egui::Color32::from_rgb(150, 150, 150))
                        .italics()
                        .size(10.0)
                ).on_hover_text("Start DSP streaming to create and use custom EQ presets.\nGo to DSP Server tab, configure output, and start streaming.");
            }
        });

        action
    }
}

pub enum PresetAction {
    Refresh,
    Select(String),
    Apply(String),
    CreateCustom,
}

/// View for DSP/Stream Server output control
pub struct DspView {
    pub selected_sink: SinkType,
    pub available_devices: Vec<String>,
    pub selected_device: Option<String>,
    pub available_input_devices: Vec<String>,
    pub selected_input_device: Option<String>,
    pub sample_rate: u32,
    pub format: FormatOption,
    pub buffer_ms: u32,
    pub is_streaming: bool,
    pub stream_status: Option<StreamStatus>,
    pub show_device_discovery: bool,
    pub discovering: bool,
    pub use_test_tone: bool, // Toggle between captured audio and test tone
    pub audio_viz: AudioVizState, // Audio waveform visualization
    pub selected_preset: Option<String>, // EQ preset for DSP processing
    pub wiim_presets: Vec<String>, // Presets loaded from WiiM device
    pub custom_presets: Vec<String>, // Custom EQ presets saved by user
    pub pre_eq_meter: crate::meter::MeterState, // Pre-EQ audio levels
    pub post_eq_meter: crate::meter::MeterState, // Post-EQ audio levels
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkType {
    LocalDac,
    Dlna,
    AirPlay,
}

impl SinkType {
    #[allow(dead_code)]
    fn as_str(&self) -> &'static str {
        match self {
            SinkType::LocalDac => "Local DAC",
            SinkType::Dlna => "DLNA/UPnP",
            SinkType::AirPlay => "AirPlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatOption {
    F32,
    S24LE,
    S16LE,
}

impl FormatOption {
    fn as_str(&self) -> &'static str {
        match self {
            FormatOption::F32 => "32-bit Float",
            FormatOption::S24LE => "24-bit PCM",
            FormatOption::S16LE => "16-bit PCM",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamStatus {
    pub latency_ms: u32,
    pub frames_written: u64,
    pub underruns: u64,
    pub buffer_fill: f32,
}

impl Default for DspView {
    fn default() -> Self {
        Self {
            selected_sink: SinkType::LocalDac,
            available_devices: vec![],
            selected_device: None,
            available_input_devices: vec![],
            selected_input_device: None,
            sample_rate: 48000,
            format: FormatOption::F32, // Changed from S24LE - Local DAC only supports F32 and S16LE
            buffer_ms: 150,
            is_streaming: false,
            stream_status: None,
            show_device_discovery: false,
            discovering: false,
            use_test_tone: false, // Default to captured audio
            audio_viz: AudioVizState::new(),
            selected_preset: None, // No preset selected by default
            wiim_presets: vec![],
            custom_presets: vec![],
            pre_eq_meter: crate::meter::MeterState::default(),
            post_eq_meter: crate::meter::MeterState::default(),
        }
    }
}

impl DspView {
    pub fn show(&mut self, ui: &mut Ui) -> Option<DspAction> {
        let mut action = None;

        ScrollArea::vertical().show(ui, |ui| {
        ui.group(|ui| {
            ui.heading("Audio Output (DSP)");
            ui.separator();

            // Sink type selector
            ui.horizontal(|ui| {
                ui.label("Output Type:");

                if ui.selectable_label(self.selected_sink == SinkType::LocalDac, "Local DAC").clicked() {
                    self.selected_sink = SinkType::LocalDac;
                    action = Some(DspAction::SinkTypeChanged(SinkType::LocalDac));
                }
                if ui.selectable_label(self.selected_sink == SinkType::Dlna, "DLNA/UPnP").clicked() {
                    self.selected_sink = SinkType::Dlna;
                    action = Some(DspAction::SinkTypeChanged(SinkType::Dlna));
                }
                if ui.selectable_label(self.selected_sink == SinkType::AirPlay, "AirPlay").clicked() {
                    self.selected_sink = SinkType::AirPlay;
                    action = Some(DspAction::SinkTypeChanged(SinkType::AirPlay));
                }
            });

            ui.add_space(5.0);
            ui.separator();

            // Audio Source section
            ui.label("Audio Source:");

            // Test tone toggle
            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.use_test_tone, "Use Test Tone").on_hover_text("Use 1kHz test tone instead of captured audio").changed() {
                    action = Some(DspAction::ToggleTestTone);
                }
            });

            // Input device selection (only show if not using test tone)
            if !self.use_test_tone {
                ui.horizontal(|ui| {
                    ui.label("Input Device:");

                    egui::ComboBox::from_id_salt("input_device_selector")
                        .selected_text(self.selected_input_device.as_deref().unwrap_or("(none)"))
                        .show_ui(ui, |ui| {
                            for device in &self.available_input_devices.clone() {
                                if ui.selectable_label(
                                    self.selected_input_device.as_ref() == Some(device),
                                    device
                                ).clicked() {
                                    self.selected_input_device = Some(device.clone());
                                    action = Some(DspAction::InputDeviceSelected(device.clone()));
                                }
                            }
                        });

                    if ui.button("🔍").on_hover_text("Discover input devices").clicked() {
                        action = Some(DspAction::DiscoverInputDevices);
                    }
                });

                // Check if loopback setup is needed (Linux-specific)
                #[cfg(target_os = "linux")]
                {
                    let has_loopback_device = self.available_input_devices.iter()
                        .any(|d| d.contains("aaeq_capture") || d.contains("aaeq_monitor"));

                    if !has_loopback_device && !self.available_input_devices.is_empty() {
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("💡")
                                    .color(egui::Color32::LIGHT_BLUE)
                            );
                            ui.label(
                                egui::RichText::new("Tip: To capture system audio, run setup-audio-loopback.sh")
                                    .color(egui::Color32::LIGHT_GRAY)
                                    .italics()
                            ).on_hover_text(
                                "This script sets up a virtual audio sink that captures system audio.\n\
                                Run: ./setup-audio-loopback.sh\n\
                                Then discover devices again to see 'aaeq_capture' or 'aaeq_monitor'."
                            );
                        });
                    }
                }
            }

            ui.add_space(5.0);
            ui.separator();

            // Output Device selection
            ui.label("Audio Output:");

            ui.horizontal(|ui| {
                ui.label("Output Device:");

                egui::ComboBox::from_id_salt("device_selector")
                    .selected_text(self.selected_device.as_deref().unwrap_or("(none)"))
                    .show_ui(ui, |ui| {
                        for device in &self.available_devices.clone() {
                            if ui.selectable_label(
                                self.selected_device.as_ref() == Some(device),
                                device
                            ).clicked() {
                                self.selected_device = Some(device.clone());
                                action = Some(DspAction::DeviceSelected(device.clone()));
                            }
                        }
                    });

                if ui.button("🔍 Discover").on_hover_text("Discover available devices").clicked() {
                    self.show_device_discovery = true;
                    self.discovering = true;
                    action = Some(DspAction::DiscoverDevices);
                }
            });

            ui.add_space(5.0);
            ui.separator();
            ui.label("Configuration:");

            // Sample rate selector
            ui.horizontal(|ui| {
                ui.label("Sample Rate:");
                egui::ComboBox::from_id_salt("sample_rate")
                    .selected_text(format!("{} Hz", self.sample_rate))
                    .show_ui(ui, |ui| {
                        for &rate in &[44100, 48000, 96000, 192000] {
                            if ui.selectable_label(
                                self.sample_rate == rate,
                                format!("{} Hz", rate)
                            ).clicked() {
                                self.sample_rate = rate;
                            }
                        }
                    });
            });

            // Format selector
            ui.horizontal(|ui| {
                ui.label("Format:");
                egui::ComboBox::from_id_salt("format")
                    .selected_text(self.format.as_str())
                    .show_ui(ui, |ui| {
                        for &fmt in &[FormatOption::F32, FormatOption::S24LE, FormatOption::S16LE] {
                            if ui.selectable_label(
                                self.format == fmt,
                                fmt.as_str()
                            ).clicked() {
                                self.format = fmt;
                            }
                        }
                    });
            });

            // Buffer size
            ui.horizontal(|ui| {
                ui.label("Buffer:");
                ui.add(egui::Slider::new(&mut self.buffer_ms, 50..=500).suffix(" ms"));
            });

            // EQ Preset selector
            ui.horizontal(|ui| {
                ui.label("EQ Preset:");
                egui::ComboBox::from_id_salt("eq_preset")
                    .selected_text(self.selected_preset.as_deref().unwrap_or("None"))
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.selected_preset.is_none(), "None").clicked() {
                            self.selected_preset = None;
                            action = Some(DspAction::PresetSelected(None));
                        }

                        // Built-in library presets
                        if !crate::preset_library::list_known_presets().is_empty() {
                            ui.label(egui::RichText::new("Built-in Presets").strong().color(egui::Color32::LIGHT_BLUE));
                            ui.separator();
                            for preset_name in crate::preset_library::list_known_presets() {
                                if ui.selectable_label(
                                    self.selected_preset.as_deref() == Some(preset_name),
                                    preset_name
                                ).clicked() {
                                    self.selected_preset = Some(preset_name.to_string());
                                    action = Some(DspAction::PresetSelected(Some(preset_name.to_string())));
                                }
                            }
                        }

                        // Custom EQ presets
                        if !self.custom_presets.is_empty() {
                            ui.separator();
                            ui.label(egui::RichText::new("Custom Presets").strong().color(egui::Color32::from_rgb(255, 180, 100)));
                            ui.separator();
                            for preset_name in &self.custom_presets.clone() {
                                if ui.selectable_label(
                                    self.selected_preset.as_deref() == Some(preset_name.as_str()),
                                    preset_name
                                ).clicked() {
                                    self.selected_preset = Some(preset_name.clone());
                                    action = Some(DspAction::PresetSelected(Some(preset_name.clone())));
                                }
                            }
                        }

                        // WiiM device presets
                        if !self.wiim_presets.is_empty() {
                            ui.separator();
                            ui.label(egui::RichText::new("WiiM Device Presets").strong().color(egui::Color32::LIGHT_GREEN));
                            ui.separator();
                            for preset_name in &self.wiim_presets.clone() {
                                if ui.selectable_label(
                                    self.selected_preset.as_deref() == Some(preset_name.as_str()),
                                    preset_name
                                ).clicked() {
                                    self.selected_preset = Some(preset_name.clone());
                                    action = Some(DspAction::PresetSelected(Some(preset_name.clone())));
                                }
                            }
                        }
                    });
            });

            ui.add_space(10.0);
            ui.separator();

            // Start/Stop controls
            ui.horizontal(|ui| {
                if !self.is_streaming {
                    if ui.button("▶ Start Streaming").clicked() {
                        action = Some(DspAction::StartStreaming);
                    }
                } else {
                    if ui.button("⏹ Stop Streaming").clicked() {
                        action = Some(DspAction::StopStreaming);
                    }
                }

                if self.is_streaming {
                    ui.label("🔴 Streaming");
                } else {
                    ui.label("⚪ Stopped");
                }
            });

            // Stream status display
            if let Some(status) = &self.stream_status {
                ui.add_space(10.0);
                ui.separator();
                ui.label("Stream Status:");

                ui.horizontal(|ui| {
                    ui.label(format!("Latency: {} ms", status.latency_ms));
                    ui.label(format!("Frames: {}", status.frames_written));
                });

                if status.underruns > 0 {
                    ui.label(
                        egui::RichText::new(format!("⚠ Underruns: {}", status.underruns))
                            .color(egui::Color32::YELLOW)
                    );
                }

                // Buffer fill indicator
                ui.horizontal(|ui| {
                    ui.label("Buffer:");
                    let progress_bar = egui::ProgressBar::new(status.buffer_fill)
                        .text(format!("{:.0}%", status.buffer_fill * 100.0));
                    ui.add(progress_bar);
                });
            }

            ui.add_space(5.0);

            // Audio visualization toggle
            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.audio_viz.enabled, "Show Waveform").on_hover_text("Display real-time audio waveform visualization").changed() {
                    action = Some(DspAction::ToggleVisualization);
                }
            });

            // Show audio visualization if enabled
            if self.audio_viz.enabled {
                ui.add_space(5.0);
                ui.separator();
                self.audio_viz.show(ui);
            }

            // Audio level meters (only show when streaming)
            if self.is_streaming {
                ui.add_space(10.0);
                ui.separator();
                ui.label("Audio Levels:");

                // Update meter ballistics
                self.pre_eq_meter.tick();
                self.post_eq_meter.tick();

                // Display meters side by side, using available width like waveform
                ui.horizontal(|ui| {
                    let available_width = ui.available_width();
                    let spacing = 20.0;
                    let meter_width = (available_width - spacing) / 2.0;
                    let meter_height = 200.0;

                    // Pre-EQ meter
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Pre-EQ").size(16.0).strong());
                        let (meter_rect, _) = ui.allocate_exact_size(
                            egui::vec2(meter_width, meter_height),
                            egui::Sense::hover()
                        );
                        if ui.is_rect_visible(meter_rect) {
                            let painter = ui.painter_at(meter_rect);
                            crate::meter::draw_mc_style_meter(ui, meter_rect, &painter, &self.pre_eq_meter);
                        }
                    });

                    ui.add_space(spacing);

                    // Post-EQ meter
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Post-EQ").size(16.0).strong());
                        let (meter_rect, _) = ui.allocate_exact_size(
                            egui::vec2(meter_width, meter_height),
                            egui::Sense::hover()
                        );
                        if ui.is_rect_visible(meter_rect) {
                            let painter = ui.painter_at(meter_rect);
                            crate::meter::draw_mc_style_meter(ui, meter_rect, &painter, &self.post_eq_meter);
                        }
                    });
                });
            }

            ui.add_space(5.0);

            // Test controls
            if ui.button("🔊 Test Tone").on_hover_text("Play a 1kHz test tone for 2 seconds").clicked() {
                action = Some(DspAction::PlayTestTone);
            }
        });

        });  // End of ScrollArea

        // Device discovery dialog
        if self.show_device_discovery {
            egui::Window::new("Discover Devices")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    if self.discovering && self.available_devices.is_empty() {
                        ui.label("Scanning for devices...");
                        ui.spinner();
                    } else if self.available_devices.is_empty() {
                        ui.label("No devices found");
                    } else {
                        ui.label(format!("Found {} device(s):", self.available_devices.len()));
                        ui.separator();

                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for device in &self.available_devices.clone() {
                                if ui.button(device).clicked() {
                                    self.selected_device = Some(device.clone());
                                    action = Some(DspAction::DeviceSelected(device.clone()));
                                    self.show_device_discovery = false;
                                    self.discovering = false;
                                }
                            }
                        });
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Scan Again").clicked() {
                            self.discovering = true;
                            action = Some(DspAction::DiscoverDevices);
                        }

                        if ui.button("Close").clicked() {
                            self.show_device_discovery = false;
                            self.discovering = false;
                        }
                    });
                });
        }

        action
    }
}

pub enum DspAction {
    SinkTypeChanged(SinkType),
    DeviceSelected(String),
    DiscoverDevices,
    ToggleTestTone,
    InputDeviceSelected(String),
    DiscoverInputDevices,
    StartStreaming,
    StopStreaming,
    PlayTestTone,
    ToggleVisualization,
    PresetSelected(Option<String>),
    SaveCustomPreset(EqPreset),
}
