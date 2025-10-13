use crate::types::{AudioBlock, OutputConfig};
use anyhow::Result;
use async_trait::async_trait;

/// Trait for audio output sinks (local DAC, DLNA, AirPlay, etc.)
#[async_trait]
pub trait OutputSink: Send + Sync {
    /// Get the name of this sink implementation
    fn name(&self) -> &'static str;

    /// Open the output sink with the specified configuration
    async fn open(&mut self, cfg: OutputConfig) -> Result<()>;

    /// Write an audio block to the sink
    async fn write(&mut self, block: AudioBlock<'_>) -> Result<()>;

    /// Drain any buffered audio (wait for playback to complete)
    async fn drain(&mut self) -> Result<()>;

    /// Close the output sink
    async fn close(&mut self) -> Result<()>;

    /// Report end-to-end latency in milliseconds
    /// This includes buffering + device/protocol latency
    fn latency_ms(&self) -> u32;

    /// Check if the sink is currently open and ready
    fn is_open(&self) -> bool;
}

/// Statistics for monitoring output sink performance
#[derive(Clone, Debug, Default)]
pub struct SinkStats {
    /// Total frames written
    pub frames_written: u64,
    /// Number of buffer underruns
    pub underruns: u64,
    /// Number of buffer overruns
    pub overruns: u64,
    /// Current buffer fill level (0.0 to 1.0)
    pub buffer_fill: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    struct MockSink {
        open: bool,
        frames_received: usize,
    }

    impl MockSink {
        fn new() -> Self {
            Self {
                open: false,
                frames_received: 0,
            }
        }
    }

    #[async_trait]
    impl OutputSink for MockSink {
        fn name(&self) -> &'static str {
            "mock"
        }

        async fn open(&mut self, _cfg: OutputConfig) -> Result<()> {
            self.open = true;
            Ok(())
        }

        async fn write(&mut self, block: AudioBlock<'_>) -> Result<()> {
            if !self.open {
                anyhow::bail!("Sink not open");
            }
            self.frames_received += block.num_frames();
            Ok(())
        }

        async fn drain(&mut self) -> Result<()> {
            Ok(())
        }

        async fn close(&mut self) -> Result<()> {
            self.open = false;
            Ok(())
        }

        fn latency_ms(&self) -> u32 {
            50
        }

        fn is_open(&self) -> bool {
            self.open
        }
    }

    #[tokio::test]
    async fn test_mock_sink_lifecycle() {
        let mut sink = MockSink::new();
        assert!(!sink.is_open());

        sink.open(OutputConfig::default()).await.unwrap();
        assert!(sink.is_open());

        let frames = vec![0.0; 480];
        let block = AudioBlock::new(&frames, 48000, 2);
        sink.write(block).await.unwrap();
        assert_eq!(sink.frames_received, 240);

        sink.close().await.unwrap();
        assert!(!sink.is_open());
    }

    #[tokio::test]
    async fn test_mock_sink_write_without_open() {
        let mut sink = MockSink::new();
        let frames = vec![0.0; 480];
        let block = AudioBlock::new(&frames, 48000, 2);

        let result = sink.write(block).await;
        assert!(result.is_err());
    }
}
