//! Hotkey model shared with the UI (`app/src/hotkey.ts`) and its translation to
//! `RegisterHotKey` arguments. Rule D-14: one or two modifiers plus exactly one regular key.

use serde::{Deserialize, Serialize};

/// Modifier names as the UI sends them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Win,
}

impl Modifier {
    /// `MOD_*` bit of `RegisterHotKey`.
    fn bit(self) -> u32 {
        match self {
            Modifier::Alt => 0x1,
            Modifier::Ctrl => 0x2,
            Modifier::Shift => 0x4,
            Modifier::Win => 0x8,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Modifier::Ctrl => "Ctrl",
            Modifier::Shift => "Shift",
            Modifier::Alt => "Alt",
            Modifier::Win => "Win",
        }
    }
}

/// `key` is a layout-independent key name derived from `KeyboardEvent.code`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

impl Default for Hotkey {
    fn default() -> Self {
        Self {
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            key: "Space".into(),
        }
    }
}

/// `RegisterHotKey` arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub mods: u32,
    pub vk: u32,
}

/// Why a hotkey is not acceptable (`HotkeyError` on the UI boundary, minus `conflict`,
/// which only the OS can report).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    Invalid,
    Reserved,
}

const NAMED_KEYS: [(&str, u32); 31] = [
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Tab", 0x09),
    ("Escape", 0x1B),
    ("Backspace", 0x08),
    ("Delete", 0x2E),
    ("Insert", 0x2D),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("ArrowLeft", 0x25),
    ("ArrowUp", 0x26),
    ("ArrowRight", 0x27),
    ("ArrowDown", 0x28),
    ("Backquote", 0xC0),
    ("Minus", 0xBD),
    ("Equal", 0xBB),
    ("BracketLeft", 0xDB),
    ("BracketRight", 0xDD),
    ("Backslash", 0xDC),
    ("Semicolon", 0xBA),
    ("Quote", 0xDE),
    ("Comma", 0xBC),
    ("Period", 0xBE),
    ("Slash", 0xBF),
    ("Pause", 0x13),
    ("CapsLock", 0x14),
    ("ScrollLock", 0x91),
    ("NumLock", 0x90),
    ("PrintScreen", 0x2C),
];

/// Combinations owned by Windows or too common to take away from every application.
const RESERVED: [&str; 16] = [
    "Ctrl+Alt+Delete",
    "Ctrl+Shift+Escape",
    "Ctrl+Escape",
    "Alt+Tab",
    "Alt+Escape",
    "Alt+F4",
    "Alt+Space",
    "Win+L",
    "Win+D",
    "Ctrl+C",
    "Ctrl+V",
    "Ctrl+X",
    "Ctrl+Z",
    "Ctrl+Y",
    "Ctrl+A",
    "Ctrl+S",
];

fn virtual_key(key: &str) -> Option<u32> {
    let bytes = key.as_bytes();
    if let [b] = bytes
        && (b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return Some(u32::from(*b));
    }
    if let Some(n) = key.strip_prefix('F').and_then(|n| n.parse::<u32>().ok())
        && (1..=24).contains(&n)
        && !key.starts_with("F0")
    {
        return Some(0x70 + n - 1);
    }
    NAMED_KEYS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, vk)| *vk)
}

impl Hotkey {
    /// "Ctrl+Shift+Space" — for the tray tooltip.
    pub fn label(&self) -> String {
        self.modifiers
            .iter()
            .map(|m| m.label())
            .chain(std::iter::once(self.key.as_str()))
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Validates the hotkey and returns the `RegisterHotKey` arguments.
    pub fn to_chord(&self) -> Result<Chord, Invalid> {
        let count = self.modifiers.len();
        if !(1..=2).contains(&count) || (count == 2 && self.modifiers[0] == self.modifiers[1]) {
            return Err(Invalid::Invalid);
        }
        let vk = virtual_key(&self.key).ok_or(Invalid::Invalid)?;
        // Shift alone plus a key is ordinary typing.
        if self.modifiers == [Modifier::Shift] || RESERVED.contains(&self.label().as_str()) {
            return Err(Invalid::Reserved);
        }
        Ok(Chord {
            mods: self.modifiers.iter().fold(0, |acc, m| acc | m.bit()),
            vk,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(mods: &[Modifier], key: &str) -> Hotkey {
        Hotkey {
            modifiers: mods.to_vec(),
            key: key.into(),
        }
    }

    #[test]
    fn default_is_ctrl_shift_space() {
        let d = Hotkey::default();
        assert_eq!(d.label(), "Ctrl+Shift+Space");
        assert_eq!(
            d.to_chord(),
            Ok(Chord {
                mods: 0x6,
                vk: 0x20
            })
        );
    }

    #[test]
    fn keys_map_to_virtual_keys() {
        use Modifier::*;
        assert_eq!(hk(&[Ctrl, Alt], "K").to_chord().map(|c| c.vk), Ok(0x4B));
        assert_eq!(hk(&[Alt], "1").to_chord().map(|c| c.vk), Ok(0x31));
        assert_eq!(
            hk(&[Win], "F9").to_chord(),
            Ok(Chord {
                mods: 0x8,
                vk: 0x78
            })
        );
        assert_eq!(hk(&[Ctrl], "F24").to_chord().map(|c| c.vk), Ok(0x87));
        assert_eq!(hk(&[Ctrl], "Backquote").to_chord().map(|c| c.vk), Ok(0xC0));
    }

    #[test]
    fn invalid_and_reserved_are_rejected() {
        use Modifier::*;
        for bad in [
            hk(&[], "K"),
            hk(&[Ctrl, Shift, Alt], "K"),
            hk(&[Ctrl, Ctrl], "K"),
            hk(&[Ctrl], ""),
            hk(&[Ctrl], "k"),
            hk(&[Ctrl], "F25"),
            hk(&[Ctrl], "F01"),
            hk(&[Ctrl], "ShiftLeft"),
        ] {
            assert_eq!(bad.to_chord(), Err(Invalid::Invalid), "{}", bad.label());
        }
        for reserved in [
            hk(&[Ctrl, Alt], "Delete"),
            hk(&[Alt], "Tab"),
            hk(&[Alt], "F4"),
            hk(&[Win], "L"),
            hk(&[Ctrl], "C"),
            hk(&[Shift], "K"),
        ] {
            assert_eq!(
                reserved.to_chord(),
                Err(Invalid::Reserved),
                "{}",
                reserved.label()
            );
        }
    }
}
