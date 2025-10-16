/// Album art loading and caching for the UI
use anyhow::Result;
use egui::ColorImage;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// State of an album art image
#[derive(Clone)]
pub enum AlbumArtState {
    /// Not yet loaded
    NotLoaded,
    /// Currently loading
    Loading,
    /// Successfully loaded
    Loaded(Arc<ColorImage>),
    /// Failed to load
    Failed,
}

/// Manager for loading and caching album art images
pub struct AlbumArtCache {
    /// Cache of loaded images (URL -> image)
    cache: Arc<RwLock<HashMap<String, AlbumArtState>>>,
    /// HTTP client for fetching images
    client: reqwest::Client,
}

impl Default for AlbumArtCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AlbumArtCache {
    /// Create a new album art cache
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();

        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            client,
        }
    }

    /// Get the current state of an album art image
    pub async fn get(&self, url: &str) -> AlbumArtState {
        let cache = self.cache.read().await;
        cache.get(url).cloned().unwrap_or(AlbumArtState::NotLoaded)
    }

    /// Request loading of an album art image (async)
    /// Returns immediately - use get() to check if loaded
    pub fn load(&self, url: String) {
        let cache = self.cache.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            // Check if already loading or loaded
            {
                let cache_read = cache.read().await;
                if let Some(state) = cache_read.get(&url) {
                    match state {
                        AlbumArtState::Loading | AlbumArtState::Loaded(_) => {
                            // Already loading or loaded, don't reload
                            return;
                        }
                        _ => {}
                    }
                }
            }

            // Mark as loading
            {
                let mut cache_write = cache.write().await;
                cache_write.insert(url.clone(), AlbumArtState::Loading);
            }

            debug!("Loading album art from: {}", url);

            // Fetch and decode image
            match Self::fetch_and_decode(&client, &url).await {
                Ok(color_image) => {
                    debug!("Successfully loaded album art from: {}", url);
                    let mut cache_write = cache.write().await;
                    cache_write.insert(url.clone(), AlbumArtState::Loaded(Arc::new(color_image)));
                }
                Err(e) => {
                    warn!("Failed to load album art from {}: {}", url, e);
                    let mut cache_write = cache.write().await;
                    cache_write.insert(url.clone(), AlbumArtState::Failed);
                }
            }
        });
    }

    /// Fetch and decode an image from a URL
    async fn fetch_and_decode(client: &reqwest::Client, url: &str) -> Result<ColorImage> {
        // Handle file:// URLs (MPRIS often uses these)
        if url.starts_with("file://") {
            let path = url.strip_prefix("file://").unwrap_or(url);
            let image_bytes = tokio::fs::read(path).await?;
            return Self::decode_image(&image_bytes);
        }

        // HTTP(S) URL
        let response = client.get(url).send().await?;
        let image_bytes = response.bytes().await?;
        Self::decode_image(&image_bytes)
    }

    /// Decode image bytes into a ColorImage
    fn decode_image(bytes: &[u8]) -> Result<ColorImage> {
        let image = image::load_from_memory(bytes)?;
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_raw();

        // Convert RGBA to egui ColorImage
        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        Ok(color_image)
    }

    /// Clear the cache
    #[allow(dead_code)]
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Remove a specific URL from the cache
    #[allow(dead_code)]
    pub async fn remove(&self, url: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(url);
    }
}
