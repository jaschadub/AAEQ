use stream_server::*;

#[tokio::test]
async fn test_output_manager_with_all_sinks() {
    let mut manager = OutputManager::new();

    // Register all available sinks
    manager.register_sink(Box::new(LocalDacSink::new(None)));
    manager.register_sink(Box::new(DlnaSink::new(
        "Test Device".to_string(),
        "127.0.0.1:8090".parse().unwrap(),
    )));

    let airplay_sink = AirPlaySink::new();
    // Note: In a real scenario, you would use set_device() with a discovered AirPlayDevice
    manager.register_sink(Box::new(airplay_sink));

    assert_eq!(manager.sink_count(), 3);

    let sinks = manager.list_sinks();
    assert!(sinks.contains(&"local_dac"));
    assert!(sinks.contains(&"dlna"));
    assert!(sinks.contains(&"airplay"));
}

#[tokio::test]
async fn test_audio_pipeline_conversion() {
    // Create test audio data (1 second of 1kHz sine wave)
    let sample_rate = 48000;
    let duration = 1.0; // seconds
    let frequency = 1000.0; // Hz

    let num_samples = (sample_rate as f64 * duration) as usize * 2; // stereo
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples / 2 {
        let t = i as f64 / sample_rate as f64;
        let sample = (2.0 * std::f64::consts::PI * frequency * t).sin() * 0.5;
        samples.push(sample); // Left
        samples.push(sample); // Right
    }

    // Create audio block
    let block = AudioBlock::new(&samples, sample_rate, 2);
    assert!(block.is_valid());

    // Test format conversions
    let mut output = Vec::new();

    // Convert to F32
    convert_format(block, SampleFormat::F32, &mut output).unwrap();
    assert_eq!(output.len(), num_samples * 4); // 4 bytes per f32

    // Convert to S16LE
    output.clear();
    convert_format(block, SampleFormat::S16LE, &mut output).unwrap();
    assert_eq!(output.len(), num_samples * 2); // 2 bytes per i16

    // Convert to S24LE
    output.clear();
    convert_format(block, SampleFormat::S24LE, &mut output).unwrap();
    assert_eq!(output.len(), num_samples * 3); // 3 bytes per 24-bit sample
}

#[tokio::test]
async fn test_audio_levels() {
    // Create test audio with known levels
    let samples = vec![0.5, 0.5, -0.5, -0.5]; // -6 dBFS approximately
    let block = AudioBlock::new(&samples, 48000, 2);

    let rms = calculate_rms_dbfs(block);
    let peak = calculate_peak_dbfs(block);

    // Peak should be 0.5, which is -6.02 dB
    assert!((peak - (-6.02)).abs() < 0.1);

    // RMS should be close to peak for constant amplitude
    assert!((rms - peak).abs() < 0.1);
}

#[tokio::test]
async fn test_soft_limiter() {
    // Create audio that clips
    let samples = vec![1.5, -1.5, 0.5, -0.5];
    let block = AudioBlock::new(&samples, 48000, 2);
    let mut output = Vec::new();

    // Apply limiter with -3 dB threshold
    apply_soft_limiter(block, -3.0, &mut output);

    // All samples should be within [-1.0, 1.0]
    for &sample in &output {
        assert!((-1.0..=1.0).contains(&sample));
    }

    // Quiet samples should be mostly unchanged
    assert!((output[2] - 0.5).abs() < 0.1);
}

#[tokio::test]
async fn test_manager_sink_switching() {
    let mut manager = OutputManager::new();

    // Register two sinks
    manager.register_sink(Box::new(LocalDacSink::new(None)));

    let airplay = AirPlaySink::new();
    // Note: In a real scenario, you would use set_device() with a discovered AirPlayDevice
    manager.register_sink(Box::new(airplay));

    // Select first sink
    let config1 = OutputConfig {
        sample_rate: 48000,
        channels: 2,
        format: SampleFormat::F32,
        buffer_ms: 150,
        exclusive: false,
    };

    // Note: This will fail on systems without audio hardware, which is expected
    // In CI/CD, these tests should be skipped or mocked
    let result = manager.select_sink(0, config1).await;
    if result.is_ok() {
        assert_eq!(manager.active_sink_name(), Some("local_dac"));

        // Note: We don't try to switch to AirPlay since it requires a device to be set
        // In a real application, you would discover devices first
    }
}

#[test]
fn test_buffer_calculations() {
    let config = OutputConfig {
        sample_rate: 48000,
        channels: 2,
        format: SampleFormat::S24LE,
        buffer_ms: 100,
        exclusive: false,
    };

    // 100ms at 48kHz = 4800 frames
    assert_eq!(config.buffer_frames(), 4800);

    // 4800 frames * 2 channels * 3 bytes = 28800 bytes
    assert_eq!(config.buffer_bytes(), 28800);
}

#[test]
fn test_sample_format_properties() {
    assert_eq!(SampleFormat::F64.bytes_per_sample(), 8);
    assert_eq!(SampleFormat::F32.bytes_per_sample(), 4);
    assert_eq!(SampleFormat::S24LE.bytes_per_sample(), 3);
    assert_eq!(SampleFormat::S16LE.bytes_per_sample(), 2);

    assert!(SampleFormat::F64.is_float());
    assert!(SampleFormat::F32.is_float());
    assert!(!SampleFormat::S24LE.is_float());
    assert!(!SampleFormat::S16LE.is_float());
}

#[test]
fn test_audio_block_validation() {
    // Valid stereo block
    let samples = vec![0.0; 480]; // 240 frames
    let block = AudioBlock::new(&samples, 48000, 2);
    assert!(block.is_valid());
    assert_eq!(block.num_frames(), 240);

    // Invalid block (not divisible by channels)
    let samples_invalid = vec![0.0; 481];
    let block_invalid = AudioBlock::new(&samples_invalid, 48000, 2);
    assert!(!block_invalid.is_valid());

    // Mono block
    let samples_mono = vec![0.0; 240];
    let block_mono = AudioBlock::new(&samples_mono, 48000, 1);
    assert!(block_mono.is_valid());
    assert_eq!(block_mono.num_frames(), 240);
}
