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
}

impl Default for NowPlayingView {
    fn default() -> Self {
        Self {
            track: None,
            current_preset: None,
        }
    }
}

impl NowPlayingView {
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
                    ui.label(&track.genre);
                });

                if let Some(preset) = &self.current_preset {
                    ui.horizontal(|ui| {
                        ui.label("Current Preset:");
                        ui.strong(preset);
                    });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.label("Save current preset for:");

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
            } else {
                ui.label("No track playing");
            }
        });

        action
    }
}

pub enum NowPlayingAction {
    SaveMapping(Scope),
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
