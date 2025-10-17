/// Album art lookup from external services
///
/// Fetches album artwork URLs using track metadata (artist, album, title)
/// from various free music databases.

use anyhow::Result;
use serde::Deserialize;
use tracing::{debug, warn};

/// iTunes Search API response structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ITunesSearchResponse {
    result_count: u32,
    results: Vec<ITunesResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ITunesResult {
    #[serde(default)]
    artwork_url_100: Option<String>,
    #[serde(default)]
    artwork_url_60: Option<String>,
}

/// Lookup album art URL using iTunes Search API
///
/// The iTunes Search API is free and doesn't require authentication.
/// Returns high-resolution album artwork (600x600) by modifying the URL.
pub async fn lookup_album_art(artist: &str, album: &str) -> Result<Option<String>> {
    // Skip lookup if metadata is missing or placeholder
    if artist.is_empty() || album.is_empty()
        || artist == "Unknown" || album == "Unknown"
        || artist == "Not playing" {
        debug!("Skipping album art lookup - invalid metadata");
        return Ok(None);
    }

    // Build search query
    let query = format!("{} {}", artist, album);
    let encoded_query = urlencoding::encode(&query);

    let url = format!(
        "https://itunes.apple.com/search?term={}&entity=album&limit=1",
        encoded_query
    );

    debug!("Looking up album art via iTunes API: artist='{}', album='{}'", artist, album);

    // Create client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    // Fetch from iTunes API
    match client.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                warn!("iTunes API returned status: {}", response.status());
                return Ok(None);
            }

            let search_result: ITunesSearchResponse = response.json().await?;

            if search_result.result_count == 0 {
                debug!("No album art found for: {} - {}", artist, album);
                return Ok(None);
            }

            // Get artwork URL and upgrade to high-res
            if let Some(result) = search_result.results.first() {
                let artwork_url = result.artwork_url_100.as_ref()
                    .or(result.artwork_url_60.as_ref());

                if let Some(url) = artwork_url {
                    // Upgrade to high-res by replacing 100x100 with 600x600
                    let high_res_url = url.replace("100x100", "600x600");
                    debug!("Found album art: {}", high_res_url);
                    return Ok(Some(high_res_url));
                }
            }

            Ok(None)
        }
        Err(e) => {
            warn!("Failed to fetch from iTunes API: {}", e);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lookup_pink_floyd() {
        let result = lookup_album_art("Pink Floyd", "The Dark Side of the Moon").await;
        assert!(result.is_ok());

        if let Ok(Some(url)) = result {
            println!("Found album art: {}", url);
            assert!(url.contains("http"));
        }
    }

    #[tokio::test]
    async fn test_lookup_unknown() {
        let result = lookup_album_art("Unknown", "Unknown").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }
}
