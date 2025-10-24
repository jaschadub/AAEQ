/// Bezier curve editor widget for graphical EQ editing
///
/// Provides an interactive canvas with draggable control points on a logarithmic
/// frequency axis and linear gain axis. Shows both target curve and realized response.
use crate::eq_fitting::{
    calculate_realized_response, compute_fit_error, freq_to_norm, norm_to_freq, sample_bezier_curve,
    MAX_GAIN_DB, MIN_GAIN_DB,
};
use aaeq_core::{EqBand, EqPreset};
use egui::{
    epaint::{CubicBezierShape, PathShape},
    Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2,
};

/// Bezier EQ editor widget state
#[derive(Clone)]
pub struct BezierEqEditor {
    /// Control points in normalized space (x: 0-1 log freq, y: -12 to +12 dB)
    pub control_points: Vec<Pos2>,
    /// Last fitted bands (for realized response calculation)
    pub last_fitted_bands: Vec<EqBand>,
    /// Last fit error in dB RMS
    pub last_fit_error: f32,
    /// Sample rate for response calculation
    pub sample_rate: u32,
}

impl Default for BezierEqEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl BezierEqEditor {
    /// Create a new Bezier EQ editor with flat response
    pub fn new() -> Self {
        Self {
            control_points: vec![
                Pos2::new(0.0, 0.0),
                Pos2::new(0.33, 0.0),
                Pos2::new(0.67, 0.0),
                Pos2::new(1.0, 0.0),
            ],
            last_fitted_bands: vec![],
            last_fit_error: 0.0,
            sample_rate: 48000,
        }
    }

    /// Set control points from curve data
    pub fn set_control_points(&mut self, points: &[(f32, f32)]) {
        self.control_points = points
            .iter()
            .map(|(norm_freq, gain_db)| Pos2::new(*norm_freq, *gain_db))
            .collect();
    }

    /// Get control points as curve data
    pub fn get_control_points(&self) -> Vec<(f32, f32)> {
        self.control_points
            .iter()
            .map(|p| (p.x, p.y))
            .collect()
    }

    /// Convert normalized position to screen coordinates
    fn to_screen(&self, norm_pos: Pos2, rect: Rect) -> Pos2 {
        // X: 0-1 normalized log frequency
        // Y: -12 to +12 dB, inverted (top = +12, bottom = -12)
        let x = rect.left() + norm_pos.x * rect.width();
        let y_norm = (norm_pos.y - MIN_GAIN_DB) / (MAX_GAIN_DB - MIN_GAIN_DB);
        let y = rect.bottom() - y_norm * rect.height();

        Pos2::new(x, y)
    }

    /// Convert screen coordinates to normalized position
    fn screen_to_norm(&self, screen_pos: Pos2, rect: Rect) -> Pos2 {
        let norm_x = ((screen_pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        let y_norm = (rect.bottom() - screen_pos.y) / rect.height();
        let norm_y = (MIN_GAIN_DB + y_norm * (MAX_GAIN_DB - MIN_GAIN_DB))
            .clamp(MIN_GAIN_DB, MAX_GAIN_DB);

        Pos2::new(norm_x, norm_y)
    }

    /// Constrain control point to valid range and prevent X crossover
    fn constrain_point(&self, mut pos: Pos2, index: usize, _rect: Rect) -> Pos2 {
        // Clamp Y to ±12 dB
        pos.y = pos.y.clamp(MIN_GAIN_DB, MAX_GAIN_DB);

        // Prevent X from crossing adjacent points (maintain order)
        if index > 0 {
            let prev_x = self.control_points[index - 1].x;
            pos.x = pos.x.max(prev_x + 0.02); // Minimum 2% separation
        }
        if index < self.control_points.len() - 1 {
            let next_x = self.control_points[index + 1].x;
            pos.x = pos.x.min(next_x - 0.02);
        }

        // Clamp to valid X range
        pos.x = pos.x.clamp(0.0, 1.0);

        pos
    }

    /// Render the editor widget
    ///
    /// Returns true if control points changed
    pub fn show(&mut self, ui: &mut Ui, _preset: &EqPreset) -> bool {
        let (response, painter) = ui.allocate_painter(
            Vec2::new(ui.available_width(), 300.0),
            Sense::hover(),
        );

        let rect = response.rect;
        let control_point_radius = 8.0;
        let mut points_changed = false;

        // Draw background grid
        self.draw_grid(&painter, rect);

        // Draw frequency labels
        self.draw_freq_labels(ui, rect);

        // Draw gain labels
        self.draw_gain_labels(ui, rect);

        // Draw realized response (green) if we have fitted bands
        if !self.last_fitted_bands.is_empty() {
            self.draw_realized_response(&painter, rect);
        }

        // Draw target Bezier curve (orange)
        self.draw_target_curve(&painter, rect);

        // Draw and handle control points
        for (i, point) in self.control_points.clone().iter().enumerate() {
            let point_screen = self.to_screen(*point, rect);
            let point_rect = Rect::from_center_size(
                point_screen,
                Vec2::splat(2.0 * control_point_radius),
            );
            let point_id = response.id.with(i);
            let point_response = ui.interact(point_rect, point_id, Sense::drag());

            if point_response.dragged() {
                let drag_delta = point_response.drag_delta();
                let new_screen = point_screen + drag_delta;
                let new_norm = self.screen_to_norm(new_screen, rect);
                let constrained = self.constrain_point(new_norm, i, rect);

                self.control_points[i] = constrained;
                points_changed = true;
            }

            // Draw control point circle
            let stroke = ui.style().interact(&point_response).fg_stroke;
            painter.circle_stroke(point_screen, control_point_radius, stroke);
        }

        // Draw auxiliary lines connecting control points
        if self.control_points.len() >= 2 {
            let aux_points: Vec<Pos2> = self
                .control_points
                .iter()
                .map(|p| self.to_screen(*p, rect))
                .collect();
            painter.add(PathShape::line(
                aux_points,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 100, 100, 60)),
            ));
        }

        points_changed
    }

    /// Draw background grid
    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let grid_color = Color32::from_rgba_unmultiplied(100, 100, 100, 40);

        // Vertical lines at standard frequencies
        let grid_freqs = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
        for freq in grid_freqs {
            let norm_x = freq_to_norm(freq);
            let x = rect.left() + norm_x * rect.width();
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, grid_color),
            );
        }

        // Horizontal lines at gain values
        for gain_db in [-12.0, -9.0, -6.0, -3.0, 0.0, 3.0, 6.0, 9.0, 12.0] {
            let y_norm = (gain_db - MIN_GAIN_DB) / (MAX_GAIN_DB - MIN_GAIN_DB);
            let y = rect.bottom() - y_norm * rect.height();
            let color = if gain_db == 0.0 {
                Color32::from_rgba_unmultiplied(150, 150, 150, 80) // 0 dB line more prominent
            } else {
                grid_color
            };
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(1.0, color),
            );
        }
    }

    /// Draw frequency axis labels
    fn draw_freq_labels(&self, ui: &mut Ui, rect: Rect) {
        let label_freqs = [(31.0, "31"), (125.0, "125"), (500.0, "500"), (2000.0, "2k"), (8000.0, "8k"), (16000.0, "16k")];

        for (freq, label) in label_freqs {
            let norm_x = freq_to_norm(freq);
            let x = rect.left() + norm_x * rect.width();
            let label_pos = Pos2::new(x, rect.bottom() + 5.0);

            ui.painter().text(
                label_pos,
                egui::Align2::CENTER_TOP,
                label,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(150, 150, 150),
            );
        }
    }

    /// Draw gain axis labels
    fn draw_gain_labels(&self, ui: &mut Ui, rect: Rect) {
        for gain_db in [-12.0, -6.0, 0.0, 6.0, 12.0] {
            let y_norm = (gain_db - MIN_GAIN_DB) / (MAX_GAIN_DB - MIN_GAIN_DB);
            let y = rect.bottom() - y_norm * rect.height();
            let label_pos = Pos2::new(rect.left() - 5.0, y);

            let label = if gain_db == 0.0 {
                "0dB".to_string()
            } else {
                format!("{:+.0}", gain_db)
            };

            ui.painter().text(
                label_pos,
                egui::Align2::RIGHT_CENTER,
                label,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(150, 150, 150),
            );
        }
    }

    /// Draw target Bezier curve
    fn draw_target_curve(&self, painter: &egui::Painter, rect: Rect) {
        if self.control_points.len() < 4 {
            return;
        }

        let screen_points: Vec<Pos2> = self
            .control_points
            .iter()
            .take(4)
            .map(|p| self.to_screen(*p, rect))
            .collect();

        let shape = CubicBezierShape::from_points_stroke(
            screen_points.try_into().unwrap(),
            false,
            Color32::TRANSPARENT,
            Stroke::new(2.0, Color32::from_rgb(255, 180, 100)), // Orange for target
        );

        painter.add(shape);
    }

    /// Draw realized frequency response
    fn draw_realized_response(&self, painter: &egui::Painter, rect: Rect) {
        // Generate frequency points for evaluation (log-spaced)
        let n_points = 200;
        let mut freq_points = Vec::with_capacity(n_points);
        for i in 0..n_points {
            let norm = i as f32 / (n_points - 1) as f32;
            freq_points.push(norm_to_freq(norm));
        }

        // Calculate realized response
        let response = calculate_realized_response(
            &self.last_fitted_bands,
            &freq_points,
            self.sample_rate,
        );

        // Convert to screen space
        let screen_points: Vec<Pos2> = response
            .iter()
            .map(|(freq, gain_db)| {
                let norm_x = freq_to_norm(*freq);
                let norm_pos = Pos2::new(norm_x, *gain_db);
                self.to_screen(norm_pos, rect)
            })
            .collect();

        // Draw as line
        if screen_points.len() >= 2 {
            painter.add(PathShape::line(
                screen_points,
                Stroke::new(2.0, Color32::from_rgb(100, 255, 100)), // Green for realized
            ));
        }
    }

    /// Update fitted bands and compute error
    pub fn update_fit(&mut self, fitted_bands: Vec<EqBand>) {
        // Sample target curve
        let control_points_tuple: Vec<(f32, f32)> = self
            .control_points
            .iter()
            .map(|p| (p.x, p.y))
            .collect();

        let target_samples = sample_bezier_curve(&control_points_tuple, 200);

        // Calculate realized response at same frequencies
        let target_freqs: Vec<f32> = target_samples.iter().map(|(f, _)| *f).collect();
        let realized = calculate_realized_response(&fitted_bands, &target_freqs, self.sample_rate);

        // Compute error
        self.last_fit_error = compute_fit_error(&target_samples, &realized);
        self.last_fitted_bands = fitted_bands;
    }

    /// Get the last fit error
    pub fn get_fit_error(&self) -> f32 {
        self.last_fit_error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_conversions() {
        let editor = BezierEqEditor::new();
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 300.0));

        // Test corners
        let norm_pos = Pos2::new(0.0, 0.0); // Min freq, 0 dB
        let screen = editor.to_screen(norm_pos, rect);
        let back = editor.from_screen(screen, rect);

        assert!((back.x - norm_pos.x).abs() < 0.01);
        assert!((back.y - norm_pos.y).abs() < 0.1);
    }

    #[test]
    fn test_constrain_points() {
        let mut editor = BezierEqEditor::new();
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 300.0));

        // Try to move second point before first
        let bad_pos = Pos2::new(-0.1, 0.0);
        let constrained = editor.constrain_point(bad_pos, 1, rect);
        assert!(constrained.x >= editor.control_points[0].x + 0.02);

        // Try to set excessive gain
        let bad_gain = Pos2::new(0.5, 20.0);
        let constrained = editor.constrain_point(bad_gain, 1, rect);
        assert!(constrained.y <= MAX_GAIN_DB);
        assert!(constrained.y >= MIN_GAIN_DB);
    }
}
