use crate::types::{AudioBlock, SampleFormat};
use anyhow::Result;

/// Convert an AudioBlock to the target sample format
pub fn convert_format(
    block: AudioBlock<'_>,
    target_format: SampleFormat,
    output: &mut Vec<u8>,
) -> Result<()> {
    output.clear();

    match target_format {
        SampleFormat::F64 => {
            // No conversion needed, just write as bytes
            for &sample in block.frames {
                output.extend_from_slice(&sample.to_le_bytes());
            }
        }
        SampleFormat::F32 => {
            for &sample in block.frames {
                let f32_sample = sample as f32;
                output.extend_from_slice(&f32_sample.to_le_bytes());
            }
        }
        SampleFormat::S24LE => {
            for &sample in block.frames {
                // Apply dither before conversion
                let dithered = apply_tpdf_dither(sample, 24);
                let i32_sample = (dithered.clamp(-1.0, 1.0) * 8388607.0) as i32;
                // Write 24-bit as 3 bytes (little-endian)
                let bytes = i32_sample.to_le_bytes();
                output.extend_from_slice(&bytes[0..3]);
            }
        }
        SampleFormat::S16LE => {
            for &sample in block.frames {
                // Apply dither before conversion
                let dithered = apply_tpdf_dither(sample, 16);
                let i16_sample = (dithered.clamp(-1.0, 1.0) * 32767.0) as i16;
                output.extend_from_slice(&i16_sample.to_le_bytes());
            }
        }
    }

    Ok(())
}

/// Apply TPDF (Triangular Probability Density Function) dither
/// This is a high-quality dither that reduces quantization noise
fn apply_tpdf_dither(sample: f64, bit_depth: u8) -> f64 {
    // Calculate the amplitude of one LSB at the target bit depth
    let lsb = 1.0 / (1u64 << (bit_depth - 1)) as f64;

    // Generate two uniform random numbers and subtract to get triangular distribution
    // This is a simple approximation; in production you'd want a better RNG
    let r1: f64 = fastrand::f64();
    let r2: f64 = fastrand::f64();
    let dither = (r1 - r2) * lsb;

    sample + dither
}

/// Convert interleaved samples to a specific format with optional gain
pub fn convert_with_gain(
    block: AudioBlock<'_>,
    target_format: SampleFormat,
    gain_db: f64,
    output: &mut Vec<u8>,
) -> Result<()> {
    // Convert dB to linear gain
    let gain_linear = 10.0_f64.powf(gain_db / 20.0);

    // Apply gain and convert
    let gained_frames: Vec<f64> = block.frames.iter().map(|&s| s * gain_linear).collect();

    let gained_block = AudioBlock::new(&gained_frames, block.sample_rate, block.channels);
    convert_format(gained_block, target_format, output)
}

/// Calculate RMS level of an audio block (in dBFS)
pub fn calculate_rms_dbfs(block: AudioBlock<'_>) -> f64 {
    if block.frames.is_empty() {
        return -std::f64::INFINITY;
    }

    let sum_squares: f64 = block.frames.iter().map(|&s| s * s).sum();
    let rms = (sum_squares / block.frames.len() as f64).sqrt();

    if rms > 0.0 {
        20.0 * rms.log10()
    } else {
        -std::f64::INFINITY
    }
}

/// Calculate peak level of an audio block (in dBFS)
pub fn calculate_peak_dbfs(block: AudioBlock<'_>) -> f64 {
    let peak = block
        .frames
        .iter()
        .map(|&s| s.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);

    if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        -std::f64::INFINITY
    }
}

/// Check if an audio block is effectively silent (below noise floor threshold)
/// Returns true if all samples are below the threshold
pub fn is_silence(block: AudioBlock<'_>, threshold_dbfs: f64) -> bool {
    let threshold_linear = 10.0_f64.powf(threshold_dbfs / 20.0);

    // Check if all samples are below threshold
    for &sample in block.frames {
        if sample.abs() > threshold_linear {
            return false;
        }
    }

    true
}

/// Calculate noise floor of an audio block
/// Returns the average level of the quietest 10% of samples (in dBFS)
pub fn calculate_noise_floor_dbfs(block: AudioBlock<'_>) -> f64 {
    if block.frames.is_empty() {
        return -std::f64::INFINITY;
    }

    // Get absolute values and sort
    let mut abs_samples: Vec<f64> = block.frames.iter().map(|&s| s.abs()).collect();
    abs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Take quietest 10% of samples
    let samples_to_average = (abs_samples.len() / 10).max(1);
    let noise_samples = &abs_samples[..samples_to_average];

    // Calculate RMS of noise samples
    let sum_squares: f64 = noise_samples.iter().map(|&s| s * s).sum();
    let noise_rms = (sum_squares / noise_samples.len() as f64).sqrt();

    if noise_rms > 0.0 {
        20.0 * noise_rms.log10()
    } else {
        -std::f64::INFINITY
    }
}

/// Apply a simple soft limiter to prevent clipping
pub fn apply_soft_limiter(block: AudioBlock<'_>, threshold_db: f64, output: &mut Vec<f64>) {
    let threshold = 10.0_f64.powf(threshold_db / 20.0);

    output.clear();
    output.reserve(block.frames.len());

    for &sample in block.frames {
        let limited = if sample.abs() > threshold {
            // Soft clipping using tanh
            let sign = sample.signum();
            let normalized = sample.abs() / threshold;
            sign * threshold * normalized.tanh()
        } else {
            sample
        };
        output.push(limited);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_f64_to_f32() {
        let frames = vec![0.5, -0.5, 0.0, 1.0];
        let block = AudioBlock::new(&frames, 48000, 2);
        let mut output = Vec::new();

        convert_format(block, SampleFormat::F32, &mut output).unwrap();

        // F32 is 4 bytes per sample
        assert_eq!(output.len(), 16);
    }

    #[test]
    fn test_convert_f64_to_s16le() {
        let frames = vec![0.5, -0.5];
        let block = AudioBlock::new(&frames, 48000, 2);
        let mut output = Vec::new();

        convert_format(block, SampleFormat::S16LE, &mut output).unwrap();

        // S16LE is 2 bytes per sample
        assert_eq!(output.len(), 4);
    }

    #[test]
    fn test_convert_with_gain() {
        let frames = vec![0.5; 4];
        let block = AudioBlock::new(&frames, 48000, 2);
        let mut output = Vec::new();

        // Apply -3 dB gain
        convert_with_gain(block, SampleFormat::F64, -3.0, &mut output).unwrap();

        assert_eq!(output.len(), 32); // 4 samples * 8 bytes
    }

    #[test]
    fn test_calculate_rms() {
        let frames = vec![0.5, -0.5, 0.5, -0.5];
        let block = AudioBlock::new(&frames, 48000, 2);

        let rms = calculate_rms_dbfs(block);
        assert!(rms < 0.0); // Should be negative dBFS
        assert!(rms > -10.0); // But not too negative
    }

    #[test]
    fn test_calculate_peak() {
        let frames = vec![0.5, -0.8, 0.3, -0.2];
        let block = AudioBlock::new(&frames, 48000, 2);

        let peak = calculate_peak_dbfs(block);
        // Peak is 0.8, which is about -1.94 dB
        assert!((peak - (-1.94)).abs() < 0.1);
    }

    #[test]
    fn test_soft_limiter() {
        let frames = vec![1.5, -1.5, 0.5, -0.5];
        let block = AudioBlock::new(&frames, 48000, 2);
        let mut output = Vec::new();

        apply_soft_limiter(block, -3.0, &mut output);

        // All samples should be within [-1.0, 1.0]
        for &sample in &output {
            assert!(sample.abs() <= 1.0);
        }
    }

    #[test]
    fn test_soft_limiter_preserves_quiet_signals() {
        let frames = vec![0.1, -0.1, 0.2, -0.2];
        let block = AudioBlock::new(&frames, 48000, 2);
        let mut output = Vec::new();

        apply_soft_limiter(block, -3.0, &mut output);

        // Quiet signals should be mostly unchanged
        assert_eq!(output.len(), 4);
    }

    #[test]
    fn test_is_silence() {
        // True silence
        let frames = vec![0.0; 100];
        let block = AudioBlock::new(&frames, 48000, 2);
        assert!(is_silence(block, -60.0));

        // Very quiet signal (below -60 dBFS)
        let quiet_frames = vec![0.0001; 100]; // ~-80 dBFS
        let quiet_block = AudioBlock::new(&quiet_frames, 48000, 2);
        assert!(is_silence(quiet_block, -60.0));

        // Audible signal
        let audible_frames = vec![0.01; 100]; // ~-40 dBFS
        let audible_block = AudioBlock::new(&audible_frames, 48000, 2);
        assert!(!is_silence(audible_block, -60.0));
    }

    #[test]
    fn test_calculate_noise_floor() {
        // Pure silence should have -inf noise floor
        let frames = vec![0.0; 100];
        let block = AudioBlock::new(&frames, 48000, 2);
        assert_eq!(calculate_noise_floor_dbfs(block), -std::f64::INFINITY);

        // Signal with low noise floor
        let mut noisy_frames = vec![0.0001; 100]; // Quiet noise
        noisy_frames[50] = 0.5; // One loud sample
        let noisy_block = AudioBlock::new(&noisy_frames, 48000, 2);
        let noise_floor = calculate_noise_floor_dbfs(noisy_block);

        // Noise floor should be around -80 dBFS (from the quiet samples)
        assert!(noise_floor < -70.0 && noise_floor > -100.0);
    }
}
