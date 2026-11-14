//! Settings model and persistence: one JSON file under `%APPDATA%\KeySwitch` written with
//! `std::fs` (no Tauri `fs` plugin, nothing reachable from the WebView). Holds preferences
//! and the Macro Studio slot (the user's own macro text, see [`crate::macro_slot`]) — never
//! text read from another application.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hotkey::Hotkey;
use crate::macro_slot::MacroSlot;

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
    /// The single Macro Studio slot (MVP).
    pub macro_slot: MacroSlot,
}

impl Default for Stored {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::default(),
            ui_language: UiLanguage::Ru,
            theme: Theme::System,
            excluded_user: Vec::new(),
            macro_slot: MacroSlot::default(),
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
        self.macro_slot = std::mem::take(&mut self.macro_slot).sanitized();
        self
    }

    /// Removes a user entry. The name is normalised as when it was added, so the case and
    /// surrounding spaces do not matter (QA-B1-05). `false` if there was no such entry.
    pub fn remove_user_excluded(&mut self, exe: &str) -> bool {
        let Some(name) = ks_engine::normalize_exe(exe) else {
            return false;
        };
        let before = self.excluded_user.len();
        self.excluded_user.retain(|e| *e != name);
        self.excluded_user.len() != before
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
    fn removal_normalises_the_name() {
        let mut s = Stored {
            excluded_user: vec!["keepass2.exe".into(), "my app.exe".into()],
            ..Stored::default()
        };
        assert!(s.remove_user_excluded("  KeePass2.EXE "));
        assert!(s.remove_user_excluded("My App.exe"));
        assert!(s.excluded_user.is_empty());
        assert!(!s.remove_user_excluded("bad/name.exe"));
    }

    #[test]
    fn corrupt_or_missing_file_gives_safe_defaults() {
        let dir = std::env::temp_dir().join(format!("ks-settings-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("settings.json");
        // A file that does not exist, and a path that is a directory.
        assert_eq!(load(Some(&dir.join("absent.json"))), Stored::default());
        assert_eq!(load(Some(&dir)), Stored::default());
        let mut oversized = br#"{"uiLanguage":"en","excludedUser":["#.to_vec();
        oversized.extend(std::iter::repeat_n(&b"\"a.exe\","[..], 40_000).flatten());
        assert!(oversized.len() > 256 * 1024);
        oversized.extend(br#""b.exe"]}"#);
        let damaged: [&[u8]; 9] = [
            b"",
            b"{ not json",
            br#"{"uiLanguage":"en","theme":"#,
            &[0xFF, 0xFE, 0x00, 0x7B, 0x00, 0xD8],
            b"[1, 2, 3]",
            b"null",
            br#"{"uiLanguage":"klingon"}"#,
            br#"{"hotkey":5,"uiLanguage":"en"}"#,
            &oversized,
        ];
        for bytes in damaged {
            std::fs::write(&path, bytes).expect("write");
            assert_eq!(load(Some(&path)), Stored::default());
            // Loading never rewrites the file (nothing is lost before the user saves).
            assert_eq!(std::fs::read(&path).expect("read"), bytes);
        }
        // Well-formed JSON with an invalid hotkey keeps the valid fields, resets the hotkey.
        std::fs::write(
            &path,
            br#"{"uiLanguage":"en","hotkey":{"key":"NoSuchKey","modifiers":[]}}"#,
        )
        .expect("write");
        let loaded = load(Some(&path));
        assert_eq!(loaded.ui_language, UiLanguage::En);
        assert_eq!(loaded.hotkey, Hotkey::default());
        // The next save replaces a damaged file with a readable one.
        std::fs::write(&path, b"{ not json").expect("write");
        save(Some(&path), &Stored::default()).expect("save");
        assert_eq!(load(Some(&path)), Stored::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_gives_defaults() {
        assert_eq!(load(None), Stored::default());
        assert!(save(None, &Stored::default()).is_err());
    }
}
