//! Settings model and persistence: one JSON file under `%APPDATA%\KeySwitch` written with
//! `std::fs` (no Tauri `fs` plugin, nothing reachable from the WebView). Holds preferences
//! only — never user text.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hotkey::Hotkey;

/// UI language (`uiLanguage` on the UI boundary).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    Ru,
    En,
}

/// UI theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

/// Executables that are excluded out of the box and cannot be removed in the UI.
pub const DEFAULT_EXCLUDED: [&str; 6] = [
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
    "windowsterminal.exe",
    "cmd.exe",
];

/// Upper bound of user entries in the excluded list.
pub const MAX_USER_EXCLUDED: usize = 200;

/// Contents of `settings.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Stored {
    pub hotkey: Hotkey,
    pub ui_language: UiLanguage,
    pub theme: Theme,
    /// User-added excluded executables (lower-cased file names).
    pub excluded_user: Vec<String>,
}

impl Default for Stored {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::default(),
            ui_language: UiLanguage::Ru,
            theme: Theme::System,
            excluded_user: Vec::new(),
        }
    }
}

impl Stored {
    /// Drops entries that would not pass validation today (a hand-edited file is untrusted).
    fn sanitized(mut self) -> Self {
        if self.hotkey.to_chord().is_err() {
            self.hotkey = Hotkey::default();
        }
        let mut seen: Vec<String> = Vec::new();
        for name in self.excluded_user.drain(..) {
            if let Some(n) = ks_engine::normalize_exe(&name)
                && !seen.contains(&n)
                && !DEFAULT_EXCLUDED.contains(&n.as_str())
                && seen.len() < MAX_USER_EXCLUDED
            {
                seen.push(n);
            }
        }
        self.excluded_user = seen;
        self
    }

    /// Default entries followed by the user's.
    pub fn excluded_all(&self) -> Vec<String> {
        DEFAULT_EXCLUDED
            .iter()
            .map(|s| (*s).to_owned())
            .chain(self.excluded_user.iter().cloned())
            .collect()
    }
}

/// `%APPDATA%\KeySwitch\settings.json`, or `None` when `%APPDATA%` is not set.
pub fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").filter(|v| !v.is_empty())?;
    Some(PathBuf::from(base).join("KeySwitch").join("settings.json"))
}

/// Loads settings; a missing or damaged file yields the defaults.
pub fn load(path: Option<&Path>) -> Stored {
    path.and_then(|p| std::fs::read(p).ok())
        .filter(|bytes| bytes.len() <= 256 * 1024)
        .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
        .unwrap_or_default()
        .sanitized()
}

/// Writes settings through a temporary file and a rename, so a crash cannot leave a
/// half-written file.
pub fn save(path: Option<&Path>, stored: &Stored) -> Result<(), String> {
    let path = path.ok_or("settings location is unavailable")?;
    let dir = path.parent().ok_or("settings location is unavailable")?;
    let io = |e: std::io::Error| format!("settings could not be saved ({:?})", e.kind());
    std::fs::create_dir_all(dir).map_err(io)?;
    let json = serde_json::to_vec_pretty(stored).map_err(|_| "settings could not be encoded")?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_sanitizes() {
        let dir = std::env::temp_dir().join(format!("ks-settings-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let s = Stored {
            ui_language: UiLanguage::En,
            excluded_user: vec!["My App.EXE".into(), "bad/name.exe".into(), "cmd.exe".into()],
            ..Stored::default()
        };
        save(Some(&path), &s).expect("save");
        let loaded = load(Some(&path));
        assert_eq!(loaded.ui_language, UiLanguage::En);
        assert_eq!(loaded.excluded_user, vec!["my app.exe".to_owned()]);
        assert!(loaded.excluded_all().contains(&"keepass.exe".to_owned()));
        std::fs::write(&path, b"{ not json").expect("write");
        assert_eq!(load(Some(&path)), Stored::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_gives_defaults() {
        assert_eq!(load(None), Stored::default());
        assert!(save(None, &Stored::default()).is_err());
    }
}
