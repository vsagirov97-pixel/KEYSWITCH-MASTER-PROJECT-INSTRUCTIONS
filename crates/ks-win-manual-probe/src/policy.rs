//! Platform-independent rules of the manual kit (unit-tested on every host):
//! argument parsing, the text policy for synthetic input, the clipboard restore whitelist,
//! layout classification and the metadata-only report format.

use std::fmt::Write as _;

/// What the owner asked the tool to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Scenarios on the tool's own window only.
    Fixture,
    /// Read-only metadata of the focused element in real applications.
    Survey,
}

impl Mode {
    /// Stable label for the report.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Fixture => "fixture",
            Mode::Survey => "survey",
        }
    }
}

/// Why the command line was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgError {
    /// `--i-understand` is missing.
    NotAcknowledged,
    /// No mode or more than one mode.
    BadMode,
    /// Unknown argument.
    Unknown,
    /// `--help` was requested.
    Help,
}

/// Parses `--i-understand fixture|survey` (order-independent).
pub fn parse_args(args: &[String]) -> Result<Mode, ArgError> {
    let mut ack = false;
    let mut mode = None;
    for a in args {
        match a.as_str() {
            "--help" | "-h" | "/?" => return Err(ArgError::Help),
            "--i-understand" => ack = true,
            "fixture" | "survey" => {
                if mode.is_some() {
                    return Err(ArgError::BadMode);
                }
                mode = Some(if a == "fixture" {
                    Mode::Fixture
                } else {
                    Mode::Survey
                });
            }
            _ => return Err(ArgError::Unknown),
        }
    }
    if !ack {
        return Err(ArgError::NotAcknowledged);
    }
    mode.ok_or(ArgError::BadMode)
}

/// Characters the synthetic-input writer refuses before generating any input (WS-06):
/// C0/C1 controls (Enter, Tab, ...), line/paragraph separators (Zl, Zp) and format
/// characters (Cf: bidi overrides/isolates, zero-width, BOM, tags, ...).
pub fn is_forbidden_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c as u32,
            0x00AD
                | 0x0600..=0x0605
                | 0x061C
                | 0x06DD
                | 0x070F
                | 0x0890..=0x0891
                | 0x08E2
                | 0x180E
                | 0x200B..=0x200F
                | 0x2028..=0x202E
                | 0x2060..=0x2064
                | 0x2066..=0x206F
                | 0xFEFF
                | 0xFFF9..=0xFFFB
                | 0x110BD
                | 0x110CD
                | 0x13430..=0x1343F
                | 0x1BCA0..=0x1BCA3
                | 0x1D173..=0x1D17A
                | 0xE0001
                | 0xE0020..=0xE007F
        )
}

/// `true` if `text` is non-empty and contains no forbidden character.
pub fn is_writable_text(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(is_forbidden_char)
}

/// Standard clipboard format ids used by the whitelist.
pub mod cf {
    /// CF_TEXT.
    pub const TEXT: u32 = 1;
    /// CF_OEMTEXT.
    pub const OEMTEXT: u32 = 7;
    /// CF_UNICODETEXT.
    pub const UNICODETEXT: u32 = 13;
    /// CF_LOCALE.
    pub const LOCALE: u32 = 16;
}

/// `true` if a clipboard with these formats can be restored exactly by the kit: only plain
/// text (Unicode, plus the formats Windows synthesises from it) and the three D-13 privacy
/// markers. Anything else (images, files, HTML, RTF, app-private formats) -> not restorable,
/// so clipboard scenarios are skipped and the owner's clipboard is never touched.
pub fn clipboard_restorable(formats: &[u32], privacy: &[u32; 3]) -> bool {
    if privacy.contains(&0) {
        return false;
    }
    let allowed = |f: u32| {
        matches!(f, cf::TEXT | cf::OEMTEXT | cf::UNICODETEXT | cf::LOCALE) || privacy.contains(&f)
    };
    if !formats.iter().all(|&f| allowed(f)) {
        return false;
    }
    let has_text = formats
        .iter()
        .any(|&f| matches!(f, cf::TEXT | cf::OEMTEXT | cf::LOCALE));
    // ANSI-only text cannot be restored losslessly from CF_UNICODETEXT.
    !has_text || formats.contains(&cf::UNICODETEXT)
}

/// Layout family of a keyboard layout language id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutKind {
    /// Primary language English.
    En,
    /// Primary language Russian.
    Ru,
    /// Anything else.
    Other,
}

/// Classifies by primary language (low 10 bits of LANGID).
pub fn layout_kind(lang_id: u16) -> LayoutKind {
    match lang_id & 0x3FF {
        0x09 => LayoutKind::En,
        0x19 => LayoutKind::Ru,
        _ => LayoutKind::Other,
    }
}

/// Exact, case-insensitive match with the classic Win32 `Edit` class (D-30 condition).
pub fn is_edit_class(class: &str) -> bool {
    class.eq_ignore_ascii_case("Edit")
}

/// Classes whose `ES_PASSWORD` style bit means "password field".
pub fn is_edit_family_class(class: &str) -> bool {
    let c = class.to_ascii_lowercase();
    c == "edit" || c.starts_with("richedit")
}

/// File name of an executable path (`C:\x\chrome.exe` -> `chrome.exe`); directories are
/// dropped so the report never contains user folder names.
pub fn exe_file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Label for a mandatory-integrity RID.
pub fn integrity_label(rid: u32) -> &'static str {
    match rid {
        0x0000 => "untrusted",
        0x1000 => "low",
        0x2000 => "medium",
        0x2100 => "medium_plus",
        0x3000 => "high",
        0x4000 => "system",
        0x5000 => "protected",
        _ => "other",
    }
}

/// Metadata-only report. Values are sanitised and length-capped; callers must never pass
/// text read from a field or the clipboard (only lengths, flags, class/exe names, codes).
#[derive(Debug, Default)]
pub struct Report {
    lines: Vec<String>,
}

/// Cap for one value (class names etc. are short; this only guards against surprises).
pub const MAX_VALUE_CHARS: usize = 120;

/// Makes a value safe for a single `key=value` line.
pub fn sanitize(v: &str) -> String {
    let mut out = String::new();
    for (n, c) in v.chars().enumerate() {
        if n >= MAX_VALUE_CHARS {
            out.push('~');
            break;
        }
        out.push(if c.is_control() || c == '"' { '_' } else { c });
    }
    out
}

impl Report {
    /// Adds a free-form header/comment line written by the tool itself.
    pub fn note(&mut self, s: &str) {
        self.lines.push(sanitize_line(s));
    }

    /// Adds one result line: `id STATUS key="value" ...`.
    pub fn line(&mut self, id: &str, status: &str, fields: &[(&str, String)]) {
        let mut s = format!("{id} {status}");
        for (k, v) in fields {
            let _ = write!(s, " {k}=\"{}\"", sanitize(v));
        }
        self.lines.push(s);
    }

    /// All lines joined with CRLF (Notepad-friendly).
    pub fn render(&self) -> String {
        let mut s = String::new();
        for l in &self.lines {
            s.push_str(l);
            s.push_str("\r\n");
        }
        s
    }

    /// Number of lines so far.
    pub fn len(&self) -> usize {
        self.lines.len()
    }
}

fn sanitize_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn args_require_acknowledgement() {
        assert_eq!(parse_args(&a(&["fixture"])), Err(ArgError::NotAcknowledged));
        assert_eq!(parse_args(&a(&[])), Err(ArgError::NotAcknowledged));
        assert_eq!(
            parse_args(&a(&["--i-understand", "fixture"])),
            Ok(Mode::Fixture)
        );
        assert_eq!(
            parse_args(&a(&["survey", "--i-understand"])),
            Ok(Mode::Survey)
        );
        assert_eq!(parse_args(&a(&["--i-understand"])), Err(ArgError::BadMode));
        assert_eq!(
            parse_args(&a(&["--i-understand", "fixture", "survey"])),
            Err(ArgError::BadMode)
        );
        assert_eq!(
            parse_args(&a(&["--i-understand", "fixture", "--x"])),
            Err(ArgError::Unknown)
        );
    }

    #[test]
    fn text_policy_rejects_controls_separators_and_format_chars() {
        for bad in [
            "a\nb",
            "a\rb",
            "\tb",
            "a\u{2028}",
            "a\u{2029}",
            "\u{202E}x",
            "x\u{2066}",
            "\u{200B}",
            "\u{FEFF}",
            "\u{0085}",
            "\u{E0041}",
        ] {
            assert!(
                !is_writable_text(bad),
                "{:?} must be refused",
                bad.escape_unicode().to_string()
            );
        }
        assert!(!is_writable_text(""));
        assert!(is_writable_text("Hello Привет 123 \u{1F600}"));
    }

    #[test]
    fn clipboard_whitelist() {
        let p = [0xC001, 0xC002, 0xC003];
        assert!(clipboard_restorable(&[], &p));
        assert!(clipboard_restorable(&[13, 16, 1, 7], &p));
        assert!(clipboard_restorable(&[13, 0xC001, 0xC002], &p));
        assert!(!clipboard_restorable(&[1, 7, 16], &p), "ANSI-only text");
        assert!(!clipboard_restorable(&[13, 2], &p), "bitmap");
        assert!(!clipboard_restorable(&[13, 0xC0FF], &p), "HTML/RTF/private");
        assert!(!clipboard_restorable(&[13], &[0, 0xC002, 0xC003]));
    }

    #[test]
    fn layouts_and_names() {
        assert_eq!(layout_kind(0x0409), LayoutKind::En);
        assert_eq!(layout_kind(0x0809), LayoutKind::En);
        assert_eq!(layout_kind(0x0419), LayoutKind::Ru);
        assert_eq!(layout_kind(0x0426), LayoutKind::Other);
        assert!(is_edit_class("EDIT") && is_edit_class("Edit"));
        assert!(!is_edit_class("RICHEDIT50W") && !is_edit_class("EditX"));
        assert!(is_edit_family_class("RichEdit20W") && is_edit_family_class("edit"));
        assert_eq!(
            exe_file_name(r"C:\Users\x\AppData\Telegram.exe"),
            "Telegram.exe"
        );
        assert_eq!(exe_file_name("notepad.exe"), "notepad.exe");
        assert_eq!(integrity_label(0x2000), "medium");
        assert_eq!(integrity_label(0x3000), "high");
    }

    #[test]
    fn report_sanitises_values() {
        let mut r = Report::default();
        r.line("x1", "PASS", &[("k", "a\"b\nc".to_string())]);
        r.line("x2", "PASS", &[("k", "y".repeat(500))]);
        let s = r.render();
        assert!(s.starts_with("x1 PASS k=\"a_b_c\"\r\n"));
        assert!(
            s.lines()
                .nth(1)
                .is_some_and(|l| l.len() < MAX_VALUE_CHARS + 20)
        );
        assert_eq!(r.len(), 2);
    }
}
