use serde::{Deserialize, Serialize};

/// Response from WiiM getPlayerStatus
/// See: HTTP API for WiiM Mini, section 2.3.1
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayerStatus {
    #[serde(default)]
    pub r#type: String,  // "0" = master/standalone, "1" = slave

    #[serde(default)]
    pub ch: String,  // "0" = stereo, "1" = left, "2" = right

    #[serde(default)]
    pub mode: String,  // See mode table in docs (10-19 = Wiimu playlist, 31 = Spotify, etc.)

    #[serde(default)]
    pub r#loop: String,  // "0" = loop all, "1" = single loop, etc.

    #[serde(default)]
    pub eq: String,  // The preset number of the Equalizer

    #[serde(default)]
    pub status: String,  // "stop", "play", "loading", "pause"

    #[serde(default)]
    pub curpos: String,  // Position in ms

    #[serde(default)]
    pub offset_pts: String,

    #[serde(default)]
    pub totlen: String,  // Duration in ms

    #[serde(default)]
    pub alarmflag: String,

    #[serde(default)]
    pub plicount: String,  // Total number of tracks in playlist

    #[serde(default)]
    pub plicurr: String,  // Current track index

    #[serde(default)]
    pub vol: String,  // Current volume (0-100)

    #[serde(default)]
    pub mute: String,  // Current mute state ("0" = unmuted, "1" = muted)

    // Additional fields that may be present (from metadata)
    // Note: Some sources use lowercase (title, artist, album),
    // while others like Spotify (mode 31) use uppercase (Title, Artist, Album)
    // with hex-encoded values
    #[serde(default, alias = "Title")]
    pub title: String,

    #[serde(default, alias = "Artist")]
    pub artist: String,

    #[serde(default, alias = "Album")]
    pub album: String,

    #[serde(default)]
    pub vendor: String,
}

/// Response from WiiM getStatusEx
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusEx {
    #[serde(default)]
    pub ssid: String,  // Device name

    #[serde(default)]
    pub uuid: String,  // Unique device ID

    #[serde(rename = "DeviceName", default)]
    pub device_name: String,

    #[serde(default)]
    pub firmware: String,
}

/// Generic JSON status response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusResponse {
    pub status: String,  // "OK" or "Failed"
}

/// EQ status response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EqStatResponse {
    #[serde(rename = "EQStat")]
    pub eq_stat: String,  // "On" or "Off"
}
