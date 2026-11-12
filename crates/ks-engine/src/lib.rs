//! KeySwitch operation pipeline (Beta 1: conversion only).
//!
//! `hotkey → gate → read → detect/convert → write → post-check → switch layout`.
//!
//! This crate has no OS dependencies and no `unsafe`: everything system-specific sits behind
//! the [`Platform`] trait (implemented by `ks-platform-win`). Results are codes only — no type
//! returned from [`run`] can hold user text; text travels exclusively as [`UserText`].

mod user_text;

pub use user_text::{UserText, wipe};

use ks_core::{Confidence, Converter, Detection};

/// Why an operation was refused before anything was changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Password field (UIA `IsPassword` or `ES_PASSWORD`).
    Password,
    /// The target runs at a higher integrity level (or it could not be determined).
    Elevated,
    /// Secure desktop / no access to the input desktop.
    SecureDesktop,
    /// The focused window belongs to KeySwitch itself.
    SelfTarget,
    /// Console window.
    Console,
    /// The target executable is on the excluded-apps list.
    ExcludedApp,
    /// Read-only field.
    ReadOnly,
    /// The text is longer than the limit.
    TooLong,
    /// The converter is not confident; without a language popup the engine refuses.
    LowConfidence,
    /// Letters of several languages; needs the user's choice (popup not in Beta 1).
    Mixed,
    /// No usable text interface in the focused control.
    Unsupported,
    /// The text contains characters the write method must not inject (`\n`, `\t`, bidi...).
    ForbiddenChars,
}

/// Why a started operation stopped. Nothing is written after a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Focus, foreground window or process changed, or a newer operation started.
    TargetChanged,
    /// A modifier key stayed down for too long.
    ModifiersHeld,
    /// Reading from the target failed.
    Read,
    /// The write call failed or was blocked.
    Write,
}

/// Early exit from a pipeline step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Nothing was changed, by policy.
    Refused(Refusal),
    /// A step failed.
    Failed(Failure),
}

/// Result of one operation. Metadata only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Text was replaced.
    Converted {
        /// The post-check re-read the target and found the expected text.
        verified: bool,
        /// The keyboard layout switch was requested successfully (or was already right).
        layout_switched: bool,
    },
    /// Nothing to convert (no selection and no word before the caret, no letters, no change).
    Nothing,
    /// Refused by policy.
    Refused(Refusal),
    /// Stopped by a failure.
    Failed(Failure),
}

impl Outcome {
    /// Stable code for diagnostics and the UI boundary.
    pub fn code(self) -> &'static str {
        match self {
            Outcome::Converted { verified: true, .. } => "CONVERTED",
            Outcome::Converted {
                verified: false, ..
            } => "CONVERTED_UNVERIFIED",
            Outcome::Nothing => "NOTHING",
            Outcome::Refused(r) => match r {
                Refusal::Password => "PASSWORD",
                Refusal::Elevated => "ELEVATED",
                Refusal::SecureDesktop => "SECURE_DESKTOP",
                Refusal::SelfTarget => "SELF_TARGET",
                Refusal::Console => "CONSOLE",
                Refusal::ExcludedApp => "EXCLUDED_APP",
                Refusal::ReadOnly => "READ_ONLY",
                Refusal::TooLong => "TOO_LONG",
                Refusal::LowConfidence => "LOW_CONFIDENCE",
                Refusal::Mixed => "MIXED",
                Refusal::Unsupported => "UNSUPPORTED",
                Refusal::ForbiddenChars => "FORBIDDEN_CHARS",
            },
            Outcome::Failed(f) => match f {
                Failure::TargetChanged => "TARGET_CHANGED",
                Failure::ModifiersHeld => "MODIFIERS_HELD",
                Failure::Read => "READ_FAILED",
                Failure::Write => "WRITE_FAILED",
            },
        }
    }
}

/// Facts about the captured target that policy needs. No text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetInfo {
    /// Lower-cased executable file name (`notepad.exe`); empty if unknown.
    pub exe: String,
}

/// System access for one operation. The implementation keeps the captured target and
/// re-verifies it before every write.
pub trait Platform {
    /// Captures the target and applies the OS-level refusals (secure desktop, self target,
    /// elevated, console, password, read-only, no text interface).
    fn begin(&mut self) -> Result<TargetInfo, Stop>;
    /// Text of the single selection, reading at most `max_units` UTF-16 units.
    /// `None` = no selection (caret only).
    fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop>;
    /// At most `max_units` units immediately before the caret. `None` = no caret.
    fn read_before_caret(&mut self, max_units: usize) -> Result<Option<UserText>, Stop>;
    /// Selects the last `units` units before the caret.
    fn select_before_caret(&mut self, units: usize) -> Result<(), Stop>;
    /// Replaces the current selection with `text`.
    fn write(&mut self, text: &UserText) -> Result<(), Stop>;
    /// `true` if the text right before the caret now equals `expected`.
    fn verify(&mut self, expected: &UserText) -> bool;
    /// Asks the target to switch to the keyboard layout of language `lang` (`"ru"`, `"en"`).
    fn switch_layout(&mut self, lang: &str) -> bool;
}

/// Policy knobs of one operation.
#[derive(Clone, Debug)]
pub struct Config {
    /// Lower-cased executable names that must never be touched.
    pub excluded: Vec<String>,
    /// Maximum length of the text to convert, UTF-16 units.
    pub max_units: usize,
    /// How far back from the caret the engine looks for the last word, UTF-16 units.
    pub caret_window: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            excluded: Vec::new(),
            max_units: 200,
            caret_window: 64,
        }
    }
}

/// Validates an executable name for the excluded list and returns its canonical form
/// (trimmed, lower-cased). Only a bare file name ending in `.exe` is accepted.
pub fn normalize_exe(name: &str) -> Option<String> {
    let n = name.trim().to_lowercase();
    let ok = n.len() > 4
        && n.len() <= 128
        && n.ends_with(".exe")
        && n.chars().all(|c| {
            !c.is_control() && !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        });
    ok.then_some(n)
}

/// The fragment before the caret that a caret-only operation converts: the last run of
/// non-whitespace characters plus the spaces typed after it.
enum Fragment {
    None,
    /// The word fills the whole look-back window: its start is unknown, so refuse.
    Unbounded,
    Found(UserText),
}

fn tail_fragment(before: &UserText, window_full: bool) -> Fragment {
    let s = before.expose();
    let word_end = s.trim_end_matches(' ').len();
    let word_start = s[..word_end]
        .char_indices()
        .rev()
        .take_while(|(_, c)| !c.is_whitespace())
        .last()
        .map_or(word_end, |(i, _)| i);
    if word_start == word_end {
        return Fragment::None;
    }
    if word_start == 0 && window_full {
        return Fragment::Unbounded;
    }
    Fragment::Found(UserText::new(s[word_start..].to_owned()))
}

/// Runs one conversion operation. Never panics on user data and never returns text.
pub fn run<P: Platform>(platform: &mut P, converter: &Converter, config: &Config) -> Outcome {
    match run_inner(platform, converter, config) {
        Ok(outcome) => outcome,
        Err(Stop::Refused(r)) => Outcome::Refused(r),
        Err(Stop::Failed(f)) => Outcome::Failed(f),
    }
}

fn run_inner<P: Platform>(
    platform: &mut P,
    converter: &Converter,
    config: &Config,
) -> Result<Outcome, Stop> {
    let target = platform.begin()?;
    if !target.exe.is_empty() && config.excluded.contains(&target.exe) {
        return Err(Stop::Refused(Refusal::ExcludedApp));
    }

    // One unit more than the limit, to tell "exactly at the limit" from "longer".
    let selection = platform
        .read_selection(config.max_units + 1)?
        .filter(|t| !t.is_empty());
    let (source, by_caret) = match selection {
        Some(text) => (text, false),
        None => {
            let Some(before) = platform.read_before_caret(config.caret_window)? else {
                return Ok(Outcome::Nothing);
            };
            let full = before.utf16_len() >= config.caret_window;
            match tail_fragment(&before, full) {
                Fragment::None => return Ok(Outcome::Nothing),
                Fragment::Unbounded => return Err(Stop::Refused(Refusal::TooLong)),
                Fragment::Found(text) => (text, true),
            }
        }
    };
    if source.utf16_len() > config.max_units {
        return Err(Stop::Refused(Refusal::TooLong));
    }

    let from = match converter.detect(source.expose()) {
        Detection::NoLetters => return Ok(Outcome::Nothing),
        Detection::Mixed(_) => return Err(Stop::Refused(Refusal::Mixed)),
        Detection::Single(lang) => lang,
    };
    let Some(to) = converter
        .languages()
        .find(|l| *l != from)
        .map(str::to_owned)
    else {
        return Ok(Outcome::Nothing);
    };
    let Ok(conversion) = converter.convert(source.expose(), &from, &to) else {
        return Ok(Outcome::Nothing);
    };
    let confidence = conversion.confidence;
    let converted = UserText::new(conversion.text);
    if confidence == Confidence::Low {
        return Err(Stop::Refused(Refusal::LowConfidence));
    }
    if converted.same_as(&source) {
        return Ok(Outcome::Nothing);
    }

    if by_caret {
        platform.select_before_caret(source.utf16_len())?;
        // The selection we just made must be exactly the fragment we converted.
        let now = platform.read_selection(config.max_units + 1)?;
        if !now.is_some_and(|t| t.same_as(&source)) {
            return Err(Stop::Failed(Failure::TargetChanged));
        }
    }
    platform.write(&converted)?;
    let verified = platform.verify(&converted);
    let layout_switched = platform.switch_layout(&to);
    Ok(Outcome::Converted {
        verified,
        layout_switched,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory single-line text field with a selection.
    struct Fake {
        text: Vec<u16>,
        sel: (usize, usize),
        exe: &'static str,
        refuse: Option<Refusal>,
        writes: usize,
        layout: Option<String>,
        /// Simulates the user typing a character between read and select.
        race: bool,
        write_error: Option<Stop>,
    }

    impl Fake {
        fn new(text: &str, sel: (usize, usize)) -> Self {
            Self {
                text: text.encode_utf16().collect(),
                sel,
                exe: "notepad.exe",
                refuse: None,
                writes: 0,
                layout: None,
                race: false,
                write_error: None,
            }
        }
        fn caret_at_end(text: &str) -> Self {
            let n = text.encode_utf16().count();
            Self::new(text, (n, n))
        }
        fn content(&self) -> String {
            String::from_utf16_lossy(&self.text)
        }
        fn slice(&self, a: usize, b: usize) -> UserText {
            UserText::new(String::from_utf16_lossy(&self.text[a..b]))
        }
    }

    impl Platform for Fake {
        fn begin(&mut self) -> Result<TargetInfo, Stop> {
            match self.refuse {
                Some(r) => Err(Stop::Refused(r)),
                None => Ok(TargetInfo {
                    exe: self.exe.into(),
                }),
            }
        }
        fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
            let (a, b) = self.sel;
            if a == b {
                return Ok(None);
            }
            Ok(Some(self.slice(a, b.min(a + max_units))))
        }
        fn read_before_caret(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
            let end = self.sel.0;
            Ok(Some(self.slice(end.saturating_sub(max_units), end)))
        }
        fn select_before_caret(&mut self, units: usize) -> Result<(), Stop> {
            if self.race {
                self.text.insert(self.sel.0, u16::from(b'x'));
                self.sel = (self.sel.0 + 1, self.sel.0 + 1);
            }
            self.sel = (self.sel.0 - units, self.sel.0);
            Ok(())
        }
        fn write(&mut self, text: &UserText) -> Result<(), Stop> {
            if let Some(e) = self.write_error {
                return Err(e);
            }
            let (a, b) = self.sel;
            let new: Vec<u16> = text.expose().encode_utf16().collect();
            let n = new.len();
            self.text.splice(a..b, new);
            self.sel = (a + n, a + n);
            self.writes += 1;
            Ok(())
        }
        fn verify(&mut self, expected: &UserText) -> bool {
            let end = self.sel.0;
            let n = expected.utf16_len();
            end >= n && self.slice(end - n, end).same_as(expected)
        }
        fn switch_layout(&mut self, lang: &str) -> bool {
            self.layout = Some(lang.to_owned());
            true
        }
    }

    fn go(f: &mut Fake) -> Outcome {
        let c = ks_core::builtin().expect("built-in data");
        run(f, &c, &Config::default())
    }

    const OK: Outcome = Outcome::Converted {
        verified: true,
        layout_switched: true,
    };

    #[test]
    fn converts_last_word_before_caret() {
        let mut f = Fake::caret_at_end("ghbdtn");
        assert_eq!(go(&mut f), OK);
        assert_eq!(f.content(), "привет");
        assert_eq!(f.layout.as_deref(), Some("ru"));
    }

    #[test]
    fn converts_only_the_last_word_and_keeps_trailing_spaces() {
        let mut f = Fake::caret_at_end("hello ghbdtn  ");
        assert_eq!(go(&mut f), OK);
        assert_eq!(f.content(), "hello привет  ");
        assert_eq!(f.writes, 1);
    }

    #[test]
    fn converts_selection_not_the_whole_field() {
        let mut f = Fake::new("abc ghbdtn xyz", (4, 10));
        assert_eq!(go(&mut f), OK);
        assert_eq!(f.content(), "abc привет xyz");
    }

    #[test]
    fn nothing_on_empty_field_and_after_newline() {
        for text in ["", "ghbdtn\n", "   ", "123 "] {
            let mut f = Fake::caret_at_end(text);
            assert_eq!(go(&mut f), Outcome::Nothing);
            assert_eq!(f.writes, 0);
            assert_eq!(f.content(), text);
        }
    }

    #[test]
    fn os_refusals_pass_through_without_reading_or_writing() {
        for r in [
            Refusal::Password,
            Refusal::Elevated,
            Refusal::SecureDesktop,
            Refusal::SelfTarget,
            Refusal::Console,
            Refusal::ReadOnly,
        ] {
            let mut f = Fake::caret_at_end("ghbdtn");
            f.refuse = Some(r);
            assert_eq!(go(&mut f), Outcome::Refused(r));
            assert_eq!(f.writes, 0);
        }
    }

    #[test]
    fn excluded_app_is_refused() {
        let mut f = Fake::caret_at_end("ghbdtn");
        f.exe = "keepass.exe";
        let c = ks_core::builtin().expect("built-in data");
        let cfg = Config {
            excluded: vec!["keepass.exe".into()],
            ..Config::default()
        };
        assert_eq!(
            run(&mut f, &c, &cfg),
            Outcome::Refused(Refusal::ExcludedApp)
        );
        assert_eq!(f.content(), "ghbdtn");
    }

    #[test]
    fn too_long_selection_and_unbounded_word_are_refused() {
        let long = "ghbdtn ".repeat(40);
        let n = long.encode_utf16().count();
        let mut f = Fake::new(&long, (0, n));
        assert_eq!(go(&mut f), Outcome::Refused(Refusal::TooLong));
        let word = "g".repeat(100);
        let mut f = Fake::caret_at_end(&word);
        assert_eq!(go(&mut f), Outcome::Refused(Refusal::TooLong));
        assert_eq!(f.writes, 0);
    }

    #[test]
    fn correct_text_is_low_confidence_and_mixed_is_refused() {
        let mut f = Fake::caret_at_end("привет");
        assert_eq!(go(&mut f), Outcome::Refused(Refusal::LowConfidence));
        let mut f = Fake::new("привет ghbdtn", (0, 13));
        assert_eq!(go(&mut f), Outcome::Refused(Refusal::Mixed));
        assert_eq!(f.writes, 0);
    }

    #[test]
    fn text_changed_between_read_and_select_aborts_before_write() {
        let mut f = Fake::caret_at_end("ghbdtn");
        f.race = true;
        assert_eq!(go(&mut f), Outcome::Failed(Failure::TargetChanged));
        assert_eq!(f.writes, 0);
    }

    #[test]
    fn write_failure_is_reported_and_layout_is_not_switched() {
        let mut f = Fake::caret_at_end("ghbdtn");
        f.write_error = Some(Stop::Failed(Failure::TargetChanged));
        assert_eq!(go(&mut f), Outcome::Failed(Failure::TargetChanged));
        assert_eq!(f.layout, None);
    }

    #[test]
    fn exe_names_are_validated() {
        assert_eq!(normalize_exe("  KeePass.EXE "), Some("keepass.exe".into()));
        for bad in [
            "",
            ".exe",
            "notepad",
            "c:\\x\\a.exe",
            "a/b.exe",
            "a*.exe",
            "a\n.exe",
        ] {
            assert_eq!(normalize_exe(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn outcome_codes_are_stable() {
        assert_eq!(OK.code(), "CONVERTED");
        assert_eq!(Outcome::Refused(Refusal::Password).code(), "PASSWORD");
        assert_eq!(Outcome::Nothing.code(), "NOTHING");
    }
}
