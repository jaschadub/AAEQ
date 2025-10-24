/// Bezier curve fitting and frequency response calculation for EQ editing
///
/// This module handles conversion between Bezier curve representations and
/// parametric EQ band gains, plus calculation of realized frequency responses.
use aaeq_core::{BezierCurveData, EqBand, EqPreset};
use std::f32::consts::PI;

/// Standard EQ band frequencies (Hz)
pub const BAND_FREQUENCIES: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

/// Frequency range for EQ editing
pub const MIN_FREQ_HZ: f32 = 20.0;
pub const MAX_FREQ_HZ: f32 = 20000.0;

/// Gain range for EQ editing
pub const MIN_GAIN_DB: f32 = -12.0;
pub const MAX_GAIN_DB: f32 = 12.0;

/// Convert normalized log frequency (0-1) to Hz
pub fn norm_to_freq(norm: f32) -> f32 {
    let log_min = MIN_FREQ_HZ.ln();
    let log_max = MAX_FREQ_HZ.ln();
    (log_min + norm * (log_max - log_min)).exp()
}

/// Convert frequency (Hz) to normalized log scale (0-1)
pub fn freq_to_norm(freq_hz: f32) -> f32 {
    let log_min = MIN_FREQ_HZ.ln();
    let log_max = MAX_FREQ_HZ.ln();
    let log_freq = freq_hz.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ).ln();
    ((log_freq - log_min) / (log_max - log_min)).clamp(0.0, 1.0)
}

/// Evaluate cubic Bezier curve at parameter t (0-1)
fn eval_cubic_bezier(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;

    let x = mt3 * p0.0 + 3.0 * mt2 * t * p1.0 + 3.0 * mt * t2 * p2.0 + t3 * p3.0;
    let y = mt3 * p0.1 + 3.0 * mt2 * t * p1.1 + 3.0 * mt * t2 * p2.1 + t3 * p3.1;

    (x, y)
}

/// Sample a Bezier curve defined by control points into frequency/gain pairs
///
/// # Arguments
/// * `control_points` - Control points in normalized space (x: 0-1 log freq, y: -12 to +12 dB)
/// * `n_samples` - Number of samples to generate (default: 2048)
///
/// # Returns
/// Vector of (frequency_hz, gain_db) tuples, sorted by frequency
pub fn sample_bezier_curve(control_points: &[(f32, f32)], n_samples: usize) -> Vec<(f32, f32)> {
    if control_points.len() < 4 {
        // Need at least 4 points for cubic Bezier
        return vec![];
    }

    let mut samples = Vec::with_capacity(n_samples);

    // Use first 4 points as cubic Bezier control points
    let p0 = control_points[0];
    let p1 = control_points[1];
    let p2 = control_points[2];
    let p3 = control_points[3];

    // Sample the curve parametrically
    for i in 0..n_samples {
        let t = i as f32 / (n_samples - 1) as f32;
        let (norm_freq, gain_db) = eval_cubic_bezier(p0, p1, p2, p3, t);

        // Convert normalized frequency to Hz
        let freq_hz = norm_to_freq(norm_freq.clamp(0.0, 1.0));
        let clamped_gain = gain_db.clamp(MIN_GAIN_DB, MAX_GAIN_DB);

        samples.push((freq_hz, clamped_gain));
    }

    // Sort by frequency (should already be sorted if x values are monotonic)
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    samples
}

/// Fit a sampled curve to the 10 fixed EQ bands using least-squares approximation
///
/// # Arguments
/// * `samples` - Vector of (frequency_hz, gain_db) tuples from curve sampling
///
/// # Returns
/// Vector of EqBand with fitted gain values
pub fn fit_to_bands(samples: &[(f32, f32)]) -> Vec<EqBand> {
    let mut bands = Vec::new();

    for &freq in &BAND_FREQUENCIES {
        let freq_f32 = freq as f32;

        // Find gain at this frequency by interpolating samples
        let gain = interpolate_gain(samples, freq_f32);

        bands.push(EqBand {
            frequency: freq,
            gain,
        });
    }

    bands
}

/// Interpolate gain at a specific frequency from sampled curve data
fn interpolate_gain(samples: &[(f32, f32)], target_freq: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    // Find surrounding samples
    let mut lower_idx = 0;
    let mut upper_idx = samples.len() - 1;

    for (i, &(freq, _)) in samples.iter().enumerate() {
        if freq <= target_freq {
            lower_idx = i;
        }
        if freq >= target_freq {
            upper_idx = i;
            break;
        }
    }

    if lower_idx == upper_idx {
        return samples[lower_idx].1;
    }

    // Linear interpolation
    let (f1, g1) = samples[lower_idx];
    let (f2, g2) = samples[upper_idx];

    if (f2 - f1).abs() < 1e-6 {
        return g1;
    }

    let t = (target_freq - f1) / (f2 - f1);
    g1 + t * (g2 - g1)
}

/// Generate Bezier control points from EQ band gains
///
/// This creates a smooth curve that passes through or near the band points.
/// Useful for converting existing band-based presets to curve representation.
///
/// # Arguments
/// * `preset` - EQ preset with band gains
///
/// # Returns
/// BezierCurveData with 4 control points
pub fn bands_to_curve(preset: &EqPreset) -> BezierCurveData {
    if preset.bands.is_empty() {
        // Return flat response as fallback
        return BezierCurveData {
            control_points: vec![
                (0.0, 0.0),
                (0.33, 0.0),
                (0.67, 0.0),
                (1.0, 0.0),
            ],
            fitted_at_sample_rate: 48000,
        };
    }

    // Create control points that approximate the band shape
    // Use individual band values at strategic frequencies for better approximation

    // Map bands by frequency for easy lookup
    let band_map: std::collections::HashMap<u32, f32> = preset.bands.iter()
        .map(|b| (b.frequency, b.gain))
        .collect();

    // P0: Low frequency point - use 62 Hz as representative of bass
    let p0_gain = band_map.get(&62).copied()
        .or_else(|| band_map.get(&31).copied())
        .unwrap_or(0.0);

    // P1: Low-mid point - use 250 Hz as crossover point
    let p1_gain = band_map.get(&250).copied()
        .or_else(|| band_map.get(&500).copied())
        .unwrap_or(0.0);

    // P2: High-mid point - use 2k Hz as presence region
    let p2_gain = band_map.get(&2000).copied()
        .or_else(|| band_map.get(&1000).copied())
        .unwrap_or(0.0);

    // P3: High frequency point - use 8k Hz as treble (NOT averaging with 16k)
    // This captures the treble region better than averaging extremes
    let p3_gain = band_map.get(&8000).copied()
        .or_else(|| band_map.get(&4000).copied())
        .unwrap_or(0.0);

    BezierCurveData {
        control_points: vec![
            (freq_to_norm(62.0), p0_gain),    // Bass
            (freq_to_norm(250.0), p1_gain),   // Low-mid
            (freq_to_norm(2000.0), p2_gain),  // High-mid
            (freq_to_norm(8000.0), p3_gain),  // Treble
        ],
        fitted_at_sample_rate: 48000,
    }
}

/// Calculate realized frequency response from biquad cascade
///
/// Computes the actual magnitude response of the parametric EQ at specified frequencies.
/// Uses the same biquad math as the DSP processor.
///
/// # Arguments
/// * `bands` - EQ bands with frequency and gain
/// * `freq_points` - Frequencies to evaluate (Hz)
/// * `sample_rate` - Audio sample rate (Hz)
///
/// # Returns
/// Vector of (frequency_hz, gain_db) tuples for the realized response
pub fn calculate_realized_response(
    bands: &[EqBand],
    freq_points: &[f32],
    sample_rate: u32,
) -> Vec<(f32, f32)> {
    let sr = sample_rate as f64;
    let mut response = Vec::with_capacity(freq_points.len());

    for &freq in freq_points {
        let mut total_mag_db = 0.0_f64;

        // Calculate magnitude response of each band at this frequency
        for band in bands {
            if band.gain.abs() < 1e-6 {
                // Skip bands with negligible gain
                continue;
            }

            let mag_db = biquad_peaking_response(
                band.frequency as f64,
                band.gain as f64,
                1.0, // Q factor (matches EqProcessor default)
                freq as f64,
                sr,
            );

            total_mag_db += mag_db;
        }

        response.push((freq, total_mag_db as f32));
    }

    response
}

/// Calculate magnitude response of a peaking biquad filter at a given frequency
///
/// # Arguments
/// * `center_freq` - Center frequency of the band (Hz)
/// * `gain_db` - Gain of the band (dB)
/// * `q` - Q factor
/// * `eval_freq` - Frequency to evaluate response at (Hz)
/// * `sample_rate` - Sample rate (Hz)
///
/// # Returns
/// Magnitude response in dB
fn biquad_peaking_response(
    center_freq: f64,
    gain_db: f64,
    q: f64,
    eval_freq: f64,
    sample_rate: f64,
) -> f64 {
    // Biquad peaking filter coefficients (matching EqProcessor)
    let a = 10_f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI as f64 * center_freq / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let a0 = 1.0 + alpha / a;
    let b0 = (1.0 + alpha * a) / a0;
    let b1 = (-2.0 * cos_w0) / a0;
    let b2 = (1.0 - alpha * a) / a0;
    let a1 = (-2.0 * cos_w0) / a0;
    let a2 = (1.0 - alpha / a) / a0;

    // Evaluate H(z) at z = e^(j*w) where w = 2*pi*f/fs
    let w = 2.0 * PI as f64 * eval_freq / sample_rate;
    let cos_w = w.cos();
    let sin_w = w.sin();
    let cos_2w = (2.0 * w).cos();
    let sin_2w = (2.0 * w).sin();

    // Numerator: b0 + b1*z^-1 + b2*z^-2
    let num_re = b0 + b1 * cos_w + b2 * cos_2w;
    let num_im = -b1 * sin_w - b2 * sin_2w;
    let num_mag_sq = num_re * num_re + num_im * num_im;

    // Denominator: 1 + a1*z^-1 + a2*z^-2
    let den_re = 1.0 + a1 * cos_w + a2 * cos_2w;
    let den_im = -a1 * sin_w - a2 * sin_2w;
    let den_mag_sq = den_re * den_re + den_im * den_im;

    // Magnitude in dB
    if den_mag_sq > 1e-12 {
        let mag_sq = num_mag_sq / den_mag_sq;
        10.0 * mag_sq.sqrt().log10()
    } else {
        0.0
    }
}

/// Compute RMS error between target curve and realized response
///
/// # Arguments
/// * `target` - Target frequency/gain pairs
/// * `realized` - Realized frequency/gain pairs (must match target frequencies)
///
/// # Returns
/// RMS error in dB
pub fn compute_fit_error(target: &[(f32, f32)], realized: &[(f32, f32)]) -> f32 {
    if target.is_empty() || target.len() != realized.len() {
        return 0.0;
    }

    let sum_sq_error: f32 = target
        .iter()
        .zip(realized.iter())
        .map(|((_, t_db), (_, r_db))| (t_db - r_db).powi(2))
        .sum();

    (sum_sq_error / target.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freq_conversions() {
        assert!((freq_to_norm(20.0) - 0.0).abs() < 0.01);
        assert!((freq_to_norm(20000.0) - 1.0).abs() < 0.01);
        assert!((freq_to_norm(1000.0) - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_sample_flat_curve() {
        let control_points = vec![
            (0.0, 0.0),
            (0.33, 0.0),
            (0.67, 0.0),
            (1.0, 0.0),
        ];
        let samples = sample_bezier_curve(&control_points, 100);

        assert_eq!(samples.len(), 100);
        // All gains should be near 0
        for (_, gain) in samples {
            assert!(gain.abs() < 0.1);
        }
    }

    #[test]
    fn test_fit_to_bands_flat() {
        let samples = vec![
            (20.0, 0.0),
            (1000.0, 0.0),
            (20000.0, 0.0),
        ];

        let bands = fit_to_bands(&samples);
        assert_eq!(bands.len(), 10);

        // All bands should have near-zero gain
        for band in bands {
            assert!(band.gain.abs() < 0.1);
        }
    }

    #[test]
    fn test_bands_to_curve() {
        let preset = EqPreset::default(); // Flat preset
        let curve_data = bands_to_curve(&preset);

        assert_eq!(curve_data.control_points.len(), 4);
        // All control points should have near-zero gain
        for (_, gain) in curve_data.control_points {
            assert!(gain.abs() < 0.1);
        }
    }
}
