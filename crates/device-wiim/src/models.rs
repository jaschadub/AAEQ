use serde::{Deserialize, Serialize};

/// Response from WiiM getPlayerStatus
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlayerStatus {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub genre: String,
}

/// Response from WiiM EQGetList
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EqListResponse {
    #[serde(default)]
    pub list: Vec<String>,
}

/// EQ band configuration from WiiM
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WiimEqBand {
    pub gain: f32,  // dB value
}
