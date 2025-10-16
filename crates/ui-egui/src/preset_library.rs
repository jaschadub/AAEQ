/// Library of common EQ preset curves
/// These are typical frequency response patterns for standard presets

use aaeq_core::EqPreset;

/// Get a reference EQ curve for a preset name
/// Returns a known curve if available, or generates a reasonable default for unknown presets
pub fn get_preset_curve(preset_name: &str) -> Option<EqPreset> {
    get_known_preset_curve(preset_name)
        .or_else(|| generate_default_curve(preset_name))
}

/// Get a preset curve with database fallback for custom presets
/// This is an async version that checks the database before generating a default curve
pub async fn get_preset_curve_with_db(
    preset_name: &str,
    pool: &sqlx::SqlitePool,
) -> Option<EqPreset> {
    use aaeq_persistence::CustomEqPresetRepository;

    // First try built-in known presets
    if let Some(preset) = get_known_preset_curve(preset_name) {
        return Some(preset);
    }

    // Then try loading from custom presets database
    let custom_repo = CustomEqPresetRepository::new(pool.clone());
    if let Ok(Some(custom_preset)) = custom_repo.get_by_name(preset_name).await {
        return Some(custom_preset);
    }

    // Finally, generate a default curve based on name heuristics (for WiiM presets)
    generate_default_curve(preset_name)
}

/// Check if a preset has a known curve in our library
pub fn is_known_preset(preset_name: &str) -> bool {
    get_known_preset_curve(preset_name).is_some()
}

/// Get a reference EQ curve for a known preset name
/// Returns None if the preset is not in our library
fn get_known_preset_curve(preset_name: &str) -> Option<EqPreset> {
    // Standard 10-band equalizer frequencies (in Hz)
    let frequencies = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

    let gains = match preset_name {
        "Flat" => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],

        // Bass-heavy presets
        "Bass Booster" | "Bass" => [8.0, 7.0, 5.0, 2.0, 0.0, -1.0, -1.0, 0.0, 0.0, 0.0],
        "Deep" => [7.0, 6.0, 4.0, 2.0, 0.0, -1.0, -2.0, -1.0, 0.0, 0.0],
        "Hip-Hop" | "Hip Hop" => [6.0, 5.0, 3.0, 1.0, 0.0, -1.0, 0.0, 1.0, 2.0, 2.0],
        "R&B" | "R & B" | "RnB" => [5.0, 4.0, 2.0, 0.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        "Dance" | "EDM" | "Electronic" => [7.0, 5.0, 2.0, 0.0, 0.0, 0.0, 2.0, 4.0, 5.0, 6.0],

        // Rock/Metal presets
        "Rock" => [5.0, 4.0, 2.0, -1.0, -2.0, -1.0, 1.0, 3.0, 5.0, 6.0],
        "Metal" | "Hard Rock" => [6.0, 5.0, 1.0, -2.0, -3.0, -1.0, 2.0, 4.0, 6.0, 7.0],
        "Punk" => [5.0, 4.0, 2.0, 0.0, -1.0, 0.0, 2.0, 4.0, 5.0, 5.0],

        // Vocal-focused presets
        "Pop" => [3.0, 2.0, 0.0, -1.0, -2.0, 1.0, 3.0, 4.0, 4.0, 3.0],
        "Vocal" | "Vocals" | "Vocal Booster" => [2.0, 1.0, -1.0, -2.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0],
        "Classical" | "Classic" => [-2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 2.0, 1.0],

        // Jazz/Acoustic presets
        "Jazz" => [3.0, 2.0, 0.0, 1.0, 2.0, 3.0, 3.0, 2.0, 1.0, 0.0],
        "Acoustic" => [4.0, 3.0, 1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 3.0, 2.0],
        "Folk" => [3.0, 2.0, 1.0, 0.0, 1.0, 2.0, 2.0, 2.0, 1.0, 0.0],

        // Bass reducer presets
        "Bass Reducer" => [-6.0, -5.0, -3.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        "Treble Reducer" => [0.0, 0.0, 0.0, 0.0, 0.0, -1.0, -3.0, -5.0, -6.0, -6.0],

        // Treble-focused presets
        "Treble Booster" | "Treble" => [0.0, 0.0, 0.0, -1.0, 0.0, 2.0, 5.0, 7.0, 8.0, 8.0],
        "Bright" => [0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 4.0, 3.0],

        // V-shaped presets
        "V-Shape" | "V Shape" => [6.0, 5.0, 3.0, 0.0, -2.0, -2.0, 0.0, 3.0, 5.0, 6.0],
        "Live" | "Concert" => [5.0, 4.0, 2.0, 0.0, -1.0, -1.0, 1.0, 3.0, 4.0, 5.0],

        // Genre-specific presets
        "Reggae" => [5.0, 4.0, 2.0, 0.0, -2.0, 0.0, 2.0, 3.0, 4.0, 4.0],
        "Country" => [3.0, 2.0, 1.0, 0.0, 0.0, 1.0, 2.0, 3.0, 2.0, 1.0],
        "Blues" => [4.0, 3.0, 1.0, 0.0, 0.0, 1.0, 2.0, 2.0, 1.0, 0.0],
        "Funk" | "Soul" => [5.0, 4.0, 2.0, 0.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        "Latin" | "Salsa" => [4.0, 3.0, 1.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        "Loudness" => [5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 2.0, 4.0, 5.0, 5.0],
        "Lounge" => [2.0, 2.0, 1.0, 0.0, 1.0, 2.0, 2.0, 1.0, 1.0, 0.0],
        "Piano" => [-1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 2.0, 1.0, 0.0, -1.0],

        // Special use presets
        "Spoken Word" | "Podcast" | "Speech" => [-2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 2.0, 0.0, -1.0, -2.0],
        "Headphone" | "Headphones" => [3.0, 2.0, 0.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 2.0],
        "Small Speakers" => [4.0, 3.0, 2.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 1.0],
        "Large Speakers" => [0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 2.0, 2.0, 1.0],

        _ => return None,
    };

    let bands: Vec<_> = frequencies.iter()
        .zip(gains.iter())
        .map(|(&freq, &gain)| aaeq_core::EqBand { frequency: freq, gain })
        .collect();

    Some(EqPreset {
        name: preset_name.to_string(),
        bands,
    })
}

/// Get a list of all known preset names in the library
/// Includes all WiiM default presets plus common additional presets
#[allow(dead_code)]
pub fn list_known_presets() -> Vec<&'static str> {
    vec![
        // WiiM default presets (same order as WiiM API)
        "Flat",
        "Acoustic",
        "Bass Booster",
        "Bass Reducer",
        "Classical",
        "Dance",
        "Deep",
        "Electronic",
        "Hip-Hop",
        "Jazz",
        "Latin",
        "Loudness",
        "Lounge",
        "Piano",
        "Pop",
        "R&B",
        "Rock",
        "Small Speakers",
        "Spoken Word",
        "Treble Booster",
        "Treble Reducer",
        "Vocal Booster",
        // Additional common presets
        "Metal",
        "V-Shape",
        "Live",
        "Reggae",
        "Country",
        "Blues",
        "Funk",
    ]
}

/// Generate a default EQ curve for unknown presets based on name heuristics
/// This allows the app to show something reasonable for custom user presets
fn generate_default_curve(preset_name: &str) -> Option<EqPreset> {
    let frequencies = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
    let name_lower = preset_name.to_lowercase();

    // Analyze preset name to guess the EQ curve
    let gains = if name_lower.contains("bass") && (name_lower.contains("boost") || name_lower.contains("heavy")) {
        // Bass boost pattern
        [6.0, 5.0, 3.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0]
    } else if name_lower.contains("bass") && (name_lower.contains("reduc") || name_lower.contains("cut")) {
        // Bass cut pattern
        [-5.0, -4.0, -2.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    } else if name_lower.contains("treble") && (name_lower.contains("boost") || name_lower.contains("bright")) {
        // Treble boost pattern
        [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 6.0, 6.0]
    } else if name_lower.contains("treble") && (name_lower.contains("reduc") || name_lower.contains("cut")) {
        // Treble cut pattern
        [0.0, 0.0, 0.0, 0.0, 0.0, -1.0, -3.0, -5.0, -6.0, -6.0]
    } else if name_lower.contains("vocal") || name_lower.contains("voice") || name_lower.contains("speech") {
        // Vocal-focused: boost mids, cut bass and treble
        [0.0, 0.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0]
    } else if name_lower.contains("v-shape") || name_lower.contains("vshape") || name_lower.contains("smile") {
        // V-shape: boost bass and treble, cut mids
        [5.0, 4.0, 2.0, 0.0, -2.0, -2.0, 0.0, 2.0, 4.0, 5.0]
    } else if name_lower.contains("loud") {
        // Loudness curve: boost bass and treble
        [5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 2.0, 4.0, 5.0, 5.0]
    } else if name_lower.contains("flat") || name_lower.contains("neutral") || name_lower.contains("off") {
        // Flat response
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    } else {
        // Default: slight mid boost (generic music enhancement)
        [2.0, 1.0, 0.0, 0.0, 1.0, 2.0, 2.0, 1.0, 1.0, 0.0]
    };

    let bands: Vec<_> = frequencies.iter()
        .zip(gains.iter())
        .map(|(&freq, &gain)| aaeq_core::EqBand { frequency: freq, gain })
        .collect();

    Some(EqPreset {
        name: preset_name.to_string(),
        bands,
    })
}
