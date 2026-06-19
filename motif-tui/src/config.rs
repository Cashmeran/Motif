//! TUI-specific config — optional, minimal.
//! Stored at `~/.motif/tui.json`. Falls back to defaults if missing.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TuiConfig {
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            show_status_bar: true,
        }
    }
}

pub fn load_or_default() -> TuiConfig {
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".motif")
        .join("tui.json");
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
        }
    }
    TuiConfig::default()
}
