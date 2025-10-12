use aaeq_core::{EqPreset, TrackMeta, Scope};
use crate::widgets::VerticalSlider;
use egui::{Context, ScrollArea, Ui};

/// View for creating/editing EQ presets with vertical sliders
pub struct EqEditorView {
    pub preset: EqPreset,
    pub preset_name: String,
}

impl Default for EqEditorView {
    fn default() -> Self {
        Self {
            preset: EqPreset::default(),
            preset_name: "Custom".to_string(),
        }
    }
}

impl EqEditorView {
    pub fn new(preset: EqPreset) -> Self {
        Self {
            preset_name: preset.name.clone(),
            preset,
        }
    }

    pub fn show(&mut self, ctx: &Context) -> Option<EqEditorAction> {
        let mut action = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("EQ Editor");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Preset Name:");
                ui.text_edit_singleline(&mut self.preset_name);
            });

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

                if ui.button("Save Preset").clicked() {
                    self.preset.name = self.preset_name.clone();
                    action = Some(EqEditorAction::Save(self.preset.clone()));
                }

                if ui.button("Apply to Device").clicked() {
                    self.preset.name = self.preset_name.clone();
                    action = Some(EqEditorAction::Apply(self.preset.clone()));
                }
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
    pub genre_edit: String,
}

impl Default for NowPlayingView {
    fn default() -> Self {
        Self {
            track: None,
            current_preset: None,
            genre_edit: String::new(),
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

    pub fn show(&mut self, ui: &mut Ui) -> Option<NowPlayingAction> {
        let mut action = None;

        ui.group(|ui| {
            ui.heading("Now Playing");

            if let Some(track) = &self.track {
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

                if let Some(preset) = &self.current_preset {
                    ui.horizontal(|ui| {
                        ui.label("Current Preset:");
                        ui.strong(preset);
                    });

                    // Visual EQ display
                    if let Some(eq_preset) = crate::preset_library::get_preset_curve(preset) {
                        ui.add_space(5.0);
                        ui.separator();

                        // Show label with indicator for estimated curves
                        let is_known = crate::preset_library::is_known_preset(preset);
                        if is_known {
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
                ui.label("No track playing");
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
    pub presets: Vec<String>,
    pub selected_preset: Option<String>,
}

impl Default for PresetsView {
    fn default() -> Self {
        Self {
            presets: vec![],
            selected_preset: None,
        }
    }
}

impl PresetsView {
    pub fn show(&mut self, ui: &mut Ui) -> Option<PresetAction> {
        let mut action = None;

        ui.group(|ui| {
            ui.heading("Presets");

            if ui.button("Refresh from Device").clicked() {
                action = Some(PresetAction::Refresh);
            }

            ui.add_space(5.0);

            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for preset in &self.presets {
                    let is_selected = self.selected_preset.as_deref() == Some(preset);
                    if ui.selectable_label(is_selected, preset).clicked() {
                        self.selected_preset = Some(preset.clone());
                        action = Some(PresetAction::Select(preset.clone()));
                    }
                }
            });

            ui.add_space(5.0);

            if let Some(selected) = &self.selected_preset {
                if ui.button("Apply Selected Preset").clicked() {
                    action = Some(PresetAction::Apply(selected.clone()));
                }
            }

            if ui.button("Create Custom EQ").clicked() {
                action = Some(PresetAction::CreateCustom);
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
