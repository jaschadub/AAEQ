/// Audio waveform visualization widget
///
/// Inspired by egui's Dancing Strings demo, this widget displays
/// real-time audio waveforms with optional coloring.

use egui::{
    Color32, Pos2, Rect, Ui,
    containers::Frame,
    emath, epaint,
    epaint::PathStroke,
    lerp, pos2, remap, vec2,
};

#[derive(Clone)]
pub struct AudioVizState {
    pub enabled: bool,
    pub colored: bool,
    pub gradient_start: Color32, // Start color of gradient
    pub gradient_end: Color32,   // End color of gradient
    pub audio_buffer: Vec<f32>,  // Ring buffer for visualization
    pub buffer_pos: usize,
}

impl Default for AudioVizState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioVizState {
    pub fn new() -> Self {
        Self {
            enabled: false,
            colored: true,
            gradient_start: Color32::from_rgb(0, 201, 255),   // Cyan (#00C9FF)
            gradient_end: Color32::from_rgb(146, 254, 157),   // Green (#92FE9D)
            audio_buffer: vec![0.0; 1024],
            buffer_pos: 0,
        }
    }

    /// Update the audio buffer with new samples
    pub fn push_samples(&mut self, samples: &[f64]) {
        for &sample in samples {
            self.audio_buffer[self.buffer_pos] = sample as f32;
            self.buffer_pos = (self.buffer_pos + 1) % self.audio_buffer.len();
        }
    }

    /// Render the audio visualization
    pub fn show(&mut self, ui: &mut Ui) {
        if !self.enabled {
            return;
        }

        let color = if ui.visuals().dark_mode {
            Color32::from_additive_luminance(196)
        } else {
            Color32::from_black_alpha(240)
        };

        ui.horizontal(|ui| {
            ui.label("Audio Waveform:");
            ui.checkbox(&mut self.colored, "Color");
            if self.colored {
                ui.label("Start:");
                ui.color_edit_button_srgba(&mut self.gradient_start);
                ui.label("End:");
                ui.color_edit_button_srgba(&mut self.gradient_end);
                if ui.small_button("↻").on_hover_text("Reset to default colors").clicked() {
                    self.gradient_start = Color32::from_rgb(0, 201, 255);   // Cyan
                    self.gradient_end = Color32::from_rgb(146, 254, 157);   // Green
                }
            }
        });

        Frame::canvas(ui.style()).show(ui, |ui| {
            ui.ctx().request_repaint();

            let desired_size = ui.available_width() * vec2(1.0, 0.25);
            let (_id, rect) = ui.allocate_space(desired_size);

            let to_screen =
                emath::RectTransform::from_to(Rect::from_x_y_ranges(0.0..=1.0, -1.0..=1.0), rect);

            let mut shapes = vec![];

            // Draw waveform from ring buffer
            let n = self.audio_buffer.len().min(512); // Sample every other point for performance
            let step = self.audio_buffer.len() / n;

            let points: Vec<Pos2> = (0..n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    let idx = (self.buffer_pos + i * step) % self.audio_buffer.len();
                    let y = self.audio_buffer[idx].clamp(-1.0, 1.0);
                    to_screen * pos2(t, y)
                })
                .collect();

            let thickness = 2.0;
            shapes.push(epaint::Shape::line(
                points,
                if self.colored {
                    let color1 = self.gradient_start;
                    let color2 = self.gradient_end;
                    PathStroke::new_uv(thickness, move |rect, p| {
                        let t = remap(p.x, rect.x_range(), 0.0..=1.0);
                        Color32::from_rgb(
                            lerp(color1.r() as f32..=color2.r() as f32, t) as u8,
                            lerp(color1.g() as f32..=color2.g() as f32, t) as u8,
                            lerp(color1.b() as f32..=color2.b() as f32, t) as u8,
                        )
                    })
                } else {
                    PathStroke::new(thickness, color)
                },
            ));

            // Draw zero line
            let zero_line = vec![
                to_screen * pos2(0.0, 0.0),
                to_screen * pos2(1.0, 0.0),
            ];
            shapes.push(epaint::Shape::line(
                zero_line,
                PathStroke::new(0.5, Color32::from_gray(64)),
            ));

            ui.painter().extend(shapes);
        });
    }
}
