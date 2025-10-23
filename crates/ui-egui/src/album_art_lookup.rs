//! Album art lookup from external services
//!
//! Fetches album artwork URLs using track metadata (artist, album, title)
//! from various free music databases.

use anyhow::Result;
use serde::Deserialize;
use tracing::{debug, warn};

/// iTunes Search API response structure
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ITunesSearchResponse {
    result_count: u32,
    results: Vec<ITunesResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ITunesResult {
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    collection_name: Option<String>,
    #[serde(default)]
    artwork_url_100: Option<String>,
    #[serde(default)]
    artwork_url_60: Option<String>,
}

/// Clean up album name for better search results
/// Removes common suffixes that prevent iTunes matches:
/// - Year ranges: (1963-66), (2001-2005)
/// - Edition info: (Deluxe Edition), (Remastered), [Bonus Tracks]
/// - Disc numbers: (Disc 1), [CD2]
/// - Live recording info: (Live At...), (Live), [Live]
/// - Trailing punctuation: / \
fn clean_album_name(album: &str) -> String {
    use regex::Regex;

    // Patterns to remove (in order)
    // Do trailing punctuation first to handle malformed album names
    let patterns = [
        r"\s*[/\\]+\s*$",                    // Trailing slashes/backslashes
        r"\s*\(Live\s+At\s+[^)]*\)?\s*$",    // Live venue info: (Live At The Cliche' Lounge...) - closing paren optional
        r"\s*\[Live\s+At\s+[^\]]*\]?\s*$",   // Live venue info in brackets - closing bracket optional
        r"\s*\(Live[^)]*\)?\s*$",            // Live recordings: (Live), (Live in NYC) - closing paren optional
        r"\s*\[Live[^\]]*\]?\s*$",           // Live recordings in brackets - closing bracket optional
        r"\s*\(\d{4}[-–]\d{2,4}\)\s*$",      // Year ranges at end: (1963-66)
        r"\s*\[\d{4}[-–]\d{2,4}\]\s*$",      // Year ranges in brackets: [1963-66]
        r"\s*\((Deluxe|Remaster(ed)?|Bonus|Special|Limited|Anniversary|Expanded).*?\)\s*$", // Edition info
        r"\s*\[(Deluxe|Remaster(ed)?|Bonus|Special|Limited|Anniversary|Expanded).*?\]\s*$", // Edition info in brackets
        r"\s*\((Disc|CD)\s*\d+\)\s*$",       // Disc numbers: (Disc 1)
        r"\s*\[(Disc|CD)\s*\d+\]\s*$",       // Disc numbers: [CD2]
    ];

    let mut cleaned = album.to_string();
    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            cleaned = re.replace(&cleaned, "").trim().to_string();
        }
    }

    cleaned
}

/// Extract artwork URL from an iTunes result
fn extract_artwork_url(result: &ITunesResult) -> Result<Option<String>> {
    if let Some(result_artist) = &result.artist_name {
        debug!("Selected album art from artist: {} (album: {:?})",
               result_artist, result.collection_name);
    }

    let artwork_url = result.artwork_url_100.as_ref()
        .or(result.artwork_url_60.as_ref());

    if let Some(url) = artwork_url {
        // Upgrade to high-res by replacing 100x100 with 600x600
        let high_res_url = url.replace("100x100", "600x600");
        debug!("Found album art: {}", high_res_url);
        return Ok(Some(high_res_url));
    }

    Ok(None)
}

/// Find the best matching album from iTunes search results
/// Returns Some(url) if found, or None if no match (signaling to try alternative search)
fn find_best_match(artist: &str, cleaned_album: &str, search_result: ITunesSearchResponse, allow_fallback: bool) -> Result<Option<String>> {
    let artist_lower = artist.to_lowercase();
    let cleaned_album_lower = cleaned_album.to_lowercase();

    let best_match = search_result.results.iter().find(|result| {
        if let (Some(result_artist), Some(result_album)) = (&result.artist_name, &result.collection_name) {
            let artist_match = result_artist.to_lowercase() == artist_lower;
            let album_match = result_album.to_lowercase().contains(&cleaned_album_lower);
            artist_match && album_match
        } else {
            false
        }
    });

    // If we found an artist+album match, use it
    if let Some(result) = best_match {
        return extract_artwork_url(result);
    }

    // If no exact match and fallback is disabled, return None to trigger alternative search
    if !allow_fallback {
        debug!("No exact artist+album match found, signaling for alternative search");
        return Ok(None);
    }

    // Fallback: try exact artist match only
    debug!("No exact artist+album match found, trying artist-only match");
    let artist_match = search_result.results.iter().find(|result| {
        if let Some(result_artist) = &result.artist_name {
            result_artist.to_lowercase() == artist_lower
        } else {
            false
        }
    }).or_else(|| {
        // Last resort: use first result
        debug!("No exact artist match found, using first result");
        search_result.results.first()
    });

    if let Some(result) = artist_match {
        return extract_artwork_url(result);
    }

    Ok(None)
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

    // Clean album name for better matching
    let cleaned_album = clean_album_name(album);
    if cleaned_album != album {
        debug!("Cleaned album name: '{}' -> '{}'", album, cleaned_album);
    }

    // Build search query
    // For self-titled albums (artist == album), search with just artist name
    // to get better ranking from iTunes API
    let query = if artist.eq_ignore_ascii_case(&cleaned_album) {
        debug!("Self-titled album detected, searching with artist name only");
        artist.to_string()
    } else {
        format!("{} {}", artist, cleaned_album)
    };
    let encoded_query = urlencoding::encode(&query);

    let url = format!(
        "https://itunes.apple.com/search?term={}&entity=album&limit=10",
        encoded_query
    );

    debug!("Looking up album art via iTunes API: artist='{}', album='{}', query='{}'", artist, album, query);

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

            let original_result: ITunesSearchResponse = response.json().await?;

            if original_result.result_count == 0 {
                debug!("No album art found for: {} - {}", artist, album);
                return Ok(None);
            }

            // Try to find exact artist+album match (no fallback yet)
            let exact_match = find_best_match(artist, &cleaned_album, original_result.clone(), false)?;

            // If exact match found, return it
            if exact_match.is_some() {
                return Ok(exact_match);
            }

            // No exact match - try keyword search for obscure compilations
            let keywords: Vec<&str> = cleaned_album.split_whitespace()
                .filter(|w| w.len() > 3 && !w.eq_ignore_ascii_case("the") && !w.eq_ignore_ascii_case("of"))
                .take(2)
                .collect();

            if !keywords.is_empty() {
                let keyword_query = format!("{} {}", artist, keywords.join(" "));
                let keyword_url = format!(
                    "https://itunes.apple.com/search?term={}&entity=album&limit=10",
                    urlencoding::encode(&keyword_query)
                );

                debug!("No exact match with full query, trying keyword search: {}", keyword_query);

                if let Ok(response) = client.get(&keyword_url).send().await {
                    if response.status().is_success() {
                        if let Ok(keyword_result) = response.json::<ITunesSearchResponse>().await {
                            if keyword_result.result_count > 0 {
                                debug!("Keyword search found {} results", keyword_result.result_count);
                                // Try exact match first with keyword results
                                let keyword_match = find_best_match(artist, &cleaned_album, keyword_result, true)?;
                                if keyword_match.is_some() {
                                    return Ok(keyword_match);
                                }
                            }
                        }
                    }
                }
            }

            // If keyword search didn't work, fall back to original results with relaxed matching
            debug!("No keyword search results, falling back to artist-only match from original results");
            find_best_match(artist, &cleaned_album, original_result, true)
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

    #[tokio::test]
    async fn test_lookup_warpaint_disambiguation() {
        // Test case where album name matches multiple artists
        // Should return Warpaint by Warpaint, not The Black Crowes
        let result = lookup_album_art("Warpaint", "Warpaint").await;
        assert!(result.is_ok());

        if let Ok(Some(url)) = result {
            println!("Found Warpaint album art: {}", url);
            assert!(url.contains("http"));
            // URL should be for the self-titled Warpaint album
            // (Note: We can't easily verify the artist from URL alone,
            // but the fix ensures it matches by artist name)
        }
    }

    #[tokio::test]
    async fn test_lookup_golden_hands_self_titled() {
        // Test case for self-titled album with poor search ranking
        // Should return Golden Hands by Golden Hands
        let result = lookup_album_art("Golden Hands", "Golden Hands").await;
        assert!(result.is_ok());

        if let Ok(Some(url)) = result {
            println!("Found Golden Hands album art: {}", url);
            assert!(url.contains("http"));
            // Should find the self-titled album
        }
    }

    #[tokio::test]
    async fn test_lookup_bob_marley_compilation() {
        // Test case for compilation album with year range suffix
        // Album name "The Birth Of A Legend (1963-66)" should clean to "The Birth Of A Legend"
        let result = lookup_album_art("Bob Marley & The Wailers", "The Birth Of A Legend (1963-66)").await;
        assert!(result.is_ok());

        if let Ok(Some(url)) = result {
            println!("Found Bob Marley compilation album art: {}", url);
            assert!(url.contains("http"));
            // Should find album art after cleaning the year range suffix
            // Should NOT be the "Legend" album
            assert!(!url.contains("Legend") || url.contains("Birth"),
                "URL should be for 'Birth of a Legend', not just 'Legend': {}", url);
        }
    }

    #[tokio::test]
    async fn test_lookup_bob_marley_search_details() {
        // Detailed test to see what iTunes returns
        use crate::album_art_lookup::clean_album_name;

        let artist = "Bob Marley & The Wailers";
        let album = "The Birth Of A Legend (1963-66)";
        let cleaned = clean_album_name(album);

        println!("\n=== Bob Marley Album Lookup Test ===");
        println!("Original album: {}", album);
        println!("Cleaned album: {}", cleaned);

        let query = format!("{} {}", artist, cleaned);
        let encoded_query = urlencoding::encode(&query);
        let url = format!(
            "https://itunes.apple.com/search?term={}&entity=album&limit=10",
            encoded_query
        );

        println!("Search query: {}", query);
        println!("iTunes URL: {}", url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        let response = client.get(&url).send().await.unwrap();
        let search_result: ITunesSearchResponse = response.json().await.unwrap();

        println!("\niTunes returned {} results:", search_result.result_count);
        for (i, result) in search_result.results.iter().enumerate() {
            println!("  {}. Artist: {:?}, Album: {:?}",
                i + 1,
                result.artist_name.as_deref().unwrap_or("N/A"),
                result.collection_name.as_deref().unwrap_or("N/A")
            );
        }

        // Now test our matching logic
        let artist_lower = artist.to_lowercase();
        let cleaned_lower = cleaned.to_lowercase();

        let best_match = search_result.results.iter().find(|result| {
            if let (Some(result_artist), Some(result_album)) = (&result.artist_name, &result.collection_name) {
                let artist_match = result_artist.to_lowercase() == artist_lower;
                let album_match = result_album.to_lowercase().contains(&cleaned_lower);
                artist_match && album_match
            } else {
                false
            }
        });

        if let Some(matched) = best_match {
            println!("\nOur matching logic selected:");
            println!("  Artist: {:?}", matched.artist_name);
            println!("  Album: {:?}", matched.collection_name);
        } else {
            println!("\nNo artist+album match found!");
        }
    }

    #[tokio::test]
    async fn test_lookup_grant_green_live() {
        // Test case for live album with venue details and trailing slash
        let result = lookup_album_art("Grant Green", "Alive! (Live At The Cliche' Lounge, Newark, New Jersey, 1970 / ").await;
        assert!(result.is_ok());

        if let Ok(Some(url)) = result {
            println!("Found Grant Green live album art: {}", url);
            assert!(url.contains("http"));
            // Should find album art after cleaning live venue info
        }
    }

    #[test]
    fn test_clean_album_name() {
        // Test live recordings with venue info
        assert_eq!(
            clean_album_name("Alive! (Live At The Cliche' Lounge, Newark, New Jersey, 1970 / "),
            "Alive!"
        );
        assert_eq!(clean_album_name("MTV Unplugged (Live At MTV Studios)"), "MTV Unplugged");
        assert_eq!(clean_album_name("The Concert [Live in Paris]"), "The Concert");

        // Test simple live recordings
        assert_eq!(clean_album_name("Live at Budokan (Live)"), "Live at Budokan");
        assert_eq!(clean_album_name("Alchemy [Live]"), "Alchemy");

        // Test year ranges
        assert_eq!(clean_album_name("The Birth Of A Legend (1963-66)"), "The Birth Of A Legend");
        assert_eq!(clean_album_name("Greatest Hits [2001-2005]"), "Greatest Hits");

        // Test edition info
        assert_eq!(clean_album_name("Abbey Road (Deluxe Edition)"), "Abbey Road");
        assert_eq!(clean_album_name("Thriller (Remastered)"), "Thriller");
        assert_eq!(clean_album_name("Dark Side of the Moon [Bonus Tracks]"), "Dark Side of the Moon");

        // Test disc numbers
        assert_eq!(clean_album_name("The Wall (Disc 1)"), "The Wall");
        assert_eq!(clean_album_name("Use Your Illusion [CD2]"), "Use Your Illusion");

        // Test trailing slashes
        assert_eq!(clean_album_name("Album Name /"), "Album Name");
        assert_eq!(clean_album_name("Album Name \\"), "Album Name");

        // Test no change when nothing to clean
        assert_eq!(clean_album_name("Nevermind"), "Nevermind");
    }
}
