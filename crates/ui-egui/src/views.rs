use aaeq_core::{EqPreset, TrackMeta, Scope};
use crate::audio_viz::AudioVizState;
use crate::widgets::VerticalSlider;
use crate::album_art::{AlbumArtCache, AlbumArtState};
use egui::{Context, ScrollArea, Ui};
use std::sync::Arc;

// Import dithering and resampling types from stream-server
pub use stream_server::dsp::{DitherMode, NoiseShaping, ResamplerQuality};

/// View for creating/editing EQ presets with vertical sliders
pub struct EqEditorView {
    pub preset: EqPreset,
    pub preset_name: String,
    pub existing_presets: Vec<String>, // List of existing preset names for validation
    pub name_error: Option<String>,    // Error message if name is invalid
    pub edit_mode: bool,               // True if editing existing preset, false if creating new
    pub original_name: Option<String>, // Original preset name when editing (for validation)
    last_live_update: std::time::Instant, // Timestamp of last live update (for throttling)
}

impl Default for EqEditorView {
    fn default() -> Self {
        Self {
            preset: EqPreset::default(),
            preset_name: "Custom".to_string(),
            existing_presets: vec![],
            name_error: None,
            edit_mode: false,
            original_name: None,
            last_live_update: std::time::Instant::now() - std::time::Duration::from_secs(1),
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
            edit_mode: false,
            original_name: None,
            last_live_update: std::time::Instant::now() - std::time::Duration::from_secs(1),
        }
    }

    /// Create an editor for editing an existing preset
    pub fn new_for_edit(preset: EqPreset) -> Self {
        let original_name = preset.name.clone();
        Self {
            preset_name: preset.name.clone(),
            preset,
            existing_presets: vec![],
            name_error: None,
            edit_mode: true,
            original_name: Some(original_name),
            last_live_update: std::time::Instant::now() - std::time::Duration::from_secs(1),
        }
    }

    /// Find a unique name by appending a number if the base name already exists
    fn find_unique_name(&self, base_name: &str) -> String {
        if !self.existing_presets.iter().any(|p| p == base_name) {
            return base_name.to_string();
        }

        // Try appending numbers until we find a unique name
        for i in 2..=100 {
            let candidate = format!("{} {}", base_name, i);
            if !self.existing_presets.iter().any(|p| p == &candidate) {
                return candidate;
            }
        }

        // Fallback: append timestamp
        format!("{} {}", base_name, chrono::Local::now().format("%Y%m%d%H%M%S"))
    }

    /// Set the list of existing presets and auto-fix name conflicts for new presets
    pub fn set_existing_presets(&mut self, presets: Vec<String>) {
        self.existing_presets = presets;

        // Only auto-fix name conflicts when creating new presets (not in edit mode)
        if !self.edit_mode && self.check_name_conflict() {
            let unique_name = self.find_unique_name(&self.preset_name);
            tracing::info!("Auto-renamed preset from '{}' to '{}' to avoid conflict", self.preset_name, unique_name);
            self.preset_name = unique_name;
            self.name_error = None;
        }
    }

    fn check_name_conflict(&self) -> bool {
        // In edit mode, allow keeping the original name
        if self.edit_mode {
            if let Some(ref original) = self.original_name {
                if &self.preset_name == original {
                    return false; // Same name as original is OK
                }
            }
        }
        self.existing_presets.iter().any(|p| p == &self.preset_name)
    }

    pub fn show(&mut self, ctx: &Context) -> Option<EqEditorAction> {
        let mut action = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            // Show different heading based on mode
            let heading = if self.edit_mode {
                "Edit EQ Preset"
            } else {
                "Create EQ Preset"
            };
            ui.heading(heading);
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Preset Name:");
                let text_edit = egui::TextEdit::singleline(&mut self.preset_name);
                let response = ui.add(text_edit);

                // Auto-select text on first open for easy editing
                if !self.edit_mode && (self.preset_name == "Custom" || self.preset_name.starts_with("Custom ")) {
                    response.request_focus();
                }

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

            // Check if using generic auto-generated name
            let is_generic_name = self.preset_name == "Custom" ||
                                  self.preset_name.starts_with("Custom ");

            // Show error message if name is invalid with auto-fix button
            if let Some(error) = &self.name_error.clone() {
                let has_conflict = self.check_name_conflict();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", error))
                            .color(egui::Color32::from_rgb(255, 100, 100))
                            .strong()
                    );

                    // Offer auto-fix button for name conflicts (not for empty names)
                    if has_conflict {
                        if ui.button("Auto-fix").on_hover_text("Automatically choose a unique name").clicked() {
                            let unique_name = self.find_unique_name(&self.preset_name);
                            self.preset_name = unique_name;
                            self.name_error = None;
                        }
                    }
                });
            } else if is_generic_name && !self.edit_mode {
                // Friendly reminder to use descriptive names
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("💡 Tip: Use a descriptive name like 'Bass Boost', 'Vocal Clarity', 'Headphones', etc.")
                            .color(egui::Color32::from_rgb(200, 200, 100))
                            .italics()
                    );
                });
            }

            ui.add_space(10.0);

            // EQ sliders in a horizontal layout
            ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut slider_changed = false;
                    for band in &mut self.preset.bands {
                        ui.vertical(|ui| {
                            let label = format_frequency(band.frequency);
                            let slider = VerticalSlider::new(
                                &mut band.gain,
                                -12.0..=12.0,
                                label,
                            );
                            let response = ui.add(slider);
                            if response.changed() {
                                slider_changed = true;
                            }
                        });
                        ui.add_space(5.0);
                    }

                    // Send live update when slider changes (for real-time preview)
                    // Throttle updates to prevent crackling (max 10 updates/sec)
                    if slider_changed {
                        let now = std::time::Instant::now();
                        let elapsed = now.duration_since(self.last_live_update);

                        if elapsed >= std::time::Duration::from_millis(100) {
                            self.last_live_update = now;
                            let mut preview_preset = self.preset.clone();
                            preview_preset.name = self.preset_name.clone();
                            action = Some(EqEditorAction::LiveUpdate(preview_preset));
                        }
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

                // Disable Save button if name is invalid
                let can_save = self.name_error.is_none();

                ui.add_enabled_ui(can_save, |ui| {
                    if ui.button("Save Preset").on_hover_text_at_pointer(
                        if can_save {
                            "Save preset to database (already applied via live preview)"
                        } else {
                            "Fix name errors before saving"
                        }
                    ).clicked() {
                        self.preset.name = self.preset_name.clone();
                        action = Some(EqEditorAction::Save(self.preset.clone()));
                    }
                });
            });
        });

        action
    }
}

pub enum EqEditorAction {
    Modified,
    LiveUpdate(EqPreset), // Real-time preview while editing (only when streaming)
    Save(EqPreset),       // Save to database (preset already applied via live preview)
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

        // Load default icon on first run (embedded at compile time)
        if self.default_icon_texture.is_none() {
            // Load the default icon from embedded bytes
            let icon_bytes = include_bytes!("../../../aaeq-icon.png");
            match image::load_from_memory(icon_bytes) {
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

        // Wrap entire Now Playing section in a scroll area to prevent buttons from being cut off
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
        ui.group(|ui| {
            ui.heading("Now Playing");

            if let Some(track) = &self.track {
                // Handle album art loading and display
                if let Some(art_url) = &track.album_art_url {
                    tracing::debug!("Track has album art URL: {} (last: {:?}, texture: {})",
                        art_url, self.last_album_art_url, self.album_art_texture.is_some());

                    // Check if URL changed - if so, clear cached texture
                    if self.last_album_art_url.as_ref() != Some(art_url) {
                        tracing::debug!("Album art URL changed from {:?} to {}, clearing cached texture",
                            self.last_album_art_url, art_url);
                        self.album_art_texture = None;
                        self.last_album_art_url = Some(art_url.clone());
                    }

                    // Handle lookup:// URLs (album art lookup from external services)
                    if art_url.starts_with("lookup://") {
                        // Extract artist and album from lookup URL
                        if let Some(metadata) = art_url.strip_prefix("lookup://") {
                            let parts: Vec<&str> = metadata.split('|').collect();
                            if parts.len() == 2 {
                                let artist = parts[0];
                                let album = parts[1];

                                // Check if we've already looked up this artist/album
                                let lookup_cache_key = format!("looked_up:{}", art_url);
                                tracing::debug!("Checking lookup cache for key: {}, texture_is_none: {}",
                                    lookup_cache_key, self.album_art_texture.is_none());

                                if self.album_art_texture.is_none() {
                                    // Try to get cached state, default to NotLoaded if not found
                                    let state = album_art_cache.try_get(&lookup_cache_key)
                                        .unwrap_or(AlbumArtState::NotLoaded);

                                    tracing::debug!("Cache state for {}: {:?}", lookup_cache_key,
                                        match &state {
                                            AlbumArtState::NotLoaded => "NotLoaded",
                                            AlbumArtState::Loading => "Loading",
                                            AlbumArtState::Loaded(_) => "Loaded",
                                            AlbumArtState::Failed => "Failed",
                                        });

                                    match state {
                                        AlbumArtState::NotLoaded => {
                                            // Start lookup
                                            tracing::info!("Starting album art lookup for: {} - {}", artist, album);
                                            let artist = artist.to_string();
                                            let album = album.to_string();
                                            let cache = album_art_cache.clone();
                                            let cache_key = lookup_cache_key.clone();

                                            // Mark as loading immediately
                                            album_art_cache.mark_loading(lookup_cache_key.clone());

                                            // Spawn lookup task
                                            tokio::spawn(async move {
                                                match crate::album_art_lookup::lookup_album_art(&artist, &album).await {
                                                    Ok(Some(url)) => {
                                                        tracing::info!("Album art lookup succeeded: {}", url);
                                                        // Load the actual image URL but cache it under the lookup key
                                                        cache.load_as(url, cache_key);
                                                    }
                                                    Ok(None) => {
                                                        tracing::debug!("Album art lookup returned no results");
                                                        // Mark as failed in cache so we don't retry
                                                        cache.mark_failed(cache_key);
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!("Album art lookup failed: {}", e);
                                                        cache.mark_failed(cache_key);
                                                    }
                                                }
                                            });

                                            ui.ctx().request_repaint();
                                        }
                                        AlbumArtState::Loading => {
                                            tracing::trace!("Album art lookup in progress...");
                                            ui.ctx().request_repaint();
                                        }
                                        AlbumArtState::Loaded(color_image) => {
                                            // Convert to texture
                                            tracing::debug!("Album art from lookup loaded, converting to texture");
                                            let texture = ui.ctx().load_texture(
                                                &format!("album_art_{}", art_url),
                                                color_image.as_ref().clone(),
                                                Default::default(),
                                            );
                                            self.album_art_texture = Some(texture);
                                        }
                                        AlbumArtState::Failed => {
                                            tracing::trace!("Album art lookup previously failed");
                                            // Don't retry
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // Handle regular HTTP/HTTPS URLs
                        if self.album_art_texture.is_none() {
                            if let Some(state) = album_art_cache.try_get(art_url) {
                                match state {
                                    AlbumArtState::NotLoaded => {
                                        // Start loading
                                        tracing::debug!("Album art not loaded, starting load for: {}", art_url);
                                        album_art_cache.load(art_url.clone());
                                        ui.ctx().request_repaint();
                                    }
                                    AlbumArtState::Loading => {
                                        // Still loading, request repaint to check again
                                        tracing::trace!("Album art still loading...");
                                        ui.ctx().request_repaint();
                                    }
                                    AlbumArtState::Loaded(color_image) => {
                                        // Convert ColorImage to texture
                                        tracing::debug!("Album art loaded successfully, converting to texture");
                                        let texture = ui.ctx().load_texture(
                                            &format!("album_art_{}", art_url),
                                            color_image.as_ref().clone(),
                                            Default::default(),
                                        );
                                        self.album_art_texture = Some(texture);
                                    }
                                    AlbumArtState::Failed => {
                                        // Failed to load, don't retry
                                        tracing::warn!("Album art failed to load for: {}", art_url);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    tracing::debug!("Track has no album art URL");
                }

                // Display layout with album art
                ui.horizontal(|ui| {
                    // Album art on the left - show album art if available, otherwise show default icon
                    if let Some(texture) = &self.album_art_texture {
                        // Display at larger size for better quality, but not full resolution
                        ui.add(egui::Image::new(texture).max_size(egui::vec2(250.0, 250.0)));
                        ui.add_space(10.0);
                    } else if let Some(default_texture) = &self.default_icon_texture {
                        // Show default icon when no album art is loaded (scaled to reasonable size)
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

                    // Always show EQ Curve section when there's a preset
                    ui.add_space(5.0);
                    ui.separator();

                    // Reserve fixed height for entire EQ curve section to prevent jumping
                    ui.vertical(|ui| {
                        ui.set_min_height(155.0); // Fixed height for label + bars + spacing

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

                        // Visual EQ display - show spinner if loading, bars if available
                        if let Some(eq_preset) = &self.current_preset_curve {
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
                        } else {
                            // Show loading spinner while curve is being fetched
                            ui.add_space(40.0); // Add space to center vertically
                            ui.horizontal(|ui| {
                                ui.add(egui::Spinner::new());
                                ui.label(
                                    egui::RichText::new("Loading EQ curve...")
                                        .color(egui::Color32::GRAY)
                                        .italics()
                                );
                            });
                        }
                    }); // End of ui.vertical with min_height
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
        }); // End of ScrollArea

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
                // Show WiiM device presets if connected, otherwise show default WiiM presets
                let presets_to_show = if !self.presets.is_empty() {
                    &self.presets
                } else {
                    // Show default WiiM presets when no device is connected
                    &crate::preset_library::list_known_presets()
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                };

                if !presets_to_show.is_empty() {
                    let label = if !self.presets.is_empty() {
                        "Device Presets"
                    } else {
                        "Default EQ Presets"
                    };
                    ui.label(egui::RichText::new(label).strong().color(egui::Color32::LIGHT_GREEN));
                    ui.separator();
                    for preset in presets_to_show {
                        let is_selected = self.selected_preset.as_deref() == Some(preset.as_str());
                        if ui.selectable_label(is_selected, preset).clicked() {
                            self.selected_preset = Some(preset.clone());
                            action = Some(PresetAction::Select(preset.clone()));
                        }
                    }
                }

                // Show custom EQ presets (only when DSP is streaming)
                if show_custom_eq && !self.custom_presets.is_empty() {
                    if !presets_to_show.is_empty() {
                        ui.add_space(5.0);
                    }
                    ui.label(egui::RichText::new("Custom Presets").strong().color(egui::Color32::from_rgb(255, 180, 100)));
                    ui.separator();
                    for preset in &self.custom_presets.clone() {
                        ui.horizontal(|ui| {
                            let is_selected = self.selected_preset.as_deref() == Some(preset.as_str());
                            let response = ui.selectable_label(is_selected, preset);

                            // Single click selects
                            if response.clicked() {
                                self.selected_preset = Some(preset.clone());
                                action = Some(PresetAction::Select(preset.clone()));
                            }

                            // Double click edits
                            if response.double_clicked() {
                                action = Some(PresetAction::EditCustom(preset.clone()));
                            }

                            // Delete button
                            if ui.small_button("🗑").on_hover_text("Delete this preset").clicked() {
                                action = Some(PresetAction::DeleteCustom(preset.clone()));
                            }
                        });
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
    EditCustom(String),   // Edit existing custom preset by name
    DeleteCustom(String), // Delete custom preset by name
}

/// View for DSP/Stream Server output control
pub struct DspView {
    pub selected_sink: SinkType,
    pub available_devices: Vec<String>, // Legacy - kept for compatibility
    pub available_local_devices: Vec<String>, // Local DAC devices
    pub available_dlna_devices: Vec<String>, // DLNA/UPnP devices
    pub available_airplay_devices: Vec<String>, // AirPlay devices
    pub selected_device: Option<String>,
    // Store last selected device per sink type
    pub last_local_dac_device: Option<String>,
    pub last_dlna_device: Option<String>,
    pub last_airplay_device: Option<String>,
    pub available_input_devices: Vec<String>,
    pub selected_input_device: Option<String>,
    pub sample_rate: u32,
    pub format: FormatOption,
    pub buffer_ms: u32,
    pub is_streaming: bool,
    pub is_starting: bool, // True while waiting for streaming to start (for loading spinner)
    pub stream_status: Option<StreamStatus>,
    pub show_device_discovery: bool,
    pub discovering: bool,
    pub use_test_tone: bool, // Toggle between captured audio and test tone
    pub audio_viz: AudioVizState, // Audio waveform visualization
    pub spectrum_analyzer: crate::spectrum_analyzer::SpectrumAnalyzerState, // Spectrum analyzer
    pub viz_mode: VisualizationMode, // Current visualization mode
    pub current_active_preset: Option<String>, // Actual active EQ preset from EQ Management
    pub pre_eq_meter: crate::meter::MeterState, // Pre-EQ audio levels
    pub post_eq_meter: crate::meter::MeterState, // Post-EQ audio levels
    pub show_meters: bool, // Toggle to show/hide audio level meters
    pub audio_output_collapsed: bool, // Track collapse state of Audio Output section
    pub viz_delay_ms: u32, // Visualization delay in milliseconds for network streaming sync
    viz_delay_auto_set: bool, // Track if delay was auto-set for current streaming session
    viz_sample_buffer: std::collections::VecDeque<(std::time::Instant, Vec<f64>)>, // Buffered samples with timestamps
    viz_metrics_buffer: std::collections::VecDeque<(std::time::Instant, VizMetrics)>, // Buffered metrics with timestamps
    // Headroom control settings
    pub headroom_db: f32, // Headroom in dB (0 to -6)
    pub auto_compensate: bool, // Apply makeup gain (future feature)
    pub clip_detection: bool, // Enable clip detection
    pub clip_count: u64, // Number of detected clips
    // Dithering settings
    pub dither_enabled: bool, // Enable dithering
    pub dither_mode: DitherMode, // Dithering algorithm
    pub noise_shaping: NoiseShaping, // Noise shaping curve
    pub target_bits: u8, // Target bit depth (16/24/32)
    // Resampling settings
    pub resample_enabled: bool, // Enable resampling
    pub resample_quality: ResamplerQuality, // Resampling quality preset
    pub target_sample_rate: u32, // Target sample rate (Hz)
    // Pipeline visualization
    pub pipeline_view: crate::pipeline_view::PipelineView,
}

/// Struct to hold visualization metrics for buffering
#[derive(Clone)]
pub struct VizMetrics {
    pub pre_eq_rms_l: f32,
    pub pre_eq_rms_r: f32,
    pub pre_eq_peak_l: f32,
    pub pre_eq_peak_r: f32,
    pub post_eq_rms_l: f32,
    pub post_eq_rms_r: f32,
    pub post_eq_peak_l: f32,
    pub post_eq_peak_r: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizationMode {
    Waveform,
    Spectrum,
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
            available_local_devices: vec![],
            available_dlna_devices: vec![],
            available_airplay_devices: vec![],
            selected_device: None,
            last_local_dac_device: None,
            last_dlna_device: None,
            last_airplay_device: None,
            available_input_devices: vec![],
            selected_input_device: None,
            sample_rate: 48000,
            format: FormatOption::F32, // Changed from S24LE - Local DAC only supports F32 and S16LE
            buffer_ms: 150,
            is_streaming: false,
            is_starting: false,
            stream_status: None,
            show_device_discovery: false,
            discovering: false,
            use_test_tone: false, // Default to captured audio
            audio_viz: AudioVizState::new(),
            spectrum_analyzer: crate::spectrum_analyzer::SpectrumAnalyzerState::new(),
            viz_mode: VisualizationMode::Waveform, // Default to waveform
            current_active_preset: None, // No active preset initially
            pre_eq_meter: crate::meter::MeterState::default(),
            post_eq_meter: crate::meter::MeterState::default(),
            show_meters: false, // Start hidden by default
            audio_output_collapsed: false, // Start expanded by default
            viz_delay_ms: 0, // No delay by default (for Local DAC)
            viz_delay_auto_set: false, // Will auto-set on first stream status
            viz_sample_buffer: std::collections::VecDeque::new(),
            viz_metrics_buffer: std::collections::VecDeque::new(),
            // Headroom control defaults
            headroom_db: -3.0, // Default -3 dB headroom
            auto_compensate: false, // Disabled by default
            clip_detection: true, // Enabled by default
            clip_count: 0, // No clips initially
            // Dithering defaults
            dither_enabled: false, // Disabled by default
            dither_mode: DitherMode::Triangular, // TPDF is industry standard
            noise_shaping: NoiseShaping::None, // No shaping by default
            target_bits: 16, // Target 16-bit output (CD quality)
            // Resampling defaults
            resample_enabled: false, // Disabled by default
            resample_quality: ResamplerQuality::Balanced, // Balanced quality
            target_sample_rate: 48000, // Target 48 kHz (studio standard)
            // Pipeline visualization
            pipeline_view: crate::pipeline_view::PipelineView::new(),
        }
    }
}

impl DspView {
    pub fn show(&mut self, ui: &mut Ui, theme: &crate::theme::Theme) -> Option<DspAction> {
        let mut action = None;
        let meter_colors = theme.meter_colors();
        let spectrum_colors = theme.spectrum_colors();

        ScrollArea::vertical().show(ui, |ui| {
        // Update and display pipeline visualization
        self.update_pipeline_view();
        if let Some(pipeline_action) = self.pipeline_view.show(ui, theme) {
            action = Some(self.handle_pipeline_action(pipeline_action));
        }

        ui.add_space(10.0);

        ui.group(|ui| {
            // Collapsible header with streaming controls
            ui.horizontal(|ui| {
                // Expand/collapse button with drawn arrow (like pipeline icons)
                let tooltip = if self.audio_output_collapsed {
                    "Expand Audio Output section"
                } else {
                    "Collapse Audio Output section"
                };

                let button_size = egui::Vec2::new(28.0, 28.0);
                let (button_rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());

                // Draw button background
                if ui.is_rect_visible(button_rect) {
                    let visuals = ui.style().interact(&response);
                    ui.painter().rect_filled(button_rect, 3.0, visuals.bg_fill);
                    ui.painter().rect_stroke(button_rect, 3.0, visuals.bg_stroke);

                    // Draw arrow using painter
                    let center = button_rect.center();
                    let arrow_color = visuals.text_color();
                    let stroke = egui::Stroke::new(2.0, arrow_color);

                    if self.audio_output_collapsed {
                        // Right-pointing arrow (►)
                        let tip = center + egui::Vec2::new(6.0, 0.0);
                        let top = center + egui::Vec2::new(-4.0, -5.0);
                        let bottom = center + egui::Vec2::new(-4.0, 5.0);
                        ui.painter().line_segment([top, tip], stroke);
                        ui.painter().line_segment([bottom, tip], stroke);
                    } else {
                        // Down-pointing arrow (▼)
                        let tip = center + egui::Vec2::new(0.0, 6.0);
                        let left = center + egui::Vec2::new(-5.0, -4.0);
                        let right = center + egui::Vec2::new(5.0, -4.0);
                        ui.painter().line_segment([left, tip], stroke);
                        ui.painter().line_segment([right, tip], stroke);
                    }
                }

                if response.on_hover_text(tooltip).clicked() {
                    self.audio_output_collapsed = !self.audio_output_collapsed;
                    // Request window resize after collapse/expand
                    ui.ctx().request_repaint();
                }

                ui.heading("Audio Output (DSP)");

                ui.add_space(10.0);

                // Start/Stop controls and EQ Status (always visible) - in horizontal layout
                ui.horizontal(|ui| {
                    // Start/Stop button
                    if !self.is_streaming && !self.is_starting {
                        if ui.add_sized([180.0, 30.0], egui::Button::new("▶ Start Streaming")).clicked() {
                            action = Some(DspAction::StartStreaming);
                        }
                    } else if self.is_starting {
                        // Show spinner while connecting
                        ui.add_enabled_ui(false, |ui| {
                            ui.add_sized([180.0, 30.0], |ui: &mut egui::Ui| {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Connecting...");
                                }).response
                            });
                        });
                    } else {
                        if ui.add_sized([180.0, 30.0], egui::Button::new("⏹ Stop Streaming")).clicked() {
                            action = Some(DspAction::StopStreaming);
                        }
                    }

                    // Status indicator beside button
                    if self.is_streaming {
                        // Draw green circle
                        let size = egui::Vec2::new(10.0, 10.0);
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        if ui.is_rect_visible(rect) {
                            ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::from_rgb(50, 205, 50));
                        }

                        ui.label(
                            egui::RichText::new("STREAMING")
                                .size(14.0)
                                .color(egui::Color32::from_rgb(50, 205, 50))
                                .strong()
                        );
                    } else {
                        // Draw red circle
                        let size = egui::Vec2::new(10.0, 10.0);
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        if ui.is_rect_visible(rect) {
                            ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::from_rgb(220, 20, 60));
                        }

                        ui.label(
                            egui::RichText::new("STOPPED")
                                .size(14.0)
                                .color(egui::Color32::from_rgb(220, 20, 60))
                                .strong()
                        );
                    }

                    // Vertical separator
                    ui.separator();

                    // EQ Status - always visible
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("EQ Status:").strong().size(11.0));
                        ui.horizontal(|ui| {
                            if self.is_streaming {
                                if let Some(preset_name) = &self.current_active_preset {
                                    // Draw green circle for active EQ
                                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::from_rgb(50, 205, 50));
                                    ui.label(
                                        egui::RichText::new(format!("ACTIVE: {}", preset_name))
                                            .color(egui::Color32::from_rgb(50, 205, 50))
                                            .strong()
                                            .size(11.0)
                                    );
                                } else {
                                    // Draw grey circle for bypassed EQ
                                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::from_rgb(100, 100, 100));
                                    ui.label(
                                        egui::RichText::new("BYPASSED (Flat)")
                                            .color(egui::Color32::from_rgb(150, 150, 150))
                                            .size(11.0)
                                    );
                                }
                            } else {
                                // Draw grey circle for stopped
                                let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(10.0, 10.0), egui::Sense::hover());
                                ui.painter().circle_filled(rect.center(), 5.0, egui::Color32::from_rgb(100, 100, 100));
                                ui.label(
                                    egui::RichText::new("STOPPED")
                                        .color(egui::Color32::from_rgb(150, 150, 150))
                                        .size(11.0)
                                );
                            }
                        });
                        // Help text for EQ Status
                        ui.label(
                            egui::RichText::new("EQ presets are managed in the EQ Management tab")
                                .size(9.0)
                                .color(egui::Color32::GRAY)
                                .italics()
                        );
                    });
                });
            });

            // Only show details when not collapsed
            if !self.audio_output_collapsed {
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

            // Warning about feedback loop for Local DAC without test tone
            if self.selected_sink == SinkType::LocalDac && !self.use_test_tone {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    // Draw warning icon as exclamation mark in a circle
                    let (icon_rect, _) = ui.allocate_exact_size(egui::Vec2::new(16.0, 16.0), egui::Sense::hover());
                    if ui.is_rect_visible(icon_rect) {
                        let painter = ui.painter();
                        let center = icon_rect.center();
                        let warning_color = egui::Color32::from_rgb(255, 200, 0);

                        // Draw circle
                        painter.circle_stroke(center, 7.0, egui::Stroke::new(2.0, warning_color));

                        // Draw exclamation mark
                        painter.line_segment(
                            [center + egui::Vec2::new(0.0, -4.0), center + egui::Vec2::new(0.0, 1.0)],
                            egui::Stroke::new(2.0, warning_color)
                        );
                        painter.circle_filled(center + egui::Vec2::new(0.0, 4.0), 1.0, warning_color);
                    }

                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Feedback Loop Warning")
                                .color(egui::Color32::from_rgb(255, 200, 0))
                                .strong()
                        );
                        ui.label(
                            egui::RichText::new("Using audio input with Local DAC may cause feedback.")
                                .color(egui::Color32::LIGHT_GRAY)
                                .size(10.0)
                        );
                        ui.label(
                            egui::RichText::new("Enable 'Use Test Tone' below to avoid feedback loop.")
                                .color(egui::Color32::LIGHT_GRAY)
                                .size(10.0)
                        );
                    });
                });
            }

            // Visualization delay control (only for network streaming)
            if matches!(self.selected_sink, SinkType::Dlna | SinkType::AirPlay) {
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.label("Visualization Delay:");

                    let prev_delay = self.viz_delay_ms;
                    ui.add(egui::Slider::new(&mut self.viz_delay_ms, 0..=5000)
                        .suffix(" ms")
                        .text(""))
                        .on_hover_text("Delay visualization to sync with network device playback.\nAuto-detected on first stream, or adjust manually.");

                    // Clear buffers if delay changed
                    if self.viz_delay_ms != prev_delay {
                        self.clear_buffers();
                        // Disable auto-detection once user manually adjusts
                        self.viz_delay_auto_set = true;
                    }

                    // Auto button to manually trigger auto-detection
                    if let Some(status) = &self.stream_status {
                        if ui.button("Auto").on_hover_text("Re-detect delay from stream latency").clicked() {
                            self.auto_set_delay_from_latency(status.latency_ms);
                            self.clear_buffers();
                            self.viz_delay_auto_set = true; // Mark as set
                        }
                    }
                });

                // Show current delay value with auto-detection indicator
                if self.viz_delay_ms > 0 {
                    let delay_text = if self.viz_delay_auto_set && self.stream_status.is_some() {
                        format!("Current delay: {} ms (auto-detected)", self.viz_delay_ms)
                    } else {
                        format!("Current delay: {} ms", self.viz_delay_ms)
                    };
                    ui.label(
                        egui::RichText::new(delay_text)
                            .size(10.0)
                            .color(egui::Color32::GRAY)
                    );
                }
            }

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

                // Check if loopback setup is needed (Windows-specific)
                #[cfg(target_os = "windows")]
                {
                    let has_loopback_device = self.available_input_devices.iter()
                        .any(|d| d.contains("(Loopback)"));

                    if !has_loopback_device && !self.available_input_devices.is_empty() {
                        ui.add_space(5.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("💡")
                                        .color(egui::Color32::LIGHT_BLUE)
                                );
                                ui.label(
                                    egui::RichText::new("To capture system audio on Windows:")
                                        .color(egui::Color32::LIGHT_GRAY)
                                        .strong()
                                );
                            });
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new("  1. WASAPI Loopback devices may appear after clicking 🔍 Discover")
                                    .color(egui::Color32::LIGHT_GRAY)
                                    .italics()
                                    .size(10.0)
                            );
                            ui.label(
                                egui::RichText::new("  2. Enable 'Stereo Mix' in Sound Settings → Recording → Show Disabled Devices")
                                    .color(egui::Color32::LIGHT_GRAY)
                                    .italics()
                                    .size(10.0)
                            );
                            ui.label(
                                egui::RichText::new("  3. Or install VB-Audio Virtual Cable for system audio capture")
                                    .color(egui::Color32::LIGHT_GRAY)
                                    .italics()
                                    .size(10.0)
                            );
                        });
                    } else if has_loopback_device {
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("✓")
                                    .color(egui::Color32::LIGHT_GREEN)
                            );
                            ui.label(
                                egui::RichText::new("WASAPI loopback devices available - select one marked with 🔊 (Loopback)")
                                    .color(egui::Color32::LIGHT_GRAY)
                                    .italics()
                                    .size(10.0)
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

                // Filter devices based on selected sink type
                let devices_to_show = match self.selected_sink {
                    SinkType::LocalDac => &self.available_local_devices,
                    SinkType::Dlna => &self.available_dlna_devices,
                    SinkType::AirPlay => &self.available_airplay_devices,
                };

                egui::ComboBox::from_id_salt("device_selector")
                    .selected_text(self.selected_device.as_deref().unwrap_or("(none)"))
                    .show_ui(ui, |ui| {
                        for device in devices_to_show.clone() {
                            if ui.selectable_label(
                                self.selected_device.as_ref() == Some(&device),
                                &device
                            ).clicked() {
                                self.selected_device = Some(device.clone());
                                action = Some(DspAction::DeviceSelected(device));
                            }
                        }
                    });

                if ui.button("🔍 Discover").on_hover_text("Discover available devices").clicked() {
                    self.show_device_discovery = true;
                    self.discovering = true;
                    action = Some(DspAction::DiscoverDevices);
                }
            });

            // Show helpful message if device is selected but cache is empty (e.g., after restart)
            if matches!(self.selected_sink, SinkType::Dlna | SinkType::AirPlay) {
                let devices_list = match self.selected_sink {
                    SinkType::Dlna => &self.available_dlna_devices,
                    SinkType::AirPlay => &self.available_airplay_devices,
                    _ => &Vec::new(),
                };

                if self.selected_device.is_some() && devices_list.is_empty() {
                    ui.add_space(5.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("⚠")
                                    .color(egui::Color32::from_rgb(255, 165, 0))
                            );
                            ui.label(
                                egui::RichText::new("Device cache is empty")
                                    .color(egui::Color32::from_rgb(255, 165, 0))
                                    .strong()
                            );
                        });
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new("  Click '🔍 Discover' to find devices on your network before streaming")
                                .color(egui::Color32::LIGHT_GRAY)
                                .italics()
                                .size(10.0)
                        );
                    });
                }
            }

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
                        // Local DAC only supports F32 and S16LE
                        let available_formats = if self.selected_sink == SinkType::LocalDac {
                            vec![FormatOption::F32, FormatOption::S16LE]
                        } else {
                            vec![FormatOption::F32, FormatOption::S24LE, FormatOption::S16LE]
                        };

                        for &fmt in &available_formats {
                            if ui.selectable_label(
                                self.format == fmt,
                                fmt.as_str()
                            ).clicked() {
                                self.format = fmt;
                            }
                        }
                    });

                // Show hint for Local DAC
                if self.selected_sink == SinkType::LocalDac {
                    ui.label(
                        egui::RichText::new("ℹ")
                            .color(egui::Color32::LIGHT_BLUE)
                    ).on_hover_text("Local DAC only supports 32-bit Float (F32) and 16-bit PCM (S16LE)");
                }
            });

            // Buffer size
            ui.horizontal(|ui| {
                ui.label("Buffer:");
                ui.add(egui::Slider::new(&mut self.buffer_ms, 50..=500).suffix(" ms"));
            });

            // Headroom control
            ui.horizontal(|ui| {
                ui.label("Headroom:");
                let prev_headroom = self.headroom_db;
                ui.add(egui::Slider::new(&mut self.headroom_db, -6.0..=0.0)
                    .suffix(" dB")
                    .text(""))
                    .on_hover_text("Create headroom to prevent clipping from EQ/DSP processing.\nRecommended: -3 dB for most applications.");

                if self.headroom_db != prev_headroom {
                    action = Some(DspAction::HeadroomChanged);
                }
            });

            // Clip detection controls
            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.clip_detection, "Clip Detection")
                    .on_hover_text("Monitor and count audio samples that exceed ±1.0")
                    .changed()
                {
                    action = Some(DspAction::ClipDetectionChanged);
                }

                // Show clip counter if clips detected
                if self.clip_count > 0 {
                    ui.label(
                        egui::RichText::new(format!("! {} clips", self.clip_count))
                            .color(egui::Color32::from_rgb(255, 100, 100))
                            .strong()
                    ).on_hover_text("Audio clipping detected! Consider increasing headroom or reducing EQ gains.");

                    if ui.small_button("Reset").on_hover_text("Reset clip counter").clicked() {
                        self.clip_count = 0;
                        action = Some(DspAction::ResetClipCount);
                    }
                }
            });

            ui.add_space(10.0);
            ui.separator();

            // Dithering & Resampling sections - side by side
            ui.horizontal(|ui| {
                // Left column: Dithering & Noise Shaping
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Dithering & Noise Shaping").strong().size(14.0));

                    if ui.checkbox(&mut self.dither_enabled, "Enable Dithering")
                        .on_hover_text("Add dither noise to reduce quantization distortion when reducing bit depth")
                        .changed()
                    {
                        action = Some(DspAction::DitherToggled);
                    }

                    if self.dither_enabled {
                        ui.add_space(5.0);

                        // Dither mode selection
                        ui.horizontal(|ui| {
                            ui.label("Dither Mode:");
                            let prev_mode = self.dither_mode;

                            let display_name = match self.dither_mode {
                                DitherMode::None => "None",
                                DitherMode::Rectangular => "Rectangular",
                                DitherMode::Triangular => "Triangular (TPDF)",
                                DitherMode::Gaussian => "Gaussian",
                            };

                            egui::ComboBox::from_id_salt("dither_mode")
                                .selected_text(display_name)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.dither_mode, DitherMode::None, "None");
                                    ui.selectable_value(&mut self.dither_mode, DitherMode::Rectangular, "Rectangular");
                                    ui.selectable_value(&mut self.dither_mode, DitherMode::Triangular, "Triangular (TPDF)");
                                    ui.selectable_value(&mut self.dither_mode, DitherMode::Gaussian, "Gaussian");
                                });

                            if self.dither_mode != prev_mode {
                                action = Some(DspAction::DitherModeChanged);
                            }
                        });
                        ui.label(
                            egui::RichText::new("TPDF (Triangular) is the industry standard")
                                .size(9.0)
                                .color(egui::Color32::GRAY)
                                .italics()
                        );

                        ui.add_space(5.0);

                        // Noise shaping selection
                        ui.horizontal(|ui| {
                            ui.label("Noise Shaping:");
                            let prev_shaping = self.noise_shaping;

                            let display_name = match self.noise_shaping {
                                NoiseShaping::None => "None",
                                NoiseShaping::FirstOrder => "First Order",
                                NoiseShaping::SecondOrder => "Second Order",
                                NoiseShaping::Gesemann => "Gesemann",
                            };

                            egui::ComboBox::from_id_salt("noise_shaping")
                                .selected_text(display_name)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.noise_shaping, NoiseShaping::None, "None");
                                    ui.selectable_value(&mut self.noise_shaping, NoiseShaping::FirstOrder, "First Order");
                                    ui.selectable_value(&mut self.noise_shaping, NoiseShaping::SecondOrder, "Second Order");
                                    ui.selectable_value(&mut self.noise_shaping, NoiseShaping::Gesemann, "Gesemann");
                                });

                            if self.noise_shaping != prev_shaping {
                                action = Some(DspAction::NoiseShapingChanged);
                            }
                        });
                        ui.label(
                            egui::RichText::new("Shapes noise spectrum to audible range")
                                .size(9.0)
                                .color(egui::Color32::GRAY)
                                .italics()
                        );

                        ui.add_space(5.0);

                        // Target bit depth selection
                        ui.horizontal(|ui| {
                            ui.label("Target Bit Depth:");
                            let prev_bits = self.target_bits;
                            egui::ComboBox::from_id_salt("target_bits")
                                .selected_text(format!("{} bits", self.target_bits))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.target_bits, 16, "16 bits (CD quality)");
                                    ui.selectable_value(&mut self.target_bits, 24, "24 bits (HD audio)");
                                    ui.selectable_value(&mut self.target_bits, 32, "32 bits (studio)");
                                });

                            if self.target_bits != prev_bits {
                                action = Some(DspAction::TargetBitsChanged);
                            }
                        });
                        ui.label(
                            egui::RichText::new("Use 16-bit for CD, 24-bit for most DACs")
                                .size(9.0)
                                .color(egui::Color32::GRAY)
                                .italics()
                        );
                    }
                });

                // Vertical separator
                ui.separator();

                // Right column: High-Quality Resampling
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("High-Quality Resampling").strong().size(14.0));

                    if ui.checkbox(&mut self.resample_enabled, "Enable Resampling")
                        .on_hover_text("Convert sample rate using high-quality sinc interpolation")
                        .changed()
                    {
                        action = Some(DspAction::ResampleToggled);
                    }

                    if self.resample_enabled {
                        ui.add_space(5.0);

                        // Quality preset selection
                        ui.horizontal(|ui| {
                            ui.label("Quality:");
                            let prev_quality = self.resample_quality;

                            let display_name = match self.resample_quality {
                                ResamplerQuality::Fast => "Fast",
                                ResamplerQuality::Balanced => "Balanced",
                                ResamplerQuality::High => "High",
                                ResamplerQuality::Ultra => "Ultra",
                            };

                            egui::ComboBox::from_id_salt("resample_quality")
                                .selected_text(display_name)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.resample_quality, ResamplerQuality::Fast, "Fast");
                                    ui.selectable_value(&mut self.resample_quality, ResamplerQuality::Balanced, "Balanced");
                                    ui.selectable_value(&mut self.resample_quality, ResamplerQuality::High, "High");
                                    ui.selectable_value(&mut self.resample_quality, ResamplerQuality::Ultra, "Ultra");
                                });

                            if self.resample_quality != prev_quality {
                                action = Some(DspAction::ResampleQualityChanged);
                            }
                        });
                        ui.label(
                            egui::RichText::new("Balanced recommended for most use cases")
                                .size(9.0)
                                .color(egui::Color32::GRAY)
                                .italics()
                        );

                        ui.add_space(5.0);

                        // Target sample rate selection
                        ui.horizontal(|ui| {
                            ui.label("Target Sample Rate:");
                            let prev_rate = self.target_sample_rate;
                            egui::ComboBox::from_id_salt("target_sample_rate")
                                .selected_text(format!("{} Hz", self.target_sample_rate))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.target_sample_rate, 44100, "44100 Hz (CD standard)");
                                    ui.selectable_value(&mut self.target_sample_rate, 48000, "48000 Hz (studio standard)");
                                    ui.selectable_value(&mut self.target_sample_rate, 88200, "88200 Hz (2x CD)");
                                    ui.selectable_value(&mut self.target_sample_rate, 96000, "96000 Hz (HD audio)");
                                    ui.selectable_value(&mut self.target_sample_rate, 192000, "192000 Hz (ultra HD)");
                                });

                            if self.target_sample_rate != prev_rate {
                                action = Some(DspAction::TargetSampleRateChanged);
                            }
                        });
                        ui.label(
                            egui::RichText::new("Match your DAC's native sample rate")
                                .size(9.0)
                                .color(egui::Color32::GRAY)
                                .italics()
                        );
                    }
                });
            });

            ui.add_space(10.0);
            ui.separator();

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

            // Test controls
            if ui.button("🔊 Test Tone").on_hover_text("Play a 1kHz test tone for 2 seconds").clicked() {
                action = Some(DspAction::PlayTestTone);
            }
            } // End of !audio_output_collapsed conditional

            // Audio visualization and meters are always visible (outside collapsed section)
            ui.add_space(5.0);

            // Audio visualization toggle
            let viz_enabled = self.audio_viz.enabled || self.spectrum_analyzer.enabled;
            let mut temp_enabled = viz_enabled;
            ui.horizontal(|ui| {
                if ui.checkbox(&mut temp_enabled, "Show Visualization").on_hover_text("Display real-time audio visualization").changed() {
                    // Enable/disable current visualization mode
                    match self.viz_mode {
                        VisualizationMode::Waveform => self.audio_viz.enabled = temp_enabled,
                        VisualizationMode::Spectrum => self.spectrum_analyzer.enabled = temp_enabled,
                    }
                    action = Some(DspAction::ToggleVisualization);
                }

                // Mode selector (only show when visualization is enabled)
                if temp_enabled {
                    ui.label("Mode:");
                    let prev_mode = self.viz_mode;
                    ui.selectable_value(&mut self.viz_mode, VisualizationMode::Waveform, "Waveform");
                    ui.selectable_value(&mut self.viz_mode, VisualizationMode::Spectrum, "Spectrum");

                    // When mode changes, enable the selected mode and disable the other
                    self.audio_viz.enabled = self.viz_mode == VisualizationMode::Waveform;
                    self.spectrum_analyzer.enabled = self.viz_mode == VisualizationMode::Spectrum;

                    if prev_mode != self.viz_mode {
                        tracing::info!("Visualization mode changed to {:?}, spectrum enabled: {}, waveform enabled: {}",
                                     self.viz_mode, self.spectrum_analyzer.enabled, self.audio_viz.enabled);
                    }
                }
            });

            // Show audio visualization if enabled
            if self.audio_viz.enabled || self.spectrum_analyzer.enabled {
                ui.add_space(5.0);
                ui.separator();

                match self.viz_mode {
                    VisualizationMode::Waveform => self.audio_viz.show(ui),
                    VisualizationMode::Spectrum => self.spectrum_analyzer.show(ui, &spectrum_colors),
                }
            }

            // Audio level meters toggle
            ui.add_space(5.0);
            if ui.checkbox(&mut self.show_meters, "Show Audio Meters").on_hover_text("Display pre/post EQ audio level meters").changed() {
                action = Some(DspAction::ToggleMeters);
            }

            // Audio level meters
            if self.show_meters {
                ui.add_space(10.0);
                ui.separator();
                ui.label("Audio Levels:");

                // Update meter ballistics (only when streaming)
                if self.is_streaming {
                    self.pre_eq_meter.tick();
                    self.post_eq_meter.tick();
                }

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
                            crate::meter::draw_mc_style_meter(ui, meter_rect, &painter, &self.pre_eq_meter, &meter_colors);
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
                            crate::meter::draw_mc_style_meter(ui, meter_rect, &painter, &self.post_eq_meter, &meter_colors);
                        }
                    });
                });
            }
        });

        });  // End of ScrollArea

        // Device discovery dialog
        if self.show_device_discovery {
            // Filter devices based on selected sink type
            let devices_to_show = match self.selected_sink {
                SinkType::LocalDac => &self.available_local_devices,
                SinkType::Dlna => &self.available_dlna_devices,
                SinkType::AirPlay => &self.available_airplay_devices,
            };

            egui::Window::new("Discover Devices")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    if self.discovering && devices_to_show.is_empty() {
                        ui.label("Scanning for devices...");
                        ui.spinner();
                    } else if devices_to_show.is_empty() {
                        ui.label("No devices found");
                    } else {
                        ui.label(format!("Found {} device(s):", devices_to_show.len()));
                        ui.separator();

                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for device in devices_to_show.clone() {
                                if ui.button(&device).clicked() {
                                    self.selected_device = Some(device.clone());
                                    action = Some(DspAction::DeviceSelected(device));
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

    /// Buffer audio samples with current timestamp
    pub fn buffer_samples(&mut self, samples: Vec<f64>) {
        let now = std::time::Instant::now();
        self.viz_sample_buffer.push_back((now, samples));

        // Log occasionally for debugging (every 50 buffers)
        if self.viz_sample_buffer.len() % 50 == 0 {
            tracing::debug!("Sample buffer size: {} (delay: {} ms)", self.viz_sample_buffer.len(), self.viz_delay_ms);
        }

        // Limit buffer size to prevent memory issues (max 10 seconds worth of data)
        // Assuming ~2048 samples per buffer at 48kHz = ~42ms per buffer
        // 10 seconds = ~240 buffers (enough for 5 second delay + safety margin)
        const MAX_BUFFER_SIZE: usize = 240;
        while self.viz_sample_buffer.len() > MAX_BUFFER_SIZE {
            tracing::warn!("Sample buffer overflow, dropping oldest sample (delay: {} ms, buffer: {} items)", self.viz_delay_ms, self.viz_sample_buffer.len());
            self.viz_sample_buffer.pop_front();
        }
    }

    /// Buffer visualization metrics with current timestamp
    pub fn buffer_metrics(&mut self, metrics: VizMetrics) {
        let now = std::time::Instant::now();
        self.viz_metrics_buffer.push_back((now, metrics));

        // Log occasionally for debugging (every 100 buffers)
        if self.viz_metrics_buffer.len() % 100 == 0 {
            tracing::debug!("Metrics buffer size: {} (delay: {} ms)", self.viz_metrics_buffer.len(), self.viz_delay_ms);
        }

        // Limit buffer size (metrics come more frequently than samples)
        // Need to hold ~10 seconds worth for 5 second delays
        const MAX_BUFFER_SIZE: usize = 1000;
        while self.viz_metrics_buffer.len() > MAX_BUFFER_SIZE {
            tracing::warn!("Metrics buffer overflow, dropping oldest metrics (delay: {} ms, buffer: {} items)", self.viz_delay_ms, self.viz_metrics_buffer.len());
            self.viz_metrics_buffer.pop_front();
        }
    }

    /// Process buffered data and release items older than viz_delay_ms
    pub fn process_buffers(&mut self) {
        let now = std::time::Instant::now();
        let delay = std::time::Duration::from_millis(self.viz_delay_ms as u64);

        let mut samples_released = 0;
        let mut metrics_released = 0;

        // Process samples
        while let Some((timestamp, _)) = self.viz_sample_buffer.front() {
            let age_ms = now.duration_since(*timestamp).as_millis();
            if now.duration_since(*timestamp) >= delay {
                if let Some((_, samples)) = self.viz_sample_buffer.pop_front() {
                    self.audio_viz.push_samples(&samples);
                    self.spectrum_analyzer.process_samples(&samples);
                    samples_released += 1;
                }
            } else {
                // Log if we're waiting on data (every ~60 frames = ~1 second)
                static mut COUNTER: u32 = 0;
                unsafe {
                    COUNTER += 1;
                    if COUNTER % 60 == 0 {
                        tracing::debug!("Waiting for delay: oldest sample is {} ms old, need {} ms", age_ms, self.viz_delay_ms);
                    }
                }
                break;
            }
        }

        // Process metrics
        while let Some((timestamp, _)) = self.viz_metrics_buffer.front() {
            if now.duration_since(*timestamp) >= delay {
                if let Some((_, metrics)) = self.viz_metrics_buffer.pop_front() {
                    self.pre_eq_meter.update_from_block(
                        metrics.pre_eq_rms_l,
                        metrics.pre_eq_rms_r,
                        metrics.pre_eq_peak_l,
                        metrics.pre_eq_peak_r,
                    );
                    self.post_eq_meter.update_from_block(
                        metrics.post_eq_rms_l,
                        metrics.post_eq_rms_r,
                        metrics.post_eq_peak_l,
                        metrics.post_eq_peak_r,
                    );
                    metrics_released += 1;
                }
            } else {
                break;
            }
        }

        // Log when data is released (every ~60 frames)
        static mut RELEASE_COUNTER: u32 = 0;
        unsafe {
            RELEASE_COUNTER += 1;
            if RELEASE_COUNTER % 60 == 0 && (samples_released > 0 || metrics_released > 0) {
                tracing::debug!(
                    "Released {} samples, {} metrics (delay: {} ms, buffered: {} samples, {} metrics)",
                    samples_released,
                    metrics_released,
                    self.viz_delay_ms,
                    self.viz_sample_buffer.len(),
                    self.viz_metrics_buffer.len()
                );
            }
        }
    }

    /// Clear all buffered data
    pub fn clear_buffers(&mut self) {
        self.viz_sample_buffer.clear();
        self.viz_metrics_buffer.clear();
    }

    /// Auto-set delay from stream latency
    pub fn auto_set_delay_from_latency(&mut self, latency_ms: u32) {
        // Use the latency as a starting point, but cap it reasonably
        // Network devices like DLNA can have 2-5 seconds of buffering
        let old_delay = self.viz_delay_ms;
        self.viz_delay_ms = latency_ms.min(5000);
        tracing::info!(
            "Auto-set visualization delay: {} ms -> {} ms (from stream latency: {} ms)",
            old_delay,
            self.viz_delay_ms,
            latency_ms
        );
    }

    /// Attempt automatic delay detection for network streaming
    /// Returns true if delay was auto-set
    pub fn try_auto_detect_delay(&mut self, status: &StreamStatus) -> bool {
        // Log all status updates for debugging
        tracing::debug!(
            "Stream status: latency={} ms, is_streaming={}, auto_set={}, sink={:?}",
            status.latency_ms,
            self.is_streaming,
            self.viz_delay_auto_set,
            self.selected_sink
        );

        // Only auto-detect for network streaming (DLNA/AirPlay)
        if !matches!(self.selected_sink, SinkType::Dlna | SinkType::AirPlay) {
            tracing::debug!("Not auto-detecting: not a network sink");
            return false;
        }

        // Only auto-set once per streaming session
        if self.viz_delay_auto_set {
            tracing::debug!("Not auto-detecting: already auto-set for this session");
            return false;
        }

        // Only auto-set if we have valid latency and we're streaming
        if !self.is_streaming {
            tracing::debug!("Not auto-detecting: not streaming yet");
            return false;
        }

        if status.latency_ms == 0 {
            tracing::debug!("Not auto-detecting: latency is 0");
            return false;
        }

        // Auto-set the delay
        tracing::info!("🎯 Auto-detecting visualization delay from stream latency: {} ms", status.latency_ms);
        self.auto_set_delay_from_latency(status.latency_ms);
        self.viz_delay_auto_set = true;
        tracing::info!("✓ Auto-detection complete, delay set to {} ms", self.viz_delay_ms);
        true
    }

    /// Reset auto-detection flag (call when streaming stops or sink changes)
    pub fn reset_auto_delay(&mut self) {
        self.viz_delay_auto_set = false;
    }

    /// Update pipeline view with current state
    fn update_pipeline_view(&mut self) {
        let output_status = if self.is_streaming {
            match self.selected_sink {
                SinkType::LocalDac => {
                    self.selected_device.as_deref().unwrap_or("Local DAC")
                }
                SinkType::Dlna => {
                    self.selected_device.as_deref().unwrap_or("DLNA")
                }
                SinkType::AirPlay => {
                    self.selected_device.as_deref().unwrap_or("AirPlay")
                }
            }
        } else {
            "Stopped"
        };

        // Get display-friendly dither mode name for pipeline
        let dither_display = match self.dither_mode {
            DitherMode::None => "None",
            DitherMode::Rectangular => "Rectangular",
            DitherMode::Triangular => "TPDF", // Shorter for pipeline display
            DitherMode::Gaussian => "Gaussian",
        };

        // Convert resample quality to display string
        let resample_display = self.resample_quality.as_str();

        self.pipeline_view.update(
            self.is_streaming,
            self.sample_rate,
            self.headroom_db,
            self.clip_count,
            self.current_active_preset.as_deref(), // Use actual active preset from EQ Management
            self.resample_enabled,
            resample_display,
            self.target_sample_rate,
            self.dither_enabled,
            dither_display,
            output_status,
        );
    }

    /// Handle pipeline action (clicking on a stage)
    fn handle_pipeline_action(&self, action: crate::pipeline_view::PipelineAction) -> DspAction {
        match action {
            crate::pipeline_view::PipelineAction::FocusInput => {
                // For now, just return a no-op action
                // In the future, could scroll to input device selector
                DspAction::ToggleVisualization // Placeholder
            }
            crate::pipeline_view::PipelineAction::FocusHeadroom => {
                // Scroll to headroom controls (they're already visible in the UI)
                DspAction::ToggleVisualization // Placeholder
            }
            crate::pipeline_view::PipelineAction::FocusEq => {
                // Could open EQ preset selector or scroll to it
                DspAction::ToggleVisualization // Placeholder
            }
            crate::pipeline_view::PipelineAction::FocusResample => {
                // Scroll to resampling controls (they're visible in the UI)
                DspAction::ToggleVisualization // Placeholder
            }
            crate::pipeline_view::PipelineAction::FocusDither => {
                // Scroll to dithering controls (they're visible in the UI)
                DspAction::ToggleVisualization // Placeholder
            }
            crate::pipeline_view::PipelineAction::FocusOutput => {
                // Scroll to output device selector
                DspAction::ToggleVisualization // Placeholder
            }
        }
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
    ToggleMeters,
    SaveCustomPreset(EqPreset),
    HeadroomChanged,
    ClipDetectionChanged,
    ResetClipCount,
    DitherToggled,
    DitherModeChanged,
    NoiseShapingChanged,
    TargetBitsChanged,
    ResampleToggled,
    ResampleQualityChanged,
    TargetSampleRateChanged,
}
