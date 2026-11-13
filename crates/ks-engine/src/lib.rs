//! KeySwitch operation pipeline (Beta 1: conversion only).
//!
//! `hotkey → gate → read → detect/convert → (language choice) → write → post-check → switch
//! layout`.
//!
//! This crate has no OS dependencies and no `unsafe`: everything system-specific sits behind
//! the [`Platform`] trait (implemented by `ks-platform-win`). Results are codes only — no type
//! returned from [`run`] can hold user text; text travels exclusively as [`UserText`].

mod user_text;

pub use user_text::{UserText, wipe};

use ks_core::{Confidence, ConvertMode, Converter, Detection};

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
    /// The converter is not confident and the platform could not ask the user.
    LowConfidence,
    /// Letters of several languages and the platform could not ask the user.
    Mixed,
    /// No usable text interface in the focused control.
    Unsupported,
    /// The text contains characters the write method must not inject (`\n`, `\t`, bidi...).
    ForbiddenChars,
    /// The clipboard method is needed but the clipboard cannot be used safely now (foreign
    /// formats, size, unresponsive owner, remote session). Nothing was touched.
    Clipboard,
    /// Another program touched the clipboard during the operation; it was aborted.
    ClipboardListener,
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
    /// The user dismissed the language choice (Esc or timeout).
    Cancelled,
}

/// Answer of the platform's language-choice popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The platform cannot ask (no popup); the engine refuses as before.
    Unavailable,
    /// Esc, timeout, or the target changed while the popup was open.
    Cancelled,
    /// Index into the offered languages.
    Picked(usize),
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
    /// The user dismissed the language choice; nothing was changed.
    Cancelled,
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
            Outcome::Cancelled => "CANCELLED",
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
                Refusal::Clipboard => "CLIPBOARD_UNAVAILABLE",
                Refusal::ClipboardListener => "CLIPBOARD_LISTENER",
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
    /// Lower-cased executable file name (`notepad.exe`); empty if unknown. With an unknown
    /// name the excluded list cannot be honoured: the engine refuses (`UNSUPPORTED`) unless
    /// the list is empty.
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
    /// Undoes [`Platform::select_before_caret`] after the operation stopped before the
    /// write took place: if the captured field still exists and its selection is still
    /// exactly `selected`, collapses it so the caret is back where it was. Never touches
    /// any other window. Best effort; the default does nothing.
    fn restore_caret(&mut self, selected: &UserText) {
        let _ = selected;
    }
    /// Replaces the current selection with `text`.
    fn write(&mut self, text: &UserText) -> Result<(), Stop>;
    /// `true` if the text right before the caret now equals `expected`.
    fn verify(&mut self, expected: &UserText) -> bool;
    /// Asks the target to switch to the keyboard layout of language `lang` (`"ru"`, `"en"`).
    fn switch_layout(&mut self, lang: &str) -> bool;
    /// Asks the user which of `langs` the text should become (low confidence or mixed
    /// text). Must not show or keep the text. Nothing has been written yet; the write that
    /// follows re-checks the target and the operation generation.
    fn choose_language(&mut self, langs: &[&str]) -> Choice {
        let _ = langs;
        Choice::Unavailable
    }
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

/// Caret mode after selecting: confirms the selection is the converted fragment, then
/// writes. On any stop the selection is undone (QA-B1-04): left in place, the user's next
/// keystroke would delete the word.
fn select_checked_write<P: Platform>(
    platform: &mut P,
    config: &Config,
    source: &UserText,
    converted: &UserText,
) -> Result<(), Stop> {
    // The selection we just made must be exactly the fragment we converted.
    let now = match platform.read_selection(config.max_units + 1) {
        Ok(Some(now)) => now,
        Ok(None) => return Err(Stop::Failed(Failure::TargetChanged)),
        Err(stop) => {
            platform.restore_caret(source);
            return Err(stop);
        }
    };
    if !now.same_as(source) {
        // Text changed between read and select: undo what was actually selected.
        platform.restore_caret(&now);
        return Err(Stop::Failed(Failure::TargetChanged));
    }
    let written = platform.write(converted);
    if written.is_err() {
        platform.restore_caret(source);
    }
    written
}

/// Runs one conversion operation. Never panics on user data and never returns text.
pub fn run<P: Platform>(platform: &mut P, converter: &Converter, config: &Config) -> Outcome {
    match run_inner(platform, converter, config) {
        Ok(outcome) => outcome,
        Err(Stop::Refused(r)) => Outcome::Refused(r),
        Err(Stop::Failed(f)) => Outcome::Failed(f),
        Err(Stop::Cancelled) => Outcome::Cancelled,
    }
}

/// The language the user picked, or the stop that ends the operation.
fn ask<P: Platform>(
    platform: &mut P,
    converter: &Converter,
    unavailable: Refusal,
) -> Result<String, Stop> {
    let langs: Vec<&str> = converter.languages().collect();
    match platform.choose_language(&langs) {
        Choice::Picked(i) => langs.get(i).map(|l| (*l).to_owned()).ok_or(Stop::Cancelled),
        Choice::Cancelled => Err(Stop::Cancelled),
        Choice::Unavailable => Err(Stop::Refused(unavailable)),
    }
}

fn run_inner<P: Platform>(
    platform: &mut P,
    converter: &Converter,
    config: &Config,
) -> Result<Outcome, Stop> {
    let target = platform.begin()?;
    if target.exe.is_empty() {
        // QA-B1-06: fail closed even if a platform does not refuse unknown executables.
        if !config.excluded.is_empty() {
            return Err(Stop::Refused(Refusal::Unsupported));
        }
    } else if config.excluded.contains(&target.exe) {
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
    // D-31: a selection is re-typed key for key; the last word before the caret is guessed
    // with the language model and its protections.
    let mode = if by_caret {
        ConvertMode::Smart
    } else {
        ConvertMode::Literal
    };

    let (to, converted) = match converter.detect(source.expose()) {
        Detection::NoLetters => return Ok(Outcome::Nothing),
        Detection::Mixed(_) => {
            // Letters of several languages: only the user knows which one was meant.
            let to = ask(platform, converter, Refusal::Mixed)?;
            let Ok(conversion) = converter.convert_mixed_with(source.expose(), &to, mode) else {
                return Ok(Outcome::Nothing);
            };
            (to, UserText::new(conversion.text))
        }
        Detection::Single(from) => {
            let Some(to) = converter
                .languages()
                .find(|l| *l != from)
                .map(str::to_owned)
            else {
                return Ok(Outcome::Nothing);
            };
            let Ok(conversion) = converter.convert_with(source.expose(), &from, &to, mode) else {
                return Ok(Outcome::Nothing);
            };
            let confidence = conversion.confidence;
            let converted = UserText::new(conversion.text);
            if confidence == Confidence::Low {
                let picked = ask(platform, converter, Refusal::LowConfidence)?;
                if picked == from {
                    // The user confirms the text is already in the right language.
                    return Ok(Outcome::Nothing);
                }
                if picked != to {
                    let Ok(other) = converter.convert_with(source.expose(), &from, &picked, mode)
                    else {
                        return Ok(Outcome::Nothing);
                    };
                    (picked, UserText::new(other.text))
                } else {
                    (to, converted)
                }
            } else {
                (to, converted)
            }
        }
    };
    if converted.same_as(&source) {
        return Ok(Outcome::Nothing);
    }

    if by_caret {
        platform.select_before_caret(source.utf16_len())?;
        // From here on a stop must not leave the word selected (QA-B1-04): the next
        // keystroke of the user would delete it.
        select_checked_write(platform, config, &source, &converted)?;
    } else {
        platform.write(&converted)?;
    }
    let verified = platform.verify(&converted);
    // The layout follows the text: no switch unless the write was confirmed.
    let layout_switched = verified && platform.switch_layout(&to);
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
        /// Simulates a target whose text cannot be re-read after the write.
        blind: bool,
        /// Answer of the language popup.
        choice: Choice,
        asked: usize,
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
                blind: false,
                choice: Choice::Unavailable,
                asked: 0,
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
            !self.blind && end >= n && self.slice(end - n, end).same_as(expected)
        }
        fn restore_caret(&mut self, selected: &UserText) {
            let (a, b) = self.sel;
            if a < b && self.slice(a, b).same_as(selected) {
                self.sel = (b, b);
            }
        }
        fn switch_layout(&mut self, lang: &str) -> bool {
            self.layout = Some(lang.to_owned());
            true
        }
        fn choose_language(&mut self, _langs: &[&str]) -> Choice {
            self.asked += 1;
            self.choice
        }
    }

    /// Index of `lang` among the built-in languages, as the popup would return it.
    fn pick(lang: &str) -> Choice {
        let c = ks_core::builtin().expect("built-in data");
        Choice::Picked(c.languages().position(|l| l == lang).expect("language"))
    }

    #[test]
    fn confident_conversion_never_asks() {
        let mut f = Fake::caret_at_end("ghbdtn");
        f.choice = Choice::Cancelled;
        assert_eq!(go(&mut f), OK);
        assert_eq!(f.asked, 0);
    }

    #[test]
    fn low_confidence_follows_the_users_choice() {
        let mut f = Fake::caret_at_end("привет");
        f.choice = pick("en");
        assert_eq!(go(&mut f), OK);
        assert_eq!(f.content(), "ghbdtn");
        assert_eq!(f.layout.as_deref(), Some("en"));
        // Picking the language the text is already in changes nothing.
        let mut f = Fake::caret_at_end("привет");
        f.choice = pick("ru");
        assert_eq!(go(&mut f), Outcome::Nothing);
        assert_eq!((f.writes, f.asked), (0, 1));
    }

    #[test]
    fn mixed_text_is_converted_to_the_picked_language() {
        let mut f = Fake::new("привет ghbdtn", (0, 13));
        f.choice = pick("ru");
        assert_eq!(go(&mut f), OK);
        assert_eq!(f.content(), "привет привет");
        assert_eq!(f.layout.as_deref(), Some("ru"));
    }

    #[test]
    fn selection_is_literal_and_caret_word_is_smart() {
        // D-31.
        let mut f = Fake::new("v1.2.3", (0, 6));
        f.choice = Choice::Cancelled;
        assert_eq!(go(&mut f), OK);
        assert_eq!((f.content().as_str(), f.asked), ("м1.2.3", 0));
        let mut f = Fake::new("ltkf.", (0, 5));
        assert_eq!(go(&mut f), OK);
        assert_eq!(f.content(), "делаю");
        let mut f = Fake::caret_at_end("v1.2.3");
        assert_eq!(go(&mut f), Outcome::Refused(Refusal::LowConfidence));
        assert_eq!(f.content(), "v1.2.3");
    }

    #[test]
    fn dismissed_choice_cancels_without_writing() {
        // A caret word of low confidence and a mixed selection both ask (D-31: a single-
        // language selection is converted key for key without asking).
        for (text, sel) in [("привет", (6, 6)), ("привет ghbdtn", (0, 13))] {
            let mut f = Fake::new(text, sel);
            f.choice = Choice::Cancelled;
            assert_eq!(go(&mut f), Outcome::Cancelled);
            assert_eq!(f.writes, 0);
            assert_eq!(f.content(), text);
        }
        assert_eq!(Outcome::Cancelled.code(), "CANCELLED");
        // An index outside the offered list is treated as a dismissal.
        let mut f = Fake::caret_at_end("привет");
        f.choice = Choice::Picked(99);
        assert_eq!(go(&mut f), Outcome::Cancelled);
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
    fn unknown_executable_is_refused_when_exclusions_exist() {
        let mut f = Fake::caret_at_end("ghbdtn");
        f.exe = "";
        let cfg = Config {
            excluded: vec!["keepass.exe".into()],
            ..Config::default()
        };
        assert_eq!(
            run(&mut f, &ks_core::builtin().expect("built-in data"), &cfg),
            Outcome::Refused(Refusal::Unsupported)
        );
        assert_eq!(f.writes, 0);
        // Without any exclusion there is nothing to honour.
        let mut f = Fake::caret_at_end("ghbdtn");
        f.exe = "";
        assert!(matches!(go(&mut f), Outcome::Converted { .. }));
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
        // The caret is back after the typed character, nothing stays selected.
        assert_eq!(f.sel, (7, 7));
    }

    #[test]
    fn write_failure_is_reported_and_layout_is_not_switched() {
        let mut f = Fake::caret_at_end("ghbdtn");
        f.write_error = Some(Stop::Failed(Failure::TargetChanged));
        assert_eq!(go(&mut f), Outcome::Failed(Failure::TargetChanged));
        assert_eq!(f.layout, None);
    }

    #[test]
    fn refused_write_in_caret_mode_does_not_leave_the_word_selected() {
        for e in [
            Stop::Refused(Refusal::ForbiddenChars),
            Stop::Refused(Refusal::TooLong),
            Stop::Failed(Failure::ModifiersHeld),
            Stop::Failed(Failure::TargetChanged),
        ] {
            let mut f = Fake::caret_at_end("abc ghbdtn");
            f.write_error = Some(e);
            assert_eq!(go(&mut f).code(), run_code(e));
            assert_eq!(f.sel, (10, 10));
            assert_eq!(f.content(), "abc ghbdtn");
        }
    }

    fn run_code(e: Stop) -> &'static str {
        match e {
            Stop::Refused(r) => Outcome::Refused(r).code(),
            Stop::Failed(x) => Outcome::Failed(x).code(),
            Stop::Cancelled => Outcome::Cancelled.code(),
        }
    }

    #[test]
    fn unverified_write_does_not_switch_layout() {
        let mut f = Fake::caret_at_end("ghbdtn");
        f.blind = true;
        let expected = Outcome::Converted {
            verified: false,
            layout_switched: false,
        };
        assert_eq!(go(&mut f), expected);
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
