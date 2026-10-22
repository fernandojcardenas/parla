//! User settings, saved as JSON in the app's config folder.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_CLAUDE_MODEL: &str = "claude-haiku-5-5";

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
    /// Send the transcript text (never audio) to Claude for a final cleanup.
    /// Off by default: Parla stays fully offline unless this is turned on.
    pub ai_cleanup: bool,
    /// Claude model used for AI cleanup.
    pub claude_model: String,
    /// Workspace ID for organization-wide API keys. Empty when not needed.
    pub anthropic_workspace_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: "Alt+Space".into(),
            microphone: None,
            remove_fillers: true,
            restore_clipboard: true,
            vocabulary: Vec::new(),
            ai_cleanup: false,
            claude_model: DEFAULT_CLAUDE_MODEL.into(),
            anthropic_workspace_id: String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_cleanup_is_off_by_default() {
        let s = Settings::default();
        assert!(!s.ai_cleanup);
        assert_eq!(s.claude_model, "claude-haiku-5-5");
    }

    #[test]
    fn old_settings_files_still_load() {
        let s: Settings = serde_json::from_str(r#"{"shortcut":"Alt+Space","remove_fillers":false}"#).unwrap();
        assert!(!s.ai_cleanup);
        assert!(!s.remove_fillers);
        assert_eq!(s.claude_model, DEFAULT_CLAUDE_MODEL);
    }
}
