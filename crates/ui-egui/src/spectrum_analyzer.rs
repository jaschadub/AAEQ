/// Real-time frequency spectrum analyzer visualization
///
/// Displays audio frequency content using FFT with:
/// - Log-scale frequency axis (20Hz - 20kHz)
/// - dB scale on Y-axis
/// - Peak-hold bars
/// - Theme-aware colors

use egui::{pos2, vec2, Color32, Rect, Ui};
use rustfft::{FftPlanner, num_complex::Complex};
use crate::theme::SpectrumColors;

/// Frequency bands - starting at 63Hz where typical music content begins (1/6-octave spacing)
const STANDARD_FREQS: &[f32] = &[
    63.0, 80.0, 90.0, 100.0, 112.0, 125.0, 140.0, 160.0, 180.0,
    200.0, 224.0, 250.0, 280.0, 315.0, 355.0, 400.0, 450.0, 500.0, 560.0,
    630.0, 710.0, 800.0, 900.0, 1000.0, 1120.0, 1250.0, 1400.0, 1600.0, 1800.0,
    2000.0, 2240.0, 2500.0, 2800.0, 3150.0, 3550.0, 4000.0, 4500.0, 5000.0, 5600.0,
    6300.0, 7100.0, 8000.0, 9000.0, 10000.0, 11200.0, 12500.0, 14000.0, 16000.0, 18000.0, 20000.0
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
    pub band_decay_db: f32,         // Band level decay rate when no signal (dB per frame)

    // Peak frequency indicator
    pub peak_band_idx: Option<usize>, // Index of band with highest energy
    pub show_peak_freq: bool,       // Toggle for peak frequency indicator

    // FFT state
    fft_buffer: Vec<Complex<f32>>,
    sample_accumulator: Vec<f64>,   // Ring buffer to accumulate samples
    accumulator_pos: usize,          // Current write position in ring buffer
    window: Vec<f32>,                // Hann window
    sample_rate: u32,
    last_sample_time: std::time::Instant, // Track when samples were last received
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
            band_decay_db: 1.5,  // Faster decay for band levels when no signal
            peak_band_idx: None,
            show_peak_freq: true, // Enabled by default
            fft_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            sample_accumulator: vec![0.0; fft_size],
            accumulator_pos: 0,
            window,
            sample_rate,
            last_sample_time: std::time::Instant::now(),
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

        // Update last sample time
        self.last_sample_time = std::time::Instant::now();

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

                // Apply window compensation (Hann window reduces amplitude by ~0.5)
                // and scale appropriately for display
                let normalized_mag = avg_mag * 2.0;

                // Convert to dBFS (reference: 1.0 = 0 dBFS)
                // Apply -3 dB offset to give headroom and prevent constant pegging
                let db = if normalized_mag > 1e-10 {
                    20.0 * normalized_mag.log10() - 3.0
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

        // Find the band with the highest current level (peak frequency indicator)
        if self.show_peak_freq {
            let mut max_db = self.db_floor;
            let mut max_idx = None;

            for (idx, &db) in self.bands_db.iter().enumerate() {
                if db > max_db && db > self.db_floor + 6.0 {  // Only show if significant (above -44 dB)
                    max_db = db;
                    max_idx = Some(idx);
                }
            }

            self.peak_band_idx = max_idx;
        } else {
            self.peak_band_idx = None;
        }
    }

    /// Update meter ballistics - decay bars when no signal
    pub fn tick(&mut self) {
        if !self.enabled {
            return;
        }

        // Check if we haven't received samples recently (more than 100ms ago)
        let time_since_samples = self.last_sample_time.elapsed();
        if time_since_samples.as_millis() > 100 {
            // Decay all bands towards floor
            for band_idx in 0..self.bands_db.len() {
                self.bands_db[band_idx] = (self.bands_db[band_idx] - self.band_decay_db).max(self.db_floor);

                // Also decay peak holds
                self.peak_db[band_idx] = (self.peak_db[band_idx] - self.peak_decay_db).max(self.db_floor);
            }
        }
    }

    /// Render the spectrum analyzer
    pub fn show(&mut self, ui: &mut Ui, colors: &SpectrumColors) {
        if !self.enabled {
            return;
        }

        // Update ballistics (decay when no signal)
        self.tick();

        ui.horizontal(|ui| {
            ui.label("Spectrum Analyzer:");
            ui.checkbox(&mut self.show_peak_freq, "Show Peak Frequency")
                .on_hover_text("Highlight the frequency band with the highest energy");
        });

        egui::containers::Frame::canvas(ui.style()).show(ui, |ui| {
            ui.ctx().request_repaint();

            let desired_size = ui.available_width() * vec2(1.0, 0.35);
            let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

            self.paint(ui, rect, colors.background, colors.grid, colors.bars, colors.peak_caps, colors.text);
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

        // Add horizontal padding for labels (30px on each side for dB scale numbers)
        let h_padding = 30.0;
        let w = rect.width() - (h_padding * 2.0);
        let h = rect.height();
        let left = rect.left() + h_padding;
        let right = rect.right() - h_padding;
        let bottom = rect.bottom();

        // Helper: map dB to y coordinate
        let y_for_db = |db: f32| {
            let t = (db - self.db_floor) / (self.db_ceil - self.db_floor);
            bottom - t * h
        };

        // Pre-calculate log-scale frequency range for bar positioning
        let log_min = self.freqs_hz.first().copied().unwrap_or(63.0).ln();
        let log_max = self.freqs_hz.last().copied().unwrap_or(20_000.0).ln();

        // Draw dB grid lines
        for db in ((self.db_floor as i32)..=(self.db_ceil as i32)).step_by(10) {
            let y = y_for_db(db as f32);
            painter.line_segment(
                [pos2(left, y), pos2(right, y)],
                egui::Stroke::new(0.5, grid),
            );

            // dB labels - centered on grid lines for better visibility
            let label = format!("{db}");
            painter.text(
                pos2(rect.left() + 6.0, y),
                egui::Align2::LEFT_CENTER,
                &label,
                egui::FontId::proportional(11.0),
                text_color,
            );
            painter.text(
                pos2(rect.right() - 6.0, y),
                egui::Align2::RIGHT_CENTER,
                &label,
                egui::FontId::proportional(11.0),
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

        // Draw bars using painter.rect_filled (more reliable than mesh)
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
                right  // Last bar extends to padded right edge
            };

            // Add tiny gap between bars for visual separation
            let gap = 0.5;
            let bar_x0 = x0 + gap;
            let bar_x1 = x1 - gap;

            // RMS bar - always draw from bottom to current level
            let bar_bottom = y_for_db(self.db_floor);
            let bar_top = y_for_db(db);

            // Draw filled bar rectangle
            let bar_rect = Rect::from_min_max(
                pos2(bar_x0, bar_top),
                pos2(bar_x1, bar_bottom),
            );
            painter.rect_filled(bar_rect, 0.0, fill);

            // Peak-hold cap
            let py = y_for_db(pdb);
            let cap_h = (h * 0.01).max(2.0);
            let cap_rect = Rect::from_min_max(
                pos2(bar_x0, py - cap_h),
                pos2(bar_x1, py),
            );
            painter.rect_filled(cap_rect, 0.0, peak_color);
        }

        // Draw peak frequency indicator
        if let Some(peak_idx) = self.peak_band_idx {
            if peak_idx < n && peak_idx < x_positions.len() {
                // Calculate bar edges for peak band
                let x0 = if peak_idx > 0 {
                    (x_positions[peak_idx - 1] + x_positions[peak_idx]) / 2.0
                } else {
                    left
                };

                let x1 = if peak_idx < n - 1 {
                    (x_positions[peak_idx] + x_positions[peak_idx + 1]) / 2.0
                } else {
                    right
                };

                let gap = 0.5;
                let bar_x0 = x0 + gap;
                let bar_x1 = x1 - gap;

                // Draw outline around peak bar
                let outline_color = Color32::from_rgb(255, 200, 0); // Golden/yellow indicator
                let stroke = egui::Stroke::new(2.0, outline_color);

                // Draw rectangle outline
                let bar_bottom = y_for_db(self.db_floor);
                let bar_top = y_for_db(self.bands_db[peak_idx]);
                let outline_rect = Rect::from_min_max(
                    pos2(bar_x0, bar_top),
                    pos2(bar_x1, bar_bottom),
                );
                painter.rect_stroke(outline_rect, 0.0, stroke);

                // Draw label with frequency value
                let peak_freq = self.freqs_hz[peak_idx];
                let label = if peak_freq >= 1000.0 {
                    format!("{:.1}kHz", peak_freq / 1000.0)
                } else {
                    format!("{:.0}Hz", peak_freq)
                };

                let label_pos = pos2((bar_x0 + bar_x1) / 2.0, rect.top() + 5.0);
                painter.text(
                    label_pos,
                    egui::Align2::CENTER_TOP,
                    label,
                    egui::FontId::proportional(12.0),
                    outline_color,
                );
            }
        }

        // Frequency grid lines and labels (starting at 63Hz)
        for &f in &[63.0_f32, 100.0, 160.0, 250.0, 400.0, 630.0,
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
                pos2(x, rect.bottom() - 4.0),
                egui::Align2::CENTER_BOTTOM,
                pretty_hz(f),
                egui::FontId::proportional(10.0),
                text_color,
            );
        }
    }
}

fn pretty_hz(f: f32) -> String {
    if f >= 1000.0 {
        format!("{:.0}k", f / 1000.0)
    } else {
        format!("{:.0}", f)
    }
}
