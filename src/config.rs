use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    Chart,
    Value,
    Both,
}

impl DisplayMode {
    pub const ALL: [DisplayMode; 3] = [DisplayMode::Chart, DisplayMode::Value, DisplayMode::Both];

    pub fn label(self) -> &'static str {
        match self {
            DisplayMode::Chart => "Chart",
            DisplayMode::Value => "Value",
            DisplayMode::Both => "Both",
        }
    }

    pub fn chart(self) -> bool {
        matches!(self, DisplayMode::Chart | DisplayMode::Both)
    }

    pub fn value(self) -> bool {
        matches!(self, DisplayMode::Value | DisplayMode::Both)
    }
}

#[derive(Debug, Clone, PartialEq, CosmicConfigEntry, Serialize, Deserialize)]
#[version = 1]
pub struct Config {
    pub display: DisplayMode,
    /// Show "S" / "W" next to the session and weekly readings.
    pub show_labels: bool,
    /// Draw a tick on the chart at how far through the window we are.
    pub show_pace: bool,
    pub refresh_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            display: DisplayMode::Both,
            show_labels: true,
            show_pace: true,
            refresh_secs: 60,
        }
    }
}
