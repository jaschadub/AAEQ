/// Theme system for AAEQ UI
use egui::{Color32, Visuals, Stroke};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    WinAmp,
    Vintage,
    Studio,
}

impl Theme {
    /// Convert theme to string for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::WinAmp => "winamp",
            Theme::Vintage => "vintage",
            Theme::Studio => "studio",
        }
    }

    /// Parse theme from string (deprecated - use str::parse() instead)
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// Get all available themes
    pub fn all() -> &'static [Theme] {
        &[Theme::Dark, Theme::Light, Theme::WinAmp, Theme::Vintage, Theme::Studio]
    }

    /// Get display name for theme
    pub fn display_name(&self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::WinAmp => "WinAmp",
            Theme::Vintage => "Vintage",
            Theme::Studio => "Studio",
        }
    }

    /// Convert theme to egui Visuals
    pub fn to_visuals(&self) -> Visuals {
        match self {
            Theme::Dark => Self::dark_theme(),
            Theme::Light => Self::light_theme(),
            Theme::WinAmp => Self::winamp_theme(),
            Theme::Vintage => Self::vintage_theme(),
            Theme::Studio => Self::studio_theme(),
        }
    }

    /// Dark theme (current default)
    fn dark_theme() -> Visuals {
        Visuals::dark()
    }

    /// Light theme
    fn light_theme() -> Visuals {
        Visuals::light()
    }

    /// WinAmp theme - classic media player style with green/gray and neon cyan accents
    fn winamp_theme() -> Visuals {
        let mut visuals = Visuals::dark();

        // WinAmp color palette
        let bg_dark = Color32::from_rgb(20, 20, 20);          // Almost black background
        let bg_medium = Color32::from_rgb(40, 40, 40);        // Medium gray for panels
        let bg_light = Color32::from_rgb(60, 60, 60);         // Lighter gray for widgets
        let accent_green = Color32::from_rgb(0, 255, 0);      // Classic WinAmp green
        let accent_cyan = Color32::from_rgb(0, 255, 255);     // Neon cyan
        let text_color = Color32::from_rgb(0, 240, 0);        // Green text

        visuals.window_fill = bg_dark;
        visuals.panel_fill = bg_medium;
        visuals.faint_bg_color = bg_light;
        visuals.extreme_bg_color = bg_dark;

        visuals.widgets.noninteractive.bg_fill = bg_medium;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_color);

        visuals.widgets.inactive.bg_fill = bg_light;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, accent_green);

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(50, 80, 50);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);

        visuals.widgets.active.bg_fill = Color32::from_rgb(0, 100, 0);
        visuals.widgets.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);

        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(0, 255, 0, 60);
        visuals.selection.stroke = Stroke::new(1.0, accent_green);

        visuals.hyperlink_color = accent_cyan;
        visuals.text_cursor.stroke = Stroke::new(2.0, accent_green);

        // Override text color to white for better visibility on custom backgrounds
        visuals.override_text_color = Some(Color32::WHITE);

        visuals
    }

    /// Vintage theme - warm browns and oranges like old hi-fi equipment
    fn vintage_theme() -> Visuals {
        let mut visuals = Visuals::dark();

        // Vintage color palette
        let bg_dark = Color32::from_rgb(25, 20, 15);          // Dark warm brown
        let bg_medium = Color32::from_rgb(50, 40, 30);        // Medium brown
        let bg_light = Color32::from_rgb(80, 65, 50);         // Light brown
        let accent_orange = Color32::from_rgb(255, 140, 60);  // Warm orange
        let accent_gold = Color32::from_rgb(218, 165, 32);    // Golden
        let text_color = Color32::from_rgb(245, 225, 195);    // Cream/beige text

        visuals.window_fill = bg_dark;
        visuals.panel_fill = bg_medium;
        visuals.faint_bg_color = bg_light;
        visuals.extreme_bg_color = bg_dark;

        visuals.widgets.noninteractive.bg_fill = bg_medium;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_color);

        visuals.widgets.inactive.bg_fill = bg_light;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, accent_gold);

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(100, 80, 60);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, text_color);

        visuals.widgets.active.bg_fill = Color32::from_rgb(180, 100, 40);  // Brighter orange background
        visuals.widgets.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);  // White text for max contrast

        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(255, 140, 60, 80);
        visuals.selection.stroke = Stroke::new(1.0, accent_gold);

        visuals.hyperlink_color = accent_orange;
        visuals.text_cursor.stroke = Stroke::new(2.0, accent_gold);

        // Override text color to white for better visibility on custom backgrounds
        visuals.override_text_color = Some(Color32::WHITE);

        visuals
    }

    /// Studio theme - matte black with subtle blue accents like pro audio gear
    fn studio_theme() -> Visuals {
        let mut visuals = Visuals::dark();

        // Studio color palette
        let bg_black = Color32::from_rgb(15, 15, 18);         // Deep matte black
        let bg_charcoal = Color32::from_rgb(28, 28, 32);      // Charcoal gray
        let bg_slate = Color32::from_rgb(45, 45, 50);         // Slate gray
        let accent_blue = Color32::from_rgb(100, 150, 255);   // Cool blue
        let accent_ice = Color32::from_rgb(180, 220, 255);    // Ice blue
        let text_color = Color32::from_rgb(220, 220, 225);    // Cool white-gray

        visuals.window_fill = bg_black;
        visuals.panel_fill = bg_charcoal;
        visuals.faint_bg_color = bg_slate;
        visuals.extreme_bg_color = bg_black;

        visuals.widgets.noninteractive.bg_fill = bg_charcoal;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text_color);

        visuals.widgets.inactive.bg_fill = bg_slate;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, accent_ice);

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 60, 70);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);

        visuals.widgets.active.bg_fill = Color32::from_rgb(50, 70, 100);
        visuals.widgets.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);

        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(100, 150, 255, 60);
        visuals.selection.stroke = Stroke::new(1.0, accent_blue);

        visuals.hyperlink_color = accent_ice;
        visuals.text_cursor.stroke = Stroke::new(2.0, accent_blue);

        // Override text color to white for better visibility on custom backgrounds
        visuals.override_text_color = Some(Color32::WHITE);

        visuals
    }
}

impl FromStr for Theme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            "winamp" => Ok(Theme::WinAmp),
            "vintage" => Ok(Theme::Vintage),
            "studio" => Ok(Theme::Studio),
            _ => Err(format!("Unknown theme: {}", s)),
        }
    }
}

/// Color palette for meters (Pre/Post EQ level gauges)
pub struct MeterColors {
    pub background: Color32,
    pub border: Color32,
    pub needle: Color32,
    pub peak_hold: Color32,
    pub ticks: Color32,
    pub labels: Color32,
}

/// Color palette for spectrum analyzer
pub struct SpectrumColors {
    pub background: Color32,
    pub grid: Color32,
    pub bars: Color32,
    pub peak_caps: Color32,
    pub text: Color32,
}

impl Theme {
    /// Get meter colors for this theme
    pub fn meter_colors(&self) -> MeterColors {
        match self {
            Theme::Dark => MeterColors {
                background: Color32::from_rgb(15, 25, 35),
                border: Color32::from_rgb(50, 120, 180),
                needle: Color32::from_rgb(255, 220, 100),
                peak_hold: Color32::from_rgb(255, 100, 100),
                ticks: Color32::from_rgb(150, 180, 210),
                labels: Color32::from_rgb(200, 220, 240),
            },
            Theme::Light => MeterColors {
                background: Color32::from_rgb(245, 245, 250),
                border: Color32::from_rgb(100, 130, 160),
                needle: Color32::from_rgb(180, 120, 0),
                peak_hold: Color32::from_rgb(220, 50, 50),
                ticks: Color32::from_rgb(80, 100, 120),
                labels: Color32::from_rgb(50, 50, 60),
            },
            Theme::WinAmp => MeterColors {
                background: Color32::from_rgb(20, 20, 20),
                border: Color32::from_rgb(0, 255, 0),
                needle: Color32::from_rgb(0, 255, 255),
                peak_hold: Color32::from_rgb(255, 255, 0),
                ticks: Color32::from_rgb(0, 200, 0),
                labels: Color32::from_rgb(0, 240, 0),
            },
            Theme::Vintage => MeterColors {
                background: Color32::from_rgb(25, 20, 15),
                border: Color32::from_rgb(218, 165, 32),
                needle: Color32::from_rgb(255, 200, 100),
                peak_hold: Color32::from_rgb(255, 100, 50),
                ticks: Color32::from_rgb(200, 150, 100),
                labels: Color32::from_rgb(245, 225, 195),
            },
            Theme::Studio => MeterColors {
                background: Color32::from_rgb(15, 15, 18),
                border: Color32::from_rgb(100, 150, 255),
                needle: Color32::from_rgb(180, 220, 255),
                peak_hold: Color32::from_rgb(255, 120, 120),
                ticks: Color32::from_rgb(150, 180, 220),
                labels: Color32::from_rgb(220, 220, 225),
            },
        }
    }

    /// Get spectrum analyzer colors for this theme
    pub fn spectrum_colors(&self) -> SpectrumColors {
        match self {
            Theme::Dark => SpectrumColors {
                background: Color32::from_rgb(15, 15, 20),
                grid: Color32::from_gray(40),
                bars: Color32::from_rgb(0, 200, 100),
                peak_caps: Color32::from_rgb(255, 220, 0),
                text: Color32::from_gray(180),
            },
            Theme::Light => SpectrumColors {
                background: Color32::from_rgb(245, 245, 250),
                grid: Color32::from_gray(200),
                bars: Color32::from_rgb(0, 180, 90),
                peak_caps: Color32::from_rgb(230, 180, 0),
                text: Color32::from_gray(80),
            },
            Theme::WinAmp => SpectrumColors {
                background: Color32::from_rgb(20, 20, 20),
                grid: Color32::from_rgb(0, 80, 0),
                bars: Color32::from_rgb(0, 255, 0),
                peak_caps: Color32::from_rgb(0, 255, 255),
                text: Color32::from_rgb(0, 240, 0),
            },
            Theme::Vintage => SpectrumColors {
                background: Color32::from_rgb(25, 20, 15),
                grid: Color32::from_rgb(80, 65, 50),
                bars: Color32::from_rgb(255, 140, 60),
                peak_caps: Color32::from_rgb(255, 200, 100),
                text: Color32::from_rgb(245, 225, 195),
            },
            Theme::Studio => SpectrumColors {
                background: Color32::from_rgb(15, 15, 18),
                grid: Color32::from_rgb(45, 45, 50),
                bars: Color32::from_rgb(100, 150, 255),
                peak_caps: Color32::from_rgb(180, 220, 255),
                text: Color32::from_rgb(220, 220, 225),
            },
        }
    }
}
