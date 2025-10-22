/// DIDL-Lite XML metadata generation for UPnP/DLNA
///
/// DIDL-Lite is the metadata format used by UPnP to describe media items.
use crate::types::OutputConfig;

/// Media metadata for DIDL-Lite generation
#[derive(Debug, Clone)]
pub struct MediaMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub duration: Option<String>, // Format: H:MM:SS
    pub album_art_uri: Option<String>, // URL to album artwork
}

impl Default for MediaMetadata {
    fn default() -> Self {
        Self {
            title: "AAEQ Stream".to_string(),
            artist: None,
            album: None,
            genre: None,
            duration: None,
            album_art_uri: None,
        }
    }
}

/// Generate DIDL-Lite XML for a media item
///
/// # Arguments
/// * `uri` - The URL to the media stream
/// * `metadata` - Metadata for the media item
/// * `config` - Audio configuration (sample rate, channels, format)
///
/// # Returns
/// DIDL-Lite XML string
pub fn generate_didl_lite(
    uri: &str,
    metadata: &MediaMetadata,
    config: &OutputConfig,
) -> String {
    let mime_type = mime_type_from_config(config);
    let protocol_info = format!(
        "http-get:*:{}:DLNA.ORG_PN=WAV;DLNA.ORG_OP=01;DLNA.ORG_FLAGS=01700000000000000000000000000000",
        mime_type
    );

    let mut didl = String::new();

    didl.push_str(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" "#);
    didl.push_str(r#"xmlns:dc="http://purl.org/dc/elements/1.1/" "#);
    didl.push_str(r#"xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">"#);

    didl.push_str(r#"<item id="1" parentID="0" restricted="1">"#);

    // Title (required)
    didl.push_str(&format!("<dc:title>{}</dc:title>", escape_xml(&metadata.title)));

    // Artist (optional)
    if let Some(artist) = &metadata.artist {
        didl.push_str(&format!("<upnp:artist>{}</upnp:artist>", escape_xml(artist)));
        didl.push_str(&format!("<dc:creator>{}</dc:creator>", escape_xml(artist)));
    }

    // Album (optional)
    if let Some(album) = &metadata.album {
        didl.push_str(&format!("<upnp:album>{}</upnp:album>", escape_xml(album)));
    }

    // Genre (optional)
    if let Some(genre) = &metadata.genre {
        didl.push_str(&format!("<upnp:genre>{}</upnp:genre>", escape_xml(genre)));
    }

    // Album Art URI (optional)
    if let Some(album_art_uri) = &metadata.album_art_uri {
        didl.push_str(&format!("<upnp:albumArtURI>{}</upnp:albumArtURI>", escape_xml(album_art_uri)));
    }

    // Class
    didl.push_str("<upnp:class>object.item.audioItem.musicTrack</upnp:class>");

    // Resource (the actual stream URL)
    didl.push_str(&format!(
        r#"<res protocolInfo="{}" "#,
        escape_xml(&protocol_info)
    ));

    if let Some(duration) = &metadata.duration {
        didl.push_str(&format!(r#"duration="{}" "#, escape_xml(duration)));
    }

    didl.push_str(&format!(
        r#"sampleFrequency="{}" "#,
        config.sample_rate
    ));

    didl.push_str(&format!(
        r#"nrAudioChannels="{}" "#,
        config.channels
    ));

    didl.push_str(&format!(
        r#"bitsPerSample="{}">"#,
        config.format.bit_depth()
    ));

    didl.push_str(&escape_xml(uri));
    didl.push_str("</res>");

    didl.push_str("</item>");
    didl.push_str("</DIDL-Lite>");

    didl
}

/// Generate minimal DIDL-Lite for a simple stream
pub fn generate_simple_didl_lite(uri: &str, title: &str, config: &OutputConfig) -> String {
    let metadata = MediaMetadata {
        title: title.to_string(),
        ..Default::default()
    };

    generate_didl_lite(uri, &metadata, config)
}

/// Determine MIME type from audio configuration
fn mime_type_from_config(config: &OutputConfig) -> &'static str {
    match config.format {
        crate::types::SampleFormat::S16LE | crate::types::SampleFormat::S24LE => "audio/wav",
        crate::types::SampleFormat::F32 | crate::types::SampleFormat::F64 => "audio/wav",
    }
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SampleFormat;

    #[test]
    fn test_generate_simple_didl() {
        let config = OutputConfig {
            sample_rate: 48000,
            channels: 2,
            format: SampleFormat::S16LE,
            buffer_ms: 150,
            exclusive: false,
        };

        let didl = generate_simple_didl_lite(
            "http://192.168.1.100:8090/stream.wav",
            "Test Stream",
            &config,
        );

        assert!(didl.contains("<dc:title>Test Stream</dc:title>"));
        assert!(didl.contains("sampleFrequency=\"48000\""));
        assert!(didl.contains("nrAudioChannels=\"2\""));
        assert!(didl.contains("http://192.168.1.100:8090/stream.wav"));
    }

    #[test]
    fn test_generate_full_didl() {
        let config = OutputConfig {
            sample_rate: 44100,
            channels: 2,
            format: SampleFormat::S24LE,
            buffer_ms: 150,
            exclusive: false,
        };

        let metadata = MediaMetadata {
            title: "Test Track".to_string(),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            genre: Some("Rock".to_string()),
            duration: Some("3:45".to_string()),
            album_art_uri: None,
        };

        let didl = generate_didl_lite("http://example.com/stream.wav", &metadata, &config);

        assert!(didl.contains("<dc:title>Test Track</dc:title>"));
        assert!(didl.contains("<upnp:artist>Test Artist</upnp:artist>"));
        assert!(didl.contains("<upnp:album>Test Album</upnp:album>"));
        assert!(didl.contains("<upnp:genre>Rock</upnp:genre>"));
        assert!(didl.contains("duration=\"3:45\""));
        assert!(didl.contains("sampleFrequency=\"44100\""));
        assert!(didl.contains("bitsPerSample=\"24\""));
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(
            escape_xml("Artist & The <Band>"),
            "Artist &amp; The &lt;Band&gt;"
        );
    }
}
