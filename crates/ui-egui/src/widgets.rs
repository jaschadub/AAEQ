use egui::{Response, Ui, Widget, Vec2, Rect, Pos2, Color32, Stroke, Sense};

/// Vertical slider widget for EQ band control
pub struct VerticalSlider<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: String,
}

impl<'a> VerticalSlider<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>, label: impl Into<String>) -> Self {
        Self {
            value,
            range,
            label: label.into(),
        }
    }
}

impl<'a> Widget for VerticalSlider<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = Vec2::new(50.0, 200.0);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);

            // Draw background track
            let track_rect = Rect::from_center_size(
                rect.center(),
                Vec2::new(8.0, rect.height() - 40.0),
            );
            ui.painter().rect_filled(
                track_rect,
                2.0,
                ui.visuals().extreme_bg_color,
            );

            // Draw center line (0 dB)
            let center_y = track_rect.center().y;
            ui.painter().line_segment(
                [
                    Pos2::new(track_rect.left() - 5.0, center_y),
                    Pos2::new(track_rect.right() + 5.0, center_y),
                ],
                Stroke::new(1.0, Color32::GRAY),
            );

            // Calculate thumb position based on value
            let normalized = (*self.value - *self.range.start()) / (*self.range.end() - *self.range.start());
            let thumb_y = track_rect.bottom() - normalized * track_rect.height();
            let thumb_center = Pos2::new(rect.center().x, thumb_y);

            // Handle drag
            if response.dragged() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let new_normalized = 1.0 - ((pointer_pos.y - track_rect.top()) / track_rect.height());
                    let new_normalized = new_normalized.clamp(0.0, 1.0);
                    *self.value = *self.range.start() + new_normalized * (*self.range.end() - *self.range.start());
                    response.mark_changed();
                }
            }

            // Draw thumb
            ui.painter().circle_filled(
                thumb_center,
                12.0,
                visuals.bg_fill,
            );
            ui.painter().circle_stroke(
                thumb_center,
                12.0,
                Stroke::new(2.0, visuals.fg_stroke.color),
            );

            // Draw label (frequency) at bottom
            let label_rect = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.bottom() - 10.0),
                Vec2::new(rect.width(), 20.0),
            );
            ui.painter().text(
                label_rect.center(),
                egui::Align2::CENTER_CENTER,
                &self.label,
                egui::FontId::proportional(10.0),
                ui.visuals().text_color(),
            );

            // Draw value at top
            let value_text = format!("{:+.1}", self.value);
            let value_rect = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + 10.0),
                Vec2::new(rect.width(), 20.0),
            );
            ui.painter().text(
                value_rect.center(),
                egui::Align2::CENTER_CENTER,
                &value_text,
                egui::FontId::proportional(10.0),
                ui.visuals().text_color(),
            );
        }

        response
    }
}
