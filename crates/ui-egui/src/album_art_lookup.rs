//! Album art lookup from external services
//!
//! Fetches album artwork URLs using track metadata (artist, album, title)
//! from various free music databases.
//!
//! The same iTunes album search also supplies the album's primary genre, which
//! is used to fill in a track's genre when neither the device nor a manual
//! override provides one. Results are cached in memory for the session so the
//! album art and genre lookups share a single request per album.

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::OnceCell;
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
    #[serde(default)]
    primary_genre_name: Option<String>,
}

/// Album metadata found via the iTunes Search API
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlbumInfo {
    /// High-resolution artwork URL
    pub artwork_url: Option<String>,
    /// iTunes primary genre for the album (only set for artist matches)
    pub genre: Option<String>,
}

/// How closely an iTunes result matched the requested artist/album
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchQuality {
    ArtistAndAlbum,
    ArtistOnly,
    /// Last-resort first result: fine for artwork, not trusted for genre
    FirstResult,
}

/// Returns true when a genre value carries no real information
pub fn genre_is_missing(genre: &str) -> bool {
    let genre = genre.trim();
    genre.is_empty() || genre.eq_ignore_ascii_case("unknown")
}

/// Returns true when artist metadata is a placeholder rather than a real artist
fn is_placeholder_artist(artist: &str) -> bool {
    artist.is_empty() || artist == "Unknown" || artist == "Not playing"
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
fn extract_artwork_url(result: &ITunesResult) -> Option<String> {
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
        return Some(high_res_url);
    }

    None
}

/// Extract a non-empty genre from an iTunes result
fn extract_genre(result: &ITunesResult) -> Option<String> {
    result
        .primary_genre_name
        .as_deref()
        .map(str::trim)
        .filter(|genre| !genre_is_missing(genre))
        .map(str::to_string)
}

/// Build album info from a matched result.
/// Genre is only taken from results that matched the artist, so a last-resort
/// "first result" guess can never select the wrong EQ preset.
fn to_album_info(result: &ITunesResult, quality: MatchQuality) -> AlbumInfo {
    let genre = match quality {
        MatchQuality::ArtistAndAlbum | MatchQuality::ArtistOnly => extract_genre(result),
        MatchQuality::FirstResult => None,
    };

    AlbumInfo {
        artwork_url: extract_artwork_url(result),
        genre,
    }
}

/// Find the best matching album from iTunes search results
/// Returns Some(info) if found, or None if no match (signaling to try alternative search)
fn find_best_match(
    artist: &str,
    cleaned_album: &str,
    search_result: &ITunesSearchResponse,
    allow_fallback: bool,
) -> Option<AlbumInfo> {
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
        return Some(to_album_info(result, MatchQuality::ArtistAndAlbum));
    }

    // If no exact match and fallback is disabled, return None to trigger alternative search
    if !allow_fallback {
        debug!("No exact artist+album match found, signaling for alternative search");
        return None;
    }

    // Fallback: try exact artist match only
    debug!("No exact artist+album match found, trying artist-only match");
    let artist_match = search_result.results.iter().find(|result| {
        if let Some(result_artist) = &result.artist_name {
            result_artist.to_lowercase() == artist_lower
        } else {
            false
        }
    });

    if let Some(result) = artist_match {
        return Some(to_album_info(result, MatchQuality::ArtistOnly));
    }

    // Last resort: use first result
    debug!("No exact artist match found, using first result");
    search_result
        .results
        .first()
        .map(|result| to_album_info(result, MatchQuality::FirstResult))
}

/// Session cache of one-time async lookups, keyed by normalized metadata.
/// Each key holds a `OnceCell`, so concurrent callers share a single request.
type LookupCache<T> = OnceLock<Mutex<HashMap<String, Arc<OnceCell<T>>>>>;

static ALBUM_INFO_CACHE: LookupCache<AlbumInfo> = OnceLock::new();
static ARTIST_GENRE_CACHE: LookupCache<Option<String>> = OnceLock::new();

/// Non-blocking genre state used by the polling loop
#[derive(Debug, Clone)]
enum GenreLookup {
    Pending,
    Done(Option<String>),
}

static GENRE_CACHE: OnceLock<Mutex<HashMap<String, GenreLookup>>> = OnceLock::new();

/// Get (or create) the cell for `key` in a lookup cache
fn cached_cell<T>(cache: &'static LookupCache<T>, key: String) -> Arc<OnceCell<T>> {
    let mut map = cache
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(key).or_default().clone()
}

/// Normalized cache key for an artist/album pair
fn album_cache_key(artist: &str, album: &str) -> String {
    format!(
        "{}|{}",
        artist.trim().to_lowercase(),
        album.trim().to_lowercase()
    )
}

/// Lookup album art URL using iTunes Search API
///
/// The iTunes Search API is free and doesn't require authentication.
/// Returns high-resolution album artwork (600x600) by modifying the URL.
pub async fn lookup_album_art(artist: &str, album: &str) -> Result<Option<String>> {
    Ok(lookup_album_info(artist, album).await.artwork_url)
}

/// Lookup album artwork and genre using the iTunes Search API.
///
/// Results (including misses and errors) are cached for the session, so each
/// album is requested at most once no matter how many callers ask for it.
pub async fn lookup_album_info(artist: &str, album: &str) -> AlbumInfo {
    let cell = cached_cell(&ALBUM_INFO_CACHE, album_cache_key(artist, album));
    cell.get_or_init(|| fetch_album_info(artist, album))
        .await
        .clone()
}

/// Fetch album info with a timeout, treating any failure as "no info"
async fn fetch_album_info(artist: &str, album: &str) -> AlbumInfo {
    // Wrap entire operation in timeout to prevent hanging
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        lookup_album_info_impl(artist, album),
    )
    .await
    {
        Ok(Ok(info)) => info,
        Ok(Err(e)) => {
            warn!(
                "iTunes album lookup failed for {} - {}: {}",
                artist, album, e
            );
            AlbumInfo::default()
        }
        Err(_) => {
            warn!(
                "Album art lookup timed out after 10 seconds for: {} - {}",
                artist, album
            );
            AlbumInfo::default()
        }
    }
}

/// Lookup a genre for a track: the album's iTunes genre, falling back to the
/// artist's primary iTunes genre. Returns None if neither is found.
pub async fn lookup_genre(artist: &str, album: &str) -> Option<String> {
    if is_placeholder_artist(artist) {
        return None;
    }

    if let Some(genre) = lookup_album_info(artist, album).await.genre {
        debug!("Found album genre for {} - {}: {}", artist, album, genre);
        return Some(genre);
    }

    debug!(
        "No album genre for {} - {}, trying artist genre",
        artist, album
    );
    let cell = cached_cell(&ARTIST_GENRE_CACHE, artist.trim().to_lowercase());
    cell.get_or_init(|| fetch_artist_genre(artist))
        .await
        .clone()
}

/// Non-blocking genre lookup for the polling loop.
///
/// Returns the genre if a lookup for this artist/album has already finished.
/// Otherwise starts one in the background (once per artist/album) and returns
/// None; a later poll picks up the result.
pub fn cached_genre(artist: &str, album: &str) -> Option<String> {
    if is_placeholder_artist(artist) {
        return None;
    }

    let key = album_cache_key(artist, album);
    let mut cache = GENRE_CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    match cache.get(&key) {
        Some(GenreLookup::Done(genre)) => return genre.clone(),
        Some(GenreLookup::Pending) => return None,
        None => {}
    }

    // Need a tokio runtime to look up in the background; fail quietly without one
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return None;
    };

    cache.insert(key.clone(), GenreLookup::Pending);
    drop(cache);

    let (artist, album) = (artist.to_string(), album.to_string());
    runtime.spawn(async move {
        let genre = lookup_genre(&artist, &album).await;
        debug!("Genre lookup for {} - {}: {:?}", artist, album, genre);
        GENRE_CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, GenreLookup::Done(genre));
    });

    None
}

/// Find the genre of the result whose artist name matches exactly
fn find_artist_genre(artist: &str, search_result: &ITunesSearchResponse) -> Option<String> {
    search_result
        .results
        .iter()
        .filter(|result| {
            result
                .artist_name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(artist.trim()))
        })
        .find_map(extract_genre)
}

/// Fetch an artist's primary genre with a timeout, treating any failure as None
async fn fetch_artist_genre(artist: &str) -> Option<String> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        lookup_artist_genre_impl(artist),
    )
    .await
    {
        Ok(Ok(genre)) => genre,
        Ok(Err(e)) => {
            warn!("iTunes artist genre lookup failed for {}: {}", artist, e);
            None
        }
        Err(_) => {
            warn!(
                "Artist genre lookup timed out after 10 seconds for: {}",
                artist
            );
            None
        }
    }
}

/// Internal implementation of artist genre lookup
async fn lookup_artist_genre_impl(artist: &str) -> Result<Option<String>> {
    let url = format!(
        "https://itunes.apple.com/search?term={}&entity=musicArtist&limit=5",
        urlencoding::encode(artist.trim())
    );

    debug!(
        "Looking up artist genre via iTunes API: artist='{}'",
        artist
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        warn!("iTunes API returned status: {}", response.status());
        return Ok(None);
    }

    let search_result: ITunesSearchResponse = response.json().await?;
    Ok(find_artist_genre(artist, &search_result))
}

/// Internal implementation of album info lookup
async fn lookup_album_info_impl(artist: &str, album: &str) -> Result<AlbumInfo> {
    // Skip lookup if metadata is missing or placeholder
    if is_placeholder_artist(artist) || album.is_empty() || album == "Unknown" {
        debug!("Skipping album art lookup - invalid metadata");
        return Ok(AlbumInfo::default());
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
                return Ok(AlbumInfo::default());
            }

            let original_result: ITunesSearchResponse = response.json().await?;

            if original_result.result_count == 0 {
                debug!("No album art found for: {} - {}", artist, album);
                return Ok(AlbumInfo::default());
            }

            // Try to find exact artist+album match (no fallback yet)
            // If exact match found, return it
            if let Some(info) = find_best_match(artist, &cleaned_album, &original_result, false) {
                return Ok(info);
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
                                if let Some(info) =
                                    find_best_match(artist, &cleaned_album, &keyword_result, true)
                                {
                                    return Ok(info);
                                }
                            }
                        }
                    }
                }
            }

            // If keyword search didn't work, fall back to original results with relaxed matching
            debug!("No keyword search results, falling back to artist-only match from original results");
            Ok(find_best_match(artist, &cleaned_album, &original_result, true).unwrap_or_default())
        }
        Err(e) => {
            warn!("Failed to fetch from iTunes API: {}", e);
            Ok(AlbumInfo::default())
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

    /// Build an iTunes response from JSON, the same way the API parses it
    fn parse_response(json: &str) -> ITunesSearchResponse {
        serde_json::from_str(json).expect("valid iTunes JSON")
    }

    #[test]
    fn test_parses_primary_genre_name() {
        let response = parse_response(
            r#"{"resultCount": 1, "results": [{
                "artistName": "Miles Davis",
                "collectionName": "Kind of Blue",
                "artworkUrl100": "https://example.com/100x100bb.jpg",
                "primaryGenreName": "Jazz"
            }]}"#,
        );

        let info = find_best_match("Miles Davis", "Kind of Blue", &response, false)
            .expect("artist+album match");
        assert_eq!(info.genre.as_deref(), Some("Jazz"));
        assert_eq!(
            info.artwork_url.as_deref(),
            Some("https://example.com/600x600bb.jpg")
        );
    }

    #[test]
    fn test_genre_from_artist_only_match() {
        let response = parse_response(
            r#"{"resultCount": 2, "results": [
                {"artistName": "Someone Else", "collectionName": "Other", "primaryGenreName": "Pop"},
                {"artistName": "Miles Davis", "collectionName": "Bitches Brew", "primaryGenreName": "Jazz"}
            ]}"#,
        );

        let info = find_best_match("Miles Davis", "Kind of Blue", &response, true)
            .expect("artist-only match");
        assert_eq!(info.genre.as_deref(), Some("Jazz"));
    }

    #[test]
    fn test_no_genre_from_first_result_fallback() {
        // Neither artist nor album match: artwork may fall back to the first
        // result, but its genre must not be trusted
        let response = parse_response(
            r#"{"resultCount": 1, "results": [{
                "artistName": "Someone Else",
                "collectionName": "Other",
                "artworkUrl100": "https://example.com/100x100bb.jpg",
                "primaryGenreName": "Pop"
            }]}"#,
        );

        let info = find_best_match("Miles Davis", "Kind of Blue", &response, true)
            .expect("first-result fallback");
        assert!(info.artwork_url.is_some());
        assert_eq!(info.genre, None);
    }

    #[test]
    fn test_missing_or_blank_genre_is_none() {
        let response = parse_response(
            r#"{"resultCount": 2, "results": [
                {"artistName": "Miles Davis", "collectionName": "Kind of Blue"},
                {"artistName": "Miles Davis", "collectionName": "Kind of Blue (Legacy)", "primaryGenreName": "  "}
            ]}"#,
        );

        let info = find_best_match("Miles Davis", "Kind of Blue", &response, false)
            .expect("artist+album match");
        assert_eq!(info.genre, None);
    }

    #[test]
    fn test_find_artist_genre_requires_exact_artist() {
        let response = parse_response(
            r#"{"resultCount": 2, "results": [
                {"artistName": "Miles Davis Tribute Band", "primaryGenreName": "Pop"},
                {"artistName": "Miles Davis", "primaryGenreName": "Jazz"}
            ]}"#,
        );
        assert_eq!(
            find_artist_genre("miles davis", &response).as_deref(),
            Some("Jazz")
        );

        let no_match = parse_response(
            r#"{"resultCount": 1, "results": [
                {"artistName": "Miles Davis Tribute Band", "primaryGenreName": "Pop"}
            ]}"#,
        );
        assert_eq!(find_artist_genre("Miles Davis", &no_match), None);
    }

    #[test]
    fn test_genre_is_missing() {
        assert!(genre_is_missing(""));
        assert!(genre_is_missing("   "));
        assert!(genre_is_missing("Unknown"));
        assert!(genre_is_missing("unknown"));
        assert!(!genre_is_missing("Rock"));
    }

    #[test]
    fn test_album_cache_key_normalizes() {
        assert_eq!(
            album_cache_key("  Pink Floyd ", "The Dark Side of the Moon"),
            album_cache_key("pink floyd", "THE DARK SIDE OF THE MOON ")
        );
        assert_ne!(
            album_cache_key("Pink Floyd", "The Wall"),
            album_cache_key("Pink Floyd", "Animals")
        );
    }

    #[test]
    fn test_cached_genre_skips_placeholder_artist() {
        // No runtime needed and no lookup started for placeholder metadata
        assert_eq!(cached_genre("Unknown", "Some Album"), None);
        assert_eq!(cached_genre("", "Some Album"), None);
    }

    #[tokio::test]
    async fn test_lookup_genre_pink_floyd() {
        // Network test, like the album art tests above: only checks the result
        // when iTunes is reachable
        if let Some(genre) = lookup_genre("Pink Floyd", "The Dark Side of the Moon").await {
            println!("Found genre: {}", genre);
            assert!(!genre_is_missing(&genre));
        }
    }

    #[tokio::test]
    async fn test_lookup_genre_falls_back_to_artist() {
        // Album that doesn't exist on iTunes: genre should come from the artist
        if let Some(genre) = lookup_genre("Miles Davis", "Not A Real Album Xyzzy 12345").await {
            println!("Found artist genre: {}", genre);
            assert!(!genre_is_missing(&genre));
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
