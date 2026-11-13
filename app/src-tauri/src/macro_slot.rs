//! Macro Studio slot (Beta 2 step 1): one free slot holding a **text** macro, stored in
//! `settings.json` next to the preferences.
//!
//! The content is the user's own authored text (SYSTEM_ARCHITECTURE: stored locally, it is
//! the user's data). It is never text read from another application, but it is treated the
//! same way: [`MacroContent`] has a redacted `Debug`, no `Display`, is wiped on drop and only
//! leaves as a [`UserText`] for the engine. Code macros (JS / Python) are not stored yet.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use ks_engine::{Macro, MacroBody, MacroPermissions, UserText};

use crate::hotkey::Hotkey;

/// Maximum length of the slot name, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// Text of a macro. Redacted in `Debug`, wiped on drop.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct MacroContent(String);

impl MacroContent {
    /// A copy for the engine.
    pub fn to_user_text(&self) -> UserText {
        UserText::new(self.0.clone())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for MacroContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MacroContent(<redacted>)")
    }
}

impl Drop for MacroContent {
    fn drop(&mut self) {
        ks_engine::wipe(std::mem::take(&mut self.0));
    }
}

impl Serialize for MacroContent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for MacroContent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self)
    }
}

/// Kind of macro. Only text in this step; an unknown value in a hand-edited file disables
/// the slot instead of discarding all settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MacroType {
    #[default]
    Text,
    #[serde(other)]
    Unsupported,
}

/// The slot as stored and as exchanged with the UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MacroSlot {
    pub name: String,
    pub enabled: bool,
    /// `None` = no hotkey (the slot cannot be triggered).
    pub hotkey: Option<Hotkey>,
    #[serde(rename = "type")]
    pub kind: MacroType,
    pub content: MacroContent,
    /// D-18: `\n` and `\t` may be part of the text. Off by default.
    pub allow_enter_tab: bool,
}

impl Default for MacroSlot {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: false,
            hotkey: None,
            kind: MacroType::Text,
            content: MacroContent::default(),
            allow_enter_tab: false,
        }
    }
}

/// Why a slot cannot be saved. Codes on the UI boundary (`MacroSlotError` in `api.ts`).
pub fn validate(slot: &MacroSlot) -> Result<(), &'static str> {
    let name_ok =
        slot.name.chars().count() <= MAX_NAME_CHARS && !slot.name.chars().any(char::is_control);
    if !name_ok {
        return Err("name_invalid");
    }
    if slot.kind != MacroType::Text {
        return Err("type_unsupported");
    }
    ks_engine::check_content(slot.content.as_str(), slot.permissions()).map_err(|e| e.code())?;
    if let Some(hotkey) = &slot.hotkey {
        hotkey.to_chord().map_err(|e| match e {
            crate::hotkey::Invalid::Invalid => "invalid",
            crate::hotkey::Invalid::Reserved => "reserved",
        })?;
    }
    Ok(())
}

impl MacroSlot {
    pub fn permissions(&self) -> MacroPermissions {
        MacroPermissions {
            allow_enter_tab: self.allow_enter_tab,
        }
    }

    /// The hotkey to register, if the slot is enabled and has one.
    pub fn active_hotkey(&self) -> Option<&Hotkey> {
        self.hotkey.as_ref().filter(|_| self.enabled)
    }

    /// The macro for the engine, if the slot is enabled. The engine re-checks the content.
    pub fn to_macro(&self) -> Option<Macro> {
        (self.enabled && self.kind == MacroType::Text).then(|| Macro {
            body: MacroBody::Text(self.content.to_user_text()),
            permissions: self.permissions(),
        })
    }

    /// A slot loaded from an untrusted file: whatever would not pass [`validate`] today is
    /// switched off or cleared; the content itself is kept (it is the user's data).
    pub fn sanitized(mut self) -> Self {
        if self.hotkey.as_ref().is_some_and(|h| h.to_chord().is_err()) {
            self.hotkey = None;
        }
        if self.name.chars().count() > MAX_NAME_CHARS || self.name.chars().any(char::is_control) {
            self.name = String::new();
        }
        if validate(&self).is_err() {
            self.enabled = false;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::Modifier;

    fn slot(content: &str) -> MacroSlot {
        MacroSlot {
            name: "Greeting".into(),
            enabled: true,
            hotkey: Some(Hotkey {
                modifiers: vec![Modifier::Ctrl, Modifier::Alt],
                key: "M".into(),
            }),
            content: MacroContent(content.into()),
            ..MacroSlot::default()
        }
    }

    #[test]
    fn validation_codes() {
        assert_eq!(validate(&slot("Hello")), Ok(()));
        assert_eq!(
            validate(&slot("a\nb")),
            Err("content_enter_tab_not_permitted")
        );
        let mut s = slot("a\nb");
        s.allow_enter_tab = true;
        assert_eq!(validate(&s), Ok(()));
        assert_eq!(validate(&slot("a\u{7}")), Err("content_forbidden_chars"));
        assert_eq!(
            validate(&slot(&"x".repeat(ks_engine::MACRO_MAX_CHARS + 1))),
            Err("content_too_long")
        );
        let mut s = slot("x");
        s.name = "a\u{0}".into();
        assert_eq!(validate(&s), Err("name_invalid"));
        let mut s = slot("x");
        s.hotkey = Some(Hotkey {
            modifiers: vec![Modifier::Ctrl],
            key: "C".into(),
        });
        assert_eq!(validate(&s), Err("reserved"));
    }

    #[test]
    fn json_shape_and_redaction() {
        let s = slot("secret-ish");
        let json = serde_json::to_string(&s).expect("json");
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"allowEnterTab\":false"));
        let back: MacroSlot = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, s);
        assert!(!format!("{s:?}").contains("secret-ish"));
    }

    #[test]
    fn untrusted_slot_is_switched_off_not_lost() {
        let parsed: MacroSlot = serde_json::from_str(
            r#"{"name":"x","enabled":true,"type":"python","content":"print(1)"}"#,
        )
        .expect("parse");
        assert_eq!(parsed.kind, MacroType::Unsupported);
        let s = parsed.sanitized();
        assert!(!s.enabled);
        assert!(s.to_macro().is_none());
        let mut s = slot("a\nb").sanitized();
        assert!(!s.enabled);
        s.allow_enter_tab = true;
        s.enabled = true;
        assert!(s.sanitized().to_macro().is_some());
    }
}
