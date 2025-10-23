/// Pipeline visualization module
///
/// Displays the DSP signal processing chain as a visual flow diagram
/// showing: Input → Headroom → EQ → Output with status indicators

use egui::{Color32, Ui, Vec2};

/// State of a pipeline stage
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StageState {
    Normal,   // Green - operating normally
    Warning,  // Yellow - potential issue (e.g., low headroom)
    Error,    // Red - problem detected (e.g., clipping)
    Bypassed, // Gray - stage is disabled/bypassed
}

/// A single stage in the DSP pipeline
#[derive(Clone, Debug)]
pub struct PipelineStage {
    pub name: &'static str,
    pub enabled: bool,
    pub status_text: String,
    pub latency_ms: f32,
    pub state: StageState,
}

impl PipelineStage {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            enabled: true,
            status_text: String::new(),
            latency_ms: 0.0,
            state: StageState::Normal,
        }
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status_text = status.into();
        self
    }

    pub fn with_latency(mut self, latency_ms: f32) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn with_state(mut self, state: StageState) -> Self {
        self.state = state;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        if !enabled {
            self.state = StageState::Bypassed;
        }
        self
    }
}

/// Pipeline visualization view
pub struct PipelineView {
    pub stages: Vec<PipelineStage>,
    pub total_latency_ms: f32,
    pub is_streaming: bool,
    pub sample_rate: u32,
}

impl Default for PipelineView {
    fn default() -> Self {
        Self {
            stages: vec![
                PipelineStage::new("INPUT").with_status("48kHz"),
                PipelineStage::new("HEADROOM").with_status("-3 dB"),
                PipelineStage::new("EQ").with_status("None"),
                PipelineStage::new("RESAMPLE").with_status("Off").with_enabled(false),
                PipelineStage::new("DITHER").with_status("Off").with_enabled(false),
                PipelineStage::new("OUTPUT").with_status("Stopped"),
            ],
            total_latency_ms: 0.0,
            is_streaming: false,
            sample_rate: 48000,
        }
    }
}

/// Actions that can be triggered by clicking pipeline stages
#[derive(Clone, Debug)]
pub enum PipelineAction {
    FocusInput,
    FocusHeadroom,
    FocusEq,
    FocusResample,
    FocusDither,
    FocusOutput,
}

impl PipelineView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the pipeline state based on current DSP configuration
    pub fn update(&mut self,
        is_streaming: bool,
        sample_rate: u32,
        headroom_db: f32,
        clip_count: u64,
        eq_preset: Option<&str>,
        resample_enabled: bool,
        resample_quality: &str,
        target_sample_rate: u32,
        dither_enabled: bool,
        dither_mode: &str,
        output_status: &str,
    ) {
        self.is_streaming = is_streaming;
        self.sample_rate = sample_rate;

        // Update INPUT stage
        self.stages[0] = PipelineStage::new("INPUT")
            .with_status(format!("{}kHz", sample_rate / 1000))
            .with_latency(0.0)
            .with_state(if is_streaming { StageState::Normal } else { StageState::Bypassed })
            .with_enabled(is_streaming);

        // Update HEADROOM stage
        let headroom_state = if clip_count > 0 {
            StageState::Error
        } else if headroom_db > -1.0 {
            StageState::Warning
        } else {
            StageState::Normal
        };

        self.stages[1] = PipelineStage::new("HEADROOM")
            .with_status(format!("{:.1} dB", headroom_db))
            .with_latency(0.1)
            .with_state(if is_streaming { headroom_state } else { StageState::Bypassed })
            .with_enabled(is_streaming);

        // Update EQ stage
        let eq_status = eq_preset.unwrap_or("None");
        self.stages[2] = PipelineStage::new("EQ")
            .with_status(eq_status.to_string())
            .with_latency(2.3)
            .with_state(if is_streaming && eq_preset.is_some() {
                StageState::Normal
            } else if is_streaming {
                StageState::Bypassed
            } else {
                StageState::Bypassed
            })
            .with_enabled(is_streaming);

        // Update RESAMPLE stage
        let resample_status = if resample_enabled {
            format!("{} → {}kHz", resample_quality, target_sample_rate / 1000)
        } else {
            "Off".to_string()
        };
        self.stages[3] = PipelineStage::new("RESAMPLE")
            .with_status(resample_status)
            .with_latency(1.5)
            .with_state(if is_streaming && resample_enabled {
                StageState::Normal
            } else {
                StageState::Bypassed
            })
            .with_enabled(is_streaming && resample_enabled);

        // Update DITHER stage
        self.stages[4] = PipelineStage::new("DITHER")
            .with_status(if dither_enabled { dither_mode.to_string() } else { "Off".to_string() })
            .with_latency(0.1)
            .with_state(if is_streaming && dither_enabled {
                StageState::Normal
            } else {
                StageState::Bypassed
            })
            .with_enabled(is_streaming && dither_enabled);

        // Update OUTPUT stage
        self.stages[5] = PipelineStage::new("OUTPUT")
            .with_status(output_status.to_string())
            .with_latency(5.2)
            .with_state(if is_streaming { StageState::Normal } else { StageState::Bypassed })
            .with_enabled(is_streaming);

        // Calculate total latency
        self.total_latency_ms = self.stages.iter().map(|s| s.latency_ms).sum();
    }

    /// Render the pipeline visualization
    pub fn show(&self, ui: &mut Ui, theme: &crate::theme::Theme) -> Option<PipelineAction> {
        let mut action = None;

        ui.group(|ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("DSP Pipeline").strong().size(14.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Draw status indicator
                    let indicator_size = Vec2::new(10.0, 10.0);
                    let (indicator_rect, _) = ui.allocate_exact_size(indicator_size, egui::Sense::hover());

                    if ui.is_rect_visible(indicator_rect) {
                        let painter = ui.painter();
                        let center = indicator_rect.center();
                        let color = if self.is_streaming {
                            Color32::from_rgb(50, 205, 50) // Green
                        } else {
                            Color32::from_rgb(100, 100, 100) // Gray
                        };
                        painter.circle_filled(center, 5.0, color);
                    }

                    ui.label(
                        egui::RichText::new(if self.is_streaming { "Active" } else { "Stopped" })
                            .size(11.0)
                    );
                });
            });

            ui.add_space(5.0);

            // Pipeline stages
            ui.horizontal(|ui| {
                for (i, stage) in self.stages.iter().enumerate() {
                    // Draw the stage box
                    if let Some(stage_action) = self.draw_stage(ui, stage, theme) {
                        action = Some(stage_action);
                    }

                    // Draw arrow between stages (except after last)
                    if i < self.stages.len() - 1 {
                        self.draw_arrow(ui);
                    }
                }
            });

            ui.add_space(5.0);

            // Footer with latency and sample rate
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Total Latency: {:.1} ms", self.total_latency_ms))
                        .size(11.0)
                        .color(Color32::GRAY)
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("Sample Rate: {} Hz", self.sample_rate))
                            .size(11.0)
                            .color(Color32::GRAY)
                    );
                });
            });
        });

        action
    }

    /// Draw a single pipeline stage
    fn draw_stage(&self, ui: &mut Ui, stage: &PipelineStage, theme: &crate::theme::Theme) -> Option<PipelineAction> {
        let mut action = None;

        // Determine colors based on state
        let accent_color = theme.meter_colors().border; // Use border color as accent
        let (bg_color, text_color, icon_color) = match stage.state {
            StageState::Normal => (
                accent_color,
                Color32::WHITE,
                Color32::from_rgb(144, 238, 144), // Light green
            ),
            StageState::Warning => (
                Color32::from_rgb(255, 165, 0), // Orange
                Color32::BLACK,
                Color32::from_rgb(255, 215, 0), // Gold
            ),
            StageState::Error => (
                Color32::from_rgb(220, 53, 69), // Red
                Color32::WHITE,
                Color32::from_rgb(255, 100, 100), // Light red
            ),
            StageState::Bypassed => (
                Color32::from_rgb(100, 100, 100), // Gray
                Color32::from_rgb(180, 180, 180),
                Color32::from_rgb(150, 150, 150),
            ),
        };

        // Create a frame for the stage
        let frame = egui::Frame::none()
            .fill(bg_color)
            .rounding(egui::Rounding::same(5.0))
            .inner_margin(egui::Margin::same(8.0));

        let response = frame.show(ui, |ui| {
            ui.set_min_size(Vec2::new(85.0, 60.0));
            ui.set_max_width(95.0);

            ui.vertical_centered(|ui| {
                // Stage name
                ui.label(
                    egui::RichText::new(stage.name)
                        .strong()
                        .size(11.0)
                        .color(text_color)
                );

                ui.add_space(2.0);

                // Status icon - draw using shapes for cross-platform compatibility
                let icon_size = Vec2::new(24.0, 24.0);
                let (icon_rect, _) = ui.allocate_exact_size(icon_size, egui::Sense::hover());

                if ui.is_rect_visible(icon_rect) {
                    let painter = ui.painter();
                    let center = icon_rect.center();
                    let radius = icon_rect.width() / 2.0;

                    if stage.enabled {
                        // Draw checkmark
                        let stroke = egui::Stroke::new(2.5, icon_color);

                        // Checkmark path: short vertical line, then longer diagonal
                        let start = center + Vec2::new(-radius * 0.4, 0.0);
                        let middle = center + Vec2::new(-radius * 0.1, radius * 0.4);
                        let end = center + Vec2::new(radius * 0.5, -radius * 0.5);

                        painter.line_segment([start, middle], stroke);
                        painter.line_segment([middle, end], stroke);
                    } else {
                        // Draw circle with diagonal line (like ⊘)
                        let stroke = egui::Stroke::new(2.0, icon_color);

                        // Draw circle
                        painter.circle_stroke(center, radius * 0.7, stroke);

                        // Draw diagonal line through it
                        let top_left = center + Vec2::new(-radius * 0.5, -radius * 0.5);
                        let bottom_right = center + Vec2::new(radius * 0.5, radius * 0.5);
                        painter.line_segment([top_left, bottom_right], stroke);
                    }
                }

                ui.add_space(2.0);

                // Status text
                ui.label(
                    egui::RichText::new(&stage.status_text)
                        .size(10.0)
                        .color(text_color)
                );
            });
        });

        // Make it clickable
        let click_response = response.response.interact(egui::Sense::click());

        if click_response.clicked() {
            action = Some(match stage.name {
                "INPUT" => PipelineAction::FocusInput,
                "HEADROOM" => PipelineAction::FocusHeadroom,
                "EQ" => PipelineAction::FocusEq,
                "RESAMPLE" => PipelineAction::FocusResample,
                "DITHER" => PipelineAction::FocusDither,
                "OUTPUT" => PipelineAction::FocusOutput,
                _ => return None,
            });
        }

        // Enhanced tooltip on hover with detailed explanations
        click_response.on_hover_ui(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(stage.name).strong().size(13.0));
                ui.separator();

                // Current status
                ui.label(format!("Status: {}", stage.status_text));
                ui.label(format!("Enabled: {}", if stage.enabled { "Yes" } else { "No" }));
                ui.label(format!("Latency: ~{:.1} ms", stage.latency_ms));

                ui.add_space(3.0);
                ui.separator();

                // Stage-specific explanations
                let description = match stage.name {
                    "INPUT" => "Captures audio from your selected input device. Sample rate determines the frequency range and processing precision.",
                    "HEADROOM" => "Reduces volume to prevent clipping. Digital audio clips at 0 dBFS causing distortion. Headroom provides safety margin for peaks.",
                    "EQ" => "Parametric equalizer adjusts frequency balance. Applies custom or mapped presets based on currently playing track.",
                    "RESAMPLE" => "Changes sample rate using high-quality sinc interpolation. Useful for matching DAC requirements or upsampling.",
                    "DITHER" => "Adds subtle noise when reducing bit depth. Eliminates quantization distortion, essential for 16-bit output.",
                    "OUTPUT" => "Streams processed audio to selected sink. Can be DLNA device, AirPlay speaker, or local DAC.",
                    _ => "DSP processing stage"
                };

                ui.label(
                    egui::RichText::new(description)
                        .size(10.0)
                        .color(Color32::LIGHT_GRAY)
                );

                // State-specific warnings/tips
                match stage.state {
                    StageState::Error if stage.name == "HEADROOM" => {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new("⚠ Clipping detected!")
                                .color(Color32::from_rgb(255, 100, 100))
                        );
                        ui.label(
                            egui::RichText::new("Increase headroom to -6 dB or reduce input volume")
                                .size(10.0)
                                .color(Color32::from_rgb(255, 150, 150))
                        );
                    }
                    StageState::Warning if stage.name == "HEADROOM" => {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new("⚠ Low headroom")
                                .color(Color32::from_rgb(255, 215, 0))
                        );
                        ui.label(
                            egui::RichText::new("Consider -3 dB or more for EQ adjustments")
                                .size(10.0)
                                .color(Color32::from_rgb(255, 215, 0))
                        );
                    }
                    StageState::Bypassed if stage.name == "EQ" => {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new("💡 Tip: Create mappings to automatically apply EQ per song")
                                .size(10.0)
                                .color(Color32::from_rgb(150, 200, 255))
                        );
                    }
                    StageState::Bypassed if stage.name == "RESAMPLE" => {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new("💡 Tip: Enable for DACs that prefer specific sample rates")
                                .size(10.0)
                                .color(Color32::from_rgb(150, 200, 255))
                        );
                    }
                    StageState::Bypassed if stage.name == "DITHER" => {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new("💡 Tip: Enable TPDF dither for 16-bit output to eliminate distortion")
                                .size(10.0)
                                .color(Color32::from_rgb(150, 200, 255))
                        );
                    }
                    _ => {}
                }

                ui.add_space(5.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("🖱 Click to jump to settings")
                        .italics()
                        .size(10.0)
                        .color(Color32::GRAY)
                );
            });
        });

        // Show latency below the box
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(format!("{:.1}ms", stage.latency_ms))
                    .size(9.0)
                    .color(Color32::GRAY)
            );
        });

        action
    }

    /// Draw an arrow between stages
    fn draw_arrow(&self, ui: &mut Ui) {
        ui.add_space(5.0);

        let arrow_color = if self.is_streaming {
            Color32::from_rgb(100, 200, 100)
        } else {
            Color32::from_rgb(120, 120, 120)
        };

        ui.vertical(|ui| {
            ui.add_space(20.0); // Align with middle of boxes

            // Draw arrow using shapes for cross-platform compatibility
            let arrow_size = Vec2::new(20.0, 16.0);
            let (arrow_rect, _) = ui.allocate_exact_size(arrow_size, egui::Sense::hover());

            if ui.is_rect_visible(arrow_rect) {
                let painter = ui.painter();
                let center = arrow_rect.center();

                if self.is_streaming {
                    // Draw solid arrow (→)
                    let stroke = egui::Stroke::new(2.0, arrow_color);

                    // Arrow shaft (horizontal line)
                    let left = center + Vec2::new(-8.0, 0.0);
                    let right = center + Vec2::new(6.0, 0.0);
                    painter.line_segment([left, right], stroke);

                    // Arrow head (two lines forming >)
                    let tip = center + Vec2::new(8.0, 0.0);
                    let top = center + Vec2::new(4.0, -4.0);
                    let bottom = center + Vec2::new(4.0, 4.0);
                    painter.line_segment([top, tip], stroke);
                    painter.line_segment([bottom, tip], stroke);
                } else {
                    // Draw dashed arrow (⋯>)
                    let stroke = egui::Stroke::new(2.0, arrow_color);

                    // Three dots
                    for i in 0..3 {
                        let x_offset = -6.0 + (i as f32 * 4.0);
                        let dot_center = center + Vec2::new(x_offset, 0.0);
                        painter.circle_filled(dot_center, 1.5, arrow_color);
                    }

                    // Arrow head (>)
                    let tip = center + Vec2::new(8.0, 0.0);
                    let top = center + Vec2::new(4.0, -4.0);
                    let bottom = center + Vec2::new(4.0, 4.0);
                    painter.line_segment([top, tip], stroke);
                    painter.line_segment([bottom, tip], stroke);
                }
            }
        });

        ui.add_space(5.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_stage_creation() {
        let stage = PipelineStage::new("TEST")
            .with_status("Active")
            .with_latency(1.5)
            .with_state(StageState::Normal);

        assert_eq!(stage.name, "TEST");
        assert_eq!(stage.status_text, "Active");
        assert_eq!(stage.latency_ms, 1.5);
        assert_eq!(stage.state, StageState::Normal);
        assert!(stage.enabled);
    }

    #[test]
    fn test_pipeline_view_default() {
        let view = PipelineView::default();
        assert_eq!(view.stages.len(), 6);
        assert!(!view.is_streaming);
        assert_eq!(view.sample_rate, 48000);
    }

    #[test]
    fn test_pipeline_update() {
        let mut view = PipelineView::new();

        view.update(
            true,           // is_streaming
            96000,          // sample_rate
            -6.0,           // headroom_db
            0,              // clip_count
            Some("Rock"),   // eq_preset
            true,           // resample_enabled
            "High",         // resample_quality
            48000,          // target_sample_rate
            true,           // dither_enabled
            "TPDF",         // dither_mode
            "DLNA Device"   // output_status
        );

        assert!(view.is_streaming);
        assert_eq!(view.sample_rate, 96000);
        assert_eq!(view.stages[0].status_text, "96kHz");
        assert_eq!(view.stages[1].status_text, "-6.0 dB");
        assert_eq!(view.stages[2].status_text, "Rock");
        assert_eq!(view.stages[3].status_text, "High → 48kHz");
        assert_eq!(view.stages[4].status_text, "TPDF");
        assert_eq!(view.stages[5].status_text, "DLNA Device");
    }

    #[test]
    fn test_clipping_detection() {
        let mut view = PipelineView::new();

        view.update(true, 48000, -3.0, 10, None, false, "Fast", 48000, false, "Off", "DAC");

        // Headroom stage should show error state due to clips
        assert_eq!(view.stages[1].state, StageState::Error);
    }

    #[test]
    fn test_low_headroom_warning() {
        let mut view = PipelineView::new();

        view.update(true, 48000, -0.5, 0, None, false, "Fast", 48000, false, "Off", "DAC");

        // Headroom stage should show warning state
        assert_eq!(view.stages[1].state, StageState::Warning);
    }

    #[test]
    fn test_total_latency_calculation() {
        let view = PipelineView::default();

        // Default stages have no latency set, so total should be 0.0
        assert!((view.total_latency_ms - 0.0).abs() < 0.01);
    }
}
