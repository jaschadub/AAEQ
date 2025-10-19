/// Real-time frequency spectrum analyzer visualization
///
/// Displays audio frequency content using FFT with:
/// - Log-scale frequency axis (20Hz - 20kHz)
/// - dB scale on Y-axis
/// - Peak-hold bars
/// - Theme-aware colors

use egui::{pos2, vec2, Color32, Rect, Ui, epaint::{Mesh, Vertex, Shape}};
use rustfft::{FftPlanner, num_complex::Complex};

/// Standard 1/3-octave center frequencies (Hz)
const STANDARD_FREQS: &[f32] = &[
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0,
    200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0,
    2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0
];

#[derive(Clone)]
pub struct SpectrumAnalyzerState {
    pub enabled: bool,
    pub bands_db: Vec<f32>,         // Current RMS level per band in dBFS
    pub peak_db: Vec<f32>,          // Peak-hold level per band in dBFS
    pub freqs_hz: Vec<f32>,         // Center frequency for each band
    pub db_floor: f32,              // Bottom of dB scale (e.g., -50)
    pub db_ceil: f32,               // Top of dB scale (e.g., +10)
    pub peak_decay_db: f32,         // Peak hold decay rate (dB per frame)

    // FFT state
    fft_buffer: Vec<Complex<f32>>,
    sample_accumulator: Vec<f64>,   // Ring buffer to accumulate samples
    accumulator_pos: usize,          // Current write position in ring buffer
    window: Vec<f32>,                // Hann window
    sample_rate: u32,
}

impl Default for SpectrumAnalyzerState {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrumAnalyzerState {
    pub fn new() -> Self {
        Self::with_fft_size(2048, 48000)
    }

    pub fn with_fft_size(fft_size: usize, sample_rate: u32) -> Self {
        let freqs_hz = STANDARD_FREQS.to_vec();
        let n_bands = freqs_hz.len();

        // Hann window for FFT
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                let x = std::f32::consts::PI * i as f32 / (fft_size - 1) as f32;
                0.5 * (1.0 - x.cos())
            })
            .collect();

        Self {
            enabled: false,
            bands_db: vec![-50.0; n_bands],
            peak_db: vec![-50.0; n_bands],
            freqs_hz,
            db_floor: -50.0,
            db_ceil: 10.0,
            peak_decay_db: 0.6,
            fft_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            sample_accumulator: vec![0.0; fft_size],
            accumulator_pos: 0,
            window,
            sample_rate,
        }
    }

    /// Process audio samples and update spectrum bands
    pub fn process_samples(&mut self, samples: &[f64]) {
        if !self.enabled {
            return;
        }

        if samples.is_empty() {
            return;
        }

        let fft_size = self.sample_accumulator.len();

        // Accumulate incoming samples into ring buffer
        for &sample in samples {
            self.sample_accumulator[self.accumulator_pos] = sample;
            self.accumulator_pos = (self.accumulator_pos + 1) % fft_size;
        }


        // Apply window and copy from ring buffer to FFT buffer
        // Read from accumulator starting at current position (oldest sample) to fill the FFT buffer
        for i in 0..fft_size {
            let buffer_idx = (self.accumulator_pos + i) % fft_size;
            let sample = self.sample_accumulator[buffer_idx];
            let windowed = sample as f32 * self.window[i];
            self.fft_buffer[i] = Complex::new(windowed, 0.0);
        }

        // Perform FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        fft.process(&mut self.fft_buffer);

        // Convert to magnitude in dBFS and bin to frequency bands
        let nyquist = self.sample_rate as f32 / 2.0;
        let freq_per_bin = nyquist / (fft_size / 2) as f32;

        for (band_idx, &center_freq) in self.freqs_hz.iter().enumerate() {
            // 1/3-octave bandwidth
            let bandwidth = center_freq * 0.23;  // ~23% for 1/3-octave
            let f_low = center_freq - bandwidth / 2.0;
            let f_high = center_freq + bandwidth / 2.0;

            // Find FFT bins in this band
            let bin_low = ((f_low / freq_per_bin) as usize).max(1);
            let bin_high = ((f_high / freq_per_bin) as usize).min(fft_size / 2);

            if bin_low >= bin_high {
                continue;
            }

            // Average magnitude in this band
            let mut sum_mag = 0.0;
            let mut count = 0;
            for bin in bin_low..bin_high {
                let mag = self.fft_buffer[bin].norm();
                sum_mag += mag;
                count += 1;
            }

            if count > 0 {
                let avg_mag = sum_mag / count as f32;
                // Convert to dBFS (reference: 1.0 = 0 dBFS)
                let db = if avg_mag > 1e-10 {
                    20.0 * avg_mag.log10()
                } else {
                    self.db_floor
                };

                // Smooth update (exponential moving average)
                let alpha = 0.3;  // Smoothing factor
                self.bands_db[band_idx] = alpha * db + (1.0 - alpha) * self.bands_db[band_idx];
                self.bands_db[band_idx] = self.bands_db[band_idx].clamp(self.db_floor, self.db_ceil);

                // Peak hold
                if self.bands_db[band_idx] > self.peak_db[band_idx] {
                    self.peak_db[band_idx] = self.bands_db[band_idx];
                } else {
                    self.peak_db[band_idx] = (self.peak_db[band_idx] - self.peak_decay_db).max(self.db_floor);
                }
            }
        }
    }

    /// Render the spectrum analyzer
    pub fn show(&mut self, ui: &mut Ui) {
        if !self.enabled {
            return;
        }

        ui.label("Spectrum Analyzer:");

        egui::containers::Frame::canvas(ui.style()).show(ui, |ui| {
            ui.ctx().request_repaint();

            let desired_size = ui.available_width() * vec2(1.0, 0.35);
            let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

            // Theme-aware colors
            let bg = if ui.visuals().dark_mode {
                Color32::from_rgb(15, 15, 20)
            } else {
                Color32::from_rgb(245, 245, 250)
            };

            let grid = if ui.visuals().dark_mode {
                Color32::from_gray(40)
            } else {
                Color32::from_gray(200)
            };

            let fill = if ui.visuals().dark_mode {
                Color32::from_rgb(0, 200, 100)  // Green
            } else {
                Color32::from_rgb(0, 180, 90)
            };

            let peak_color = if ui.visuals().dark_mode {
                Color32::from_rgb(255, 220, 0)  // Yellow
            } else {
                Color32::from_rgb(230, 180, 0)
            };

            let text_color = if ui.visuals().dark_mode {
                Color32::from_gray(180)
            } else {
                Color32::from_gray(80)
            };

            self.paint(ui, rect, bg, grid, fill, peak_color, text_color);
        });
    }

    fn paint(&self, ui: &mut Ui, rect: Rect, bg: Color32, grid: Color32,
             fill: Color32, peak_color: Color32, text_color: Color32) {
        let painter = ui.painter();

        // Background
        painter.rect_filled(rect, 0.0, bg);

        let n = self.bands_db.len();
        if n == 0 {
            return;
        }

        let w = rect.width();
        let h = rect.height();
        let left = rect.left();
        let bottom = rect.bottom();

        // Helper: map dB to y coordinate
        let y_for_db = |db: f32| {
            let t = (db - self.db_floor) / (self.db_ceil - self.db_floor);
            bottom - t * h
        };

        // Pre-calculate log-scale frequency range for bar positioning
        let log_min = self.freqs_hz.first().copied().unwrap_or(20.0).ln();
        let log_max = self.freqs_hz.last().copied().unwrap_or(20_000.0).ln();

        // Draw dB grid lines
        for db in ((self.db_floor as i32)..=(self.db_ceil as i32)).step_by(10) {
            let y = y_for_db(db as f32);
            painter.line_segment(
                [pos2(left, y), pos2(rect.right(), y)],
                egui::Stroke::new(0.5, grid),
            );

            // dB labels
            let label = format!("{db}");
            painter.text(
                pos2(rect.left() + 4.0, y - 8.0),
                egui::Align2::LEFT_TOP,
                &label,
                egui::FontId::proportional(10.0),
                text_color,
            );
            painter.text(
                pos2(rect.right() - 4.0, y - 8.0),
                egui::Align2::RIGHT_TOP,
                &label,
                egui::FontId::proportional(10.0),
                text_color,
            );
        }

        // Pre-calculate x positions for all bands
        let x_positions: Vec<f32> = self.freqs_hz.iter()
            .map(|&f| {
                let fx = f.ln();
                let xt = (fx - log_min) / (log_max - log_min);
                left + xt * w
            })
            .collect();

        // Debug: log first bar dimensions
        if n > 0 {
            tracing::debug!("Rect: left={}, right={}, bottom={}, top={}, width={}, height={}",
                          rect.left(), rect.right(), rect.bottom(), rect.top(), rect.width(), rect.height());
            tracing::debug!("First x_pos: {}, Second x_pos: {}", x_positions[0], x_positions.get(1).copied().unwrap_or(0.0));
        }

        // Bars as a single mesh (fast rendering)
        let mut mesh = Mesh::default();

        for (i, (&db, &pdb)) in self.bands_db.iter().zip(self.peak_db.iter()).enumerate() {
            // Calculate bar edges as midpoints between adjacent bands
            let x0 = if i > 0 {
                (x_positions[i - 1] + x_positions[i]) / 2.0
            } else {
                left  // First bar starts at left edge
            };

            let x1 = if i < n - 1 {
                (x_positions[i] + x_positions[i + 1]) / 2.0
            } else {
                rect.right()  // Last bar extends to right edge
            };

            // Add tiny gap between bars for visual separation (0.5px on each side)
            let gap = 0.5;
            let bar_x0 = x0 + gap;
            let bar_x1 = x1 - gap;

            // RMS bar - always draw from bottom to current level
            let bar_bottom = y_for_db(self.db_floor);  // This should be rect.bottom()
            let bar_top = y_for_db(db);

            // Draw bar (bars go upward from bottom)
            add_rect_to_mesh(&mut mesh, bar_x0, bar_top, bar_x1, bar_bottom, fill);

            // Debug first few bars
            if i < 3 {
                tracing::debug!("Bar {}: x0={:.1}, x1={:.1}, width={:.1}, y_top={:.1}, y_bottom={:.1}, db={:.1}",
                              i, bar_x0, bar_x1, bar_x1 - bar_x0, bar_top, bar_bottom, db);
            }

            // Peak-hold cap
            let py = y_for_db(pdb);
            let cap_h = (h * 0.01).max(2.0);
            add_rect_to_mesh(&mut mesh, bar_x0, py - cap_h, bar_x1, py, peak_color);
        }

        painter.add(Shape::mesh(mesh));

        // Frequency grid lines and labels
        for &f in &[20.0_f32, 31.5, 50.0, 63.0, 100.0, 160.0, 250.0, 400.0, 630.0,
                    1_000.0, 1_600.0, 2_500.0, 4_000.0, 6_300.0, 10_000.0, 16_000.0, 20_000.0] {
            let fx = f.ln();
            if !(log_min..=log_max).contains(&fx) {
                continue;
            }
            let xt = (fx - log_min) / (log_max - log_min);
            let x = left + xt * w;

            // Vertical grid line
            painter.line_segment(
                [pos2(x, rect.top()), pos2(x, rect.bottom())],
                egui::Stroke::new(0.3, grid.linear_multiply(0.5)),
            );

            // Frequency label
            painter.text(
                pos2(x, rect.bottom() - 2.0),
                egui::Align2::CENTER_BOTTOM,
                pretty_hz(f),
                egui::FontId::proportional(9.0),
                text_color,
            );
        }
    }
}

fn add_rect_to_mesh(mesh: &mut Mesh, x0: f32, y0: f32, x1: f32, y1: f32, col: Color32) {
    let idx = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&[
        Vertex { pos: pos2(x0, y0), uv: egui::pos2(0.0, 0.0), color: col },
        Vertex { pos: pos2(x1, y0), uv: egui::pos2(1.0, 0.0), color: col },
        Vertex { pos: pos2(x1, y1), uv: egui::pos2(1.0, 1.0), color: col },
        Vertex { pos: pos2(x0, y1), uv: egui::pos2(0.0, 1.0), color: col },
    ]);
    mesh.indices.extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
}

fn pretty_hz(f: f32) -> String {
    if f >= 1000.0 {
        format!("{:.0}k", f / 1000.0)
    } else {
        format!("{:.0}", f)
    }
}
