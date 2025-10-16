/// Professional audio metering module with analog-style VU/dBFS displays
use egui::{Color32, Painter, Pos2, Rect, Rounding, Stroke, Ui};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeterScale {
    DbFs,   // -60..0 dBFS
    Vu,     // -20..+3 VU (0VU ~= -18 dBFS)
    Watts { load_ohms: f32, fs_vrms: f32 }, // power estimate
}

pub struct MeterState {
    // instantaneous inputs from DSP (per block)
    pub rms_dbfs_l: f32,
    pub rms_dbfs_r: f32,
    pub peak_dbfs_l: f32,
    pub peak_dbfs_r: f32,

    // internal smoothed needles
    needle_dbfs_l: f32,
    needle_dbfs_r: f32,
    peak_hold_dbfs_l: f32,
    peak_hold_dbfs_r: f32,

    last_update: Instant,
    // ballistics
    attack_ms: f32,  // how fast needle goes up (smaller = faster)
    release_ms: f32, // how fast needle falls
    peak_hold_ms: f32,
    scale: MeterScale,
}

impl Default for MeterState {
    fn default() -> Self {
        Self {
            rms_dbfs_l: -60.0,
            rms_dbfs_r: -60.0,
            peak_dbfs_l: -60.0,
            peak_dbfs_r: -60.0,
            needle_dbfs_l: -60.0,
            needle_dbfs_r: -60.0,
            peak_hold_dbfs_l: -60.0,
            peak_hold_dbfs_r: -60.0,
            last_update: Instant::now(),
            attack_ms: 10.0,
            release_ms: 300.0,   // VU-ish
            peak_hold_ms: 800.0,
            scale: MeterScale::DbFs,
        }
    }
}

impl MeterState {
    pub fn set_scale(&mut self, scale: MeterScale) {
        self.scale = scale;
    }

    pub fn set_ballistics(&mut self, attack_ms: f32, release_ms: f32, peak_hold_ms: f32) {
        self.attack_ms = attack_ms.max(1.0);
        self.release_ms = release_ms.max(10.0);
        self.peak_hold_ms = peak_hold_ms.max(0.0);
    }

    pub fn update_from_block(&mut self, rms_l: f32, rms_r: f32, peak_l: f32, peak_r: f32) {
        self.rms_dbfs_l = rms_l.clamp(-120.0, 0.0);
        self.rms_dbfs_r = rms_r.clamp(-120.0, 0.0);
        self.peak_dbfs_l = peak_l.clamp(-120.0, 0.0);
        self.peak_dbfs_r = peak_r.clamp(-120.0, 0.0);
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_update).as_secs_f32() * 1000.0;
        self.last_update = now;

        let step = |target: f32, current: f32, attack_ms: f32, release_ms: f32| -> f32 {
            if target > current {
                let k = (dt / attack_ms).min(1.0);
                current + (target - current) * k
            } else {
                let k = (dt / release_ms).min(1.0);
                current + (target - current) * k
            }
        };

        self.needle_dbfs_l = step(self.rms_dbfs_l, self.needle_dbfs_l, self.attack_ms, self.release_ms);
        self.needle_dbfs_r = step(self.rms_dbfs_r, self.needle_dbfs_r, self.attack_ms, self.release_ms);

        // peak hold decay
        let decay = dt >= self.peak_hold_ms;
        if self.peak_dbfs_l > self.peak_hold_dbfs_l {
            self.peak_hold_dbfs_l = self.peak_dbfs_l;
        } else if decay {
            self.peak_hold_dbfs_l -= 1.5 * (dt / 1000.0) * 10.0; // ~15 dB/s
        }

        if self.peak_dbfs_r > self.peak_hold_dbfs_r {
            self.peak_hold_dbfs_r = self.peak_dbfs_r;
        } else if decay {
            self.peak_hold_dbfs_r -= 1.5 * (dt / 1000.0) * 10.0;
        }
    }
}

/// Draw MC-style analog meter with arc display
pub fn draw_mc_style_meter(_ui: &mut Ui, rect: Rect, painter: &Painter, state: &MeterState) {
    // Enhanced palette with richer colors
    let background = Color32::from_rgb(15, 25, 35);
    let border = Color32::from_rgb(50, 120, 180);
    let needle = Color32::from_rgb(255, 220, 100);  // Brighter gold for needle
    let peak_hold = Color32::from_rgb(255, 100, 100);  // Red for peak hold
    let ticks = Color32::from_rgb(150, 180, 210);
    let label_color = Color32::from_rgb(200, 220, 240);

    // background with gradient effect
    painter.rect_filled(rect, Rounding::same(10.0), background);

    // Enhanced border with glow
    painter.rect_stroke(rect.shrink(1.0), Rounding::same(10.0), Stroke::new(2.5, border));
    painter.rect_stroke(rect.shrink(2.5), Rounding::same(9.0), Stroke::new(1.0, Color32::from_rgb(80, 150, 200)));

    // split L/R with more spacing
    let mid = rect.center().x;
    let left = Rect::from_min_max(rect.min, Pos2::new(mid - 6.0, rect.max.y));
    let right = Rect::from_min_max(Pos2::new(mid + 6.0, rect.min.y), rect.max);

    // Center divider line
    painter.line_segment(
        [Pos2::new(mid, rect.min.y + 5.0), Pos2::new(mid, rect.max.y - 5.0)],
        Stroke::new(1.5, Color32::from_rgb(40, 80, 120))
    );

    draw_one_meter(painter, left, "LEFT", state.needle_dbfs_l, state.peak_hold_dbfs_l, &state.scale, ticks, needle, label_color, peak_hold);
    draw_one_meter(painter, right, "RIGHT", state.needle_dbfs_r, state.peak_hold_dbfs_r, &state.scale, ticks, needle, label_color, peak_hold);
}

fn draw_one_meter(
    painter: &Painter,
    rect: Rect,
    label: &str,
    needle_dbfs: f32,
    peak_hold_dbfs: f32,
    scale: &MeterScale,
    tick_col: Color32,
    needle_col: Color32,
    label_col: Color32,
    peak_col: Color32,
) {
    // arc geometry - slightly larger arc
    let cx = rect.center().x;
    let cy = rect.max.y - 15.0;
    let radius = rect.width() * 0.95;
    let start = -145f32.to_radians();
    let end   = -35f32.to_radians();

    // ticks & labels
    let (marks, to_angle) = match scale {
        MeterScale::DbFs => {
            let marks = (-60..=0).step_by(6).map(|d| d as f32).collect::<Vec<_>>();
            let map = |db: f32| lerp(db, -60.0, 0.0, start, end);
            (marks, Box::new(map) as Box<dyn Fn(f32) -> f32>)
        }
        MeterScale::Vu => {
            // −20..+3 VU (0 VU at ~−18 dBFS)
            let marks = (-20..=3).step_by(2).map(|v| v as f32).collect::<Vec<_>>();
            let map = |vu: f32| lerp(vu, -20.0, 3.0, start, end);
            (marks, Box::new(map) as Box<dyn Fn(f32) -> f32>)
        }
        MeterScale::Watts { .. } => {
            // 0.001..100 W (log scale)
            let marks = vec![0.001, 0.01, 0.1, 1.0, 10.0, 100.0];
            let map = |w: f32| lerp(w.log10(), -3.0f32, 2.0, start, end);
            (marks, Box::new(map) as Box<dyn Fn(f32) -> f32>)
        }
    };

    // Draw background arc
    let arc_points = 50;
    for i in 0..arc_points {
        let t = i as f32 / arc_points as f32;
        let a1 = start + (end - start) * t;
        let a2 = start + (end - start) * ((i + 1) as f32 / arc_points as f32);
        let p1 = polar(cx, cy, radius * 0.82, a1);
        let p2 = polar(cx, cy, radius * 0.82, a2);
        painter.line_segment([p1, p2], Stroke::new(2.0, Color32::from_rgb(40, 60, 80)));
    }

    // Enhanced tick marks
    for m in &marks {
        let a = to_angle(*m);
        let p1 = polar(cx, cy, radius * 0.75, a);
        let p2 = polar(cx, cy, radius * 0.86, a);
        // Major ticks are thicker
        let is_major = (*m as i32) % 12 == 0 || *m == 0.0;
        let thickness = if is_major { 2.0 } else { 1.0 };
        painter.line_segment([p1, p2], Stroke::new(thickness, tick_col));
    }

    // label with enhanced styling
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 12.0),
        egui::Align2::CENTER_TOP,
        label,
        egui::FontId::proportional(13.0),
        label_col,
    );

    // convert needle value to angle
    let needle_angle = match scale {
        MeterScale::DbFs => to_angle(needle_dbfs.clamp(-60.0, 0.0)),
        MeterScale::Vu   => {
            // simple map: assume 0 VU ≈ -18 dBFS
            let vu = (needle_dbfs + 18.0).clamp(-20.0, 3.0);
            to_angle(vu)
        }
        MeterScale::Watts { load_ohms, fs_vrms } => {
            // estimate watts from dBFS
            let vrms = dbfs_to_vrms(needle_dbfs, *fs_vrms);
            let watts = (vrms * vrms) / (*load_ohms);
            to_angle(watts.max(0.0001))
        }
    };

    // peak hold pip
    let pip_angle = match scale {
        MeterScale::DbFs => to_angle(peak_hold_dbfs.clamp(-60.0, 0.0)),
        MeterScale::Vu   => {
            let vu = (peak_hold_dbfs + 18.0).clamp(-20.0, 3.0);
            to_angle(vu)
        }
        MeterScale::Watts { load_ohms, fs_vrms } => {
            let vrms = dbfs_to_vrms(peak_hold_dbfs, *fs_vrms);
            let w = (vrms * vrms) / (*load_ohms);
            to_angle(w.max(0.0001))
        }
    };
    let pip = polar(cx, cy, radius * 0.74, pip_angle);
    painter.circle_filled(pip, 3.0, peak_col);  // Larger, red peak hold indicator

    // needle with center hub
    let base = Pos2::new(cx, cy);
    let tip = polar(cx, cy, radius * 0.82, needle_angle);
    painter.line_segment([base, tip], Stroke::new(2.5, needle_col));
    // Center hub for needle pivot
    painter.circle_filled(base, 4.0, Color32::from_rgb(40, 60, 80));
    painter.circle_filled(base, 2.5, needle_col);
}

// helpers
fn lerp(x: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> f32 {
    let t = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
    y0 + t * (y1 - y0)
}

fn polar(cx: f32, cy: f32, r: f32, angle: f32) -> Pos2 {
    Pos2::new(cx + r * angle.cos(), cy + r * angle.sin())
}

fn dbfs_to_vrms(dbfs: f32, fs_vrms: f32) -> f32 {
    // -6 dBFS => 0.5 * FS amplitude (approx), Vrms scales by 10^(dB/20)
    fs_vrms * 10f32.powf(dbfs / 20.0)
}
