//! User settings, saved as JSON in the app's config folder.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Hold this to dictate. Uses Tauri's shortcut format, e.g. "Alt+Space".
    pub shortcut: String,
    /// Microphone name, or None for the system default.
    pub microphone: Option<String>,
    pub remove_fillers: bool,
    /// Put the user's clipboard back after pasting.
    pub restore_clipboard: bool,
    /// Names and terms to spell correctly.
    pub vocabulary: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: "Alt+Space".into(),
            microphone: None,
            remove_fillers: true,
            restore_clipboard: true,
            vocabulary: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, serde_json::to_string_pretty(self).unwrap())
    }
}
