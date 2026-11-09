//! Keyboard layouts as data: physical key (scan code + shift) <-> character.

use std::collections::HashMap;
use std::fmt;

/// Scan code (PC set 1) of a physical key.
pub type ScanCode = u16;

/// One physical key press, with or without Shift.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyStroke {
    pub scan: ScanCode,
    pub shift: bool,
}

/// A keyboard layout loaded from a `.layout` data file.
#[derive(Debug, Clone)]
pub struct Layout {
    id: String,
    to_char: HashMap<KeyStroke, char>,
    to_key: HashMap<char, KeyStroke>,
}

/// Error while parsing a layout file. Contains only layout data, never user text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutError {
    pub line: usize,
    pub reason: &'static str,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for LayoutError {}

impl Layout {
    /// Parses the line format `<0xSC> <base> <shift>`; `//` starts a comment line.
    pub fn parse(id: &str, src: &str) -> Result<Self, LayoutError> {
        let mut to_char = HashMap::new();
        let mut to_key = HashMap::new();
        for (idx, raw) in src.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            let err = |reason| LayoutError {
                line: line_no,
                reason,
            };
            let mut parts = line.split_whitespace();
            let sc = parts.next().ok_or(err("missing scan code"))?;
            let sc = sc
                .strip_prefix("0x")
                .and_then(|h| ScanCode::from_str_radix(h, 16).ok())
                .ok_or(err("bad scan code"))?;
            for shift in [false, true] {
                let tok = parts.next().ok_or(err("missing character"))?;
                let mut chars = tok.chars();
                let ch = chars.next().ok_or(err("missing character"))?;
                if chars.next().is_some() {
                    return Err(err("expected a single character"));
                }
                let key = KeyStroke { scan: sc, shift };
                if to_char.insert(key, ch).is_some() {
                    return Err(err("duplicate key"));
                }
                if to_key.insert(ch, key).is_some() {
                    return Err(err("duplicate character"));
                }
            }
            if parts.next().is_some() {
                return Err(err("trailing data"));
            }
        }
        Ok(Self {
            id: id.to_owned(),
            to_char,
            to_key,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Physical key that produces `ch` in this layout.
    pub fn key_of(&self, ch: char) -> Option<KeyStroke> {
        self.to_key.get(&ch).copied()
    }

    /// Character produced by `key` in this layout.
    pub fn char_of(&self, key: KeyStroke) -> Option<char> {
        self.to_char.get(&key).copied()
    }

    /// Number of mapped key strokes.
    pub fn len(&self) -> usize {
        self.to_char.len()
    }

    pub fn is_empty(&self) -> bool {
        self.to_char.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rejects() {
        let l = Layout::parse("t", "// c\n0x10 q Q\n").unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(
            l.key_of('Q'),
            Some(KeyStroke {
                scan: 0x10,
                shift: true
            })
        );
        assert_eq!(Layout::parse("t", "0x10 q").unwrap_err().line, 1);
        assert!(Layout::parse("t", "0x10 q Q\n0x11 q W").is_err());
        assert!(Layout::parse("t", "16 q Q").is_err());
        assert!(Layout::parse("t", "0x10 qq Q").is_err());
    }
}
