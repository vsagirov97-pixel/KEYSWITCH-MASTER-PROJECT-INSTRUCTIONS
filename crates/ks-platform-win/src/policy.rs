//! Platform-independent decisions used by the probe (and candidates for `ks-platform-win`):
//! which characters W3 refuses (WS-06), which window styles mark a password or read-only
//! edit (WS-03, D-30) and which window classes allow W1 (D-30). Pure functions, unit-tested
//! on every host.

/// `ES_PASSWORD` (winuser.h). Same bit for `Edit` and RichEdit classes.
pub const ES_PASSWORD: u32 = 0x0020;
/// `ES_READONLY` (winuser.h).
pub const ES_READONLY: u32 = 0x0800;
/// `ES_MULTILINE` (winuser.h).
pub const ES_MULTILINE: u32 = 0x0004;

/// Why a character makes W3 refuse the whole text (nothing is sent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharClass {
    /// General category Cc (C0/C1 controls: `\n`, `\r`, `\t`, ...).
    Control,
    /// General category Cf (format: bidi overrides/isolates, ZW*, BOM, soft hyphen, tags...).
    Format,
    /// U+2028 LINE SEPARATOR (Zl) or U+2029 PARAGRAPH SEPARATOR (Zp).
    Separator,
}

/// `Some(class)` if `c` must not be injected by W3 (WS-06: Cc, Cf, Zl, Zp).
///
/// Note: this also refuses ZWJ/ZWNJ (U+200D/U+200C, category Cf), i.e. emoji ZWJ
/// sequences and some Persian/Indic text go to another method or a refusal. For EN<->RU
/// conversion this is acceptable; the product decision is recorded in the S-2b report.
pub fn forbidden_char(c: char) -> Option<CharClass> {
    let u = c as u32;
    if c.is_control() {
        return Some(CharClass::Control);
    }
    if u == 0x2028 || u == 0x2029 {
        return Some(CharClass::Separator);
    }
    // One table for the write policy and the macro content policy (`check_content`).
    if ks_engine::is_format_char(c) {
        return Some(CharClass::Format);
    }
    None
}

/// First forbidden character class in `text`, if any.
pub fn first_forbidden(text: &str) -> Option<CharClass> {
    text.chars().find_map(forbidden_char)
}

/// `true` if `class` (from `GetClassNameW` / `RealGetWindowClassW`) is a classic edit or a
/// RichEdit class, i.e. a class for which the `ES_*` style bits have their edit meaning.
pub fn is_edit_family(class: &str) -> bool {
    let c = class.to_ascii_lowercase();
    c == "edit" || c.starts_with("richedit")
}

/// WS-03: the window style marks a password field. Only meaningful for the edit family;
/// for other classes bit 0x20 means something else and this returns `false` (the UIA
/// `IsPassword` check still applies).
pub fn style_says_password(class: &str, style: u32) -> bool {
    is_edit_family(class) && style & ES_PASSWORD != 0
}

/// Why W1 (`EM_REPLACESEL`) is not permitted for a window (D-30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum W1Refusal {
    /// `GetClassNameW` or `RealGetWindowClassW` is not exactly `Edit` (case-insensitive).
    NotEditClass,
    /// The edit has `ES_PASSWORD`.
    Password,
    /// The edit has `ES_READONLY`.
    ReadOnly,
}

/// D-30: W1 only if both class names are exactly `Edit` (case-insensitive) and the style
/// has neither `ES_PASSWORD` nor `ES_READONLY`.
pub fn w1_permitted(class: &str, real_class: &str, style: u32) -> Result<(), W1Refusal> {
    if !class.eq_ignore_ascii_case("edit") || !real_class.eq_ignore_ascii_case("edit") {
        return Err(W1Refusal::NotEditClass);
    }
    if style & ES_PASSWORD != 0 {
        return Err(W1Refusal::Password);
    }
    if style & ES_READONLY != 0 {
        return Err(W1Refusal::ReadOnly);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws06_rejects_cc_cf_zl_zp() {
        for c in ['\n', '\r', '\t', '\u{0}', '\u{7f}', '\u{85}'] {
            assert_eq!(forbidden_char(c), Some(CharClass::Control));
        }
        assert_eq!(forbidden_char('\u{2028}'), Some(CharClass::Separator));
        assert_eq!(forbidden_char('\u{2029}'), Some(CharClass::Separator));
        for u in [
            0x202A, 0x202B, 0x202C, 0x202D, 0x202E, 0x2066, 0x2067, 0x2068, 0x2069, 0x200B, 0x200E,
            0x200F, 0xFEFF, 0x00AD, 0x061C, 0xE0041,
        ] {
            let c = char::from_u32(u).expect("valid scalar");
            assert_eq!(forbidden_char(c), Some(CharClass::Format), "U+{u:04X}");
        }
    }

    #[test]
    fn ws06_accepts_ordinary_text() {
        assert_eq!(first_forbidden("Ghbdtn, Привет! 123 \u{1F600} ё Ё"), None);
        assert_eq!(first_forbidden("ab\u{202E}cd"), Some(CharClass::Format));
    }

    #[test]
    fn ws03_password_style_only_for_edit_family() {
        assert!(style_says_password("Edit", 0x5001_00A0));
        assert!(style_says_password("RICHEDIT50W", ES_PASSWORD));
        assert!(!style_says_password("Edit", 0x5001_0080));
        assert!(!style_says_password("Button", ES_PASSWORD));
    }

    #[test]
    fn d30_w1_only_plain_edit() {
        assert_eq!(w1_permitted("Edit", "Edit", 0), Ok(()));
        assert_eq!(w1_permitted("EDIT", "edit", 0), Ok(()));
        assert_eq!(
            w1_permitted("RICHEDIT50W", "RICHEDIT50W", 0),
            Err(W1Refusal::NotEditClass)
        );
        // A superclassed edit: class name differs although RealGetWindowClass says Edit.
        assert_eq!(
            w1_permitted("MyEdit", "Edit", 0),
            Err(W1Refusal::NotEditClass)
        );
        assert_eq!(
            w1_permitted("Edit", "Edit", ES_PASSWORD),
            Err(W1Refusal::Password)
        );
        assert_eq!(
            w1_permitted("Edit", "Edit", ES_READONLY),
            Err(W1Refusal::ReadOnly)
        );
    }
}
