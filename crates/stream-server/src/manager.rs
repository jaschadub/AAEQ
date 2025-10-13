use crate::sink::{OutputSink, SinkStats};
use crate::types::{AudioBlock, OutputConfig};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages multiple output sinks and routing
pub struct OutputManager {
    sinks: Vec<SinkEntry>,
    active_idx: Option<usize>,
}

struct SinkEntry {
    sink: Box<dyn OutputSink>,
    stats: SinkStats,
    config: Option<OutputConfig>,
}

impl OutputManager {
    /// Create a new OutputManager
    pub fn new() -> Self {
        Self {
            sinks: Vec::new(),
            active_idx: None,
        }
    }

    /// Register a new output sink
    pub fn register_sink(&mut self, sink: Box<dyn OutputSink>) {
        self.sinks.push(SinkEntry {
            sink,
            stats: SinkStats::default(),
            config: None,
        });
    }

    /// Get the number of registered sinks
    pub fn sink_count(&self) -> usize {
        self.sinks.len()
    }

    /// List all available sink names
    pub fn list_sinks(&self) -> Vec<&str> {
        self.sinks.iter().map(|entry| entry.sink.name()).collect()
    }

    /// Select a sink by index
    pub async fn select_sink(&mut self, idx: usize, config: OutputConfig) -> Result<()> {
        if idx >= self.sinks.len() {
            return Err(anyhow!("Sink index {} out of range", idx));
        }

        // Close the currently active sink if any
        if let Some(active_idx) = self.active_idx {
            if active_idx != idx {
                self.sinks[active_idx].sink.close().await?;
                self.sinks[active_idx].config = None;
            }
        }

        // Open the new sink
        self.sinks[idx].sink.open(config.clone()).await?;
        self.sinks[idx].config = Some(config);
        self.active_idx = Some(idx);

        Ok(())
    }

    /// Select a sink by name
    pub async fn select_sink_by_name(&mut self, name: &str, config: OutputConfig) -> Result<()> {
        let idx = self
            .sinks
            .iter()
            .position(|entry| entry.sink.name() == name)
            .ok_or_else(|| anyhow!("Sink '{}' not found", name))?;

        self.select_sink(idx, config).await
    }

    /// Write audio to the active sink
    pub async fn write(&mut self, block: AudioBlock<'_>) -> Result<()> {
        let active_idx = self
            .active_idx
            .ok_or_else(|| anyhow!("No active sink selected"))?;

        let entry = &mut self.sinks[active_idx];
        entry.sink.write(block).await?;
        entry.stats.frames_written += block.num_frames() as u64;

        Ok(())
    }

    /// Drain the active sink
    pub async fn drain(&mut self) -> Result<()> {
        let active_idx = self
            .active_idx
            .ok_or_else(|| anyhow!("No active sink selected"))?;

        self.sinks[active_idx].sink.drain().await
    }

    /// Close the active sink
    pub async fn close_active(&mut self) -> Result<()> {
        if let Some(idx) = self.active_idx {
            self.sinks[idx].sink.close().await?;
            self.sinks[idx].config = None;
            self.active_idx = None;
        }
        Ok(())
    }

    /// Get the active sink's name
    pub fn active_sink_name(&self) -> Option<&str> {
        self.active_idx
            .map(|idx| self.sinks[idx].sink.name())
    }

    /// Get the active sink's configuration
    pub fn active_sink_config(&self) -> Option<&OutputConfig> {
        self.active_idx
            .and_then(|idx| self.sinks[idx].config.as_ref())
    }

    /// Get statistics for the active sink
    pub fn active_sink_stats(&self) -> Option<&SinkStats> {
        self.active_idx.map(|idx| &self.sinks[idx].stats)
    }

    /// Get the active sink's latency
    pub fn active_sink_latency(&self) -> Option<u32> {
        self.active_idx
            .map(|idx| self.sinks[idx].sink.latency_ms())
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for OutputManager
pub type SharedOutputManager = Arc<RwLock<OutputManager>>;

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    // Mock sink for testing
    struct MockSink {
        name: &'static str,
        open: bool,
    }

    impl MockSink {
        fn new(name: &'static str) -> Self {
            Self { name, open: false }
        }
    }

    #[async_trait]
    impl OutputSink for MockSink {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn open(&mut self, _cfg: OutputConfig) -> Result<()> {
            self.open = true;
            Ok(())
        }

        async fn write(&mut self, _block: AudioBlock<'_>) -> Result<()> {
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
    async fn test_manager_register_sinks() {
        let mut manager = OutputManager::new();
        assert_eq!(manager.sink_count(), 0);

        manager.register_sink(Box::new(MockSink::new("sink1")));
        manager.register_sink(Box::new(MockSink::new("sink2")));

        assert_eq!(manager.sink_count(), 2);
        assert_eq!(manager.list_sinks(), vec!["sink1", "sink2"]);
    }

    #[tokio::test]
    async fn test_manager_select_sink() {
        let mut manager = OutputManager::new();
        manager.register_sink(Box::new(MockSink::new("test")));

        let config = OutputConfig::default();
        manager.select_sink(0, config).await.unwrap();

        assert_eq!(manager.active_sink_name(), Some("test"));
        assert!(manager.active_sink_config().is_some());
    }

    #[tokio::test]
    async fn test_manager_select_by_name() {
        let mut manager = OutputManager::new();
        manager.register_sink(Box::new(MockSink::new("sink1")));
        manager.register_sink(Box::new(MockSink::new("sink2")));

        let config = OutputConfig::default();
        manager.select_sink_by_name("sink2", config).await.unwrap();

        assert_eq!(manager.active_sink_name(), Some("sink2"));
    }

    #[tokio::test]
    async fn test_manager_write_without_active() {
        let mut manager = OutputManager::new();
        let frames = vec![0.0; 480];
        let block = AudioBlock::new(&frames, 48000, 2);

        let result = manager.write(block).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_manager_write_with_active() {
        let mut manager = OutputManager::new();
        manager.register_sink(Box::new(MockSink::new("test")));

        let config = OutputConfig::default();
        manager.select_sink(0, config).await.unwrap();

        let frames = vec![0.0; 480];
        let block = AudioBlock::new(&frames, 48000, 2);

        manager.write(block).await.unwrap();

        let stats = manager.active_sink_stats().unwrap();
        assert_eq!(stats.frames_written, 240);
    }

    #[tokio::test]
    async fn test_manager_close_active() {
        let mut manager = OutputManager::new();
        manager.register_sink(Box::new(MockSink::new("test")));

        let config = OutputConfig::default();
        manager.select_sink(0, config).await.unwrap();
        assert!(manager.active_sink_name().is_some());

        manager.close_active().await.unwrap();
        assert!(manager.active_sink_name().is_none());
    }
}
