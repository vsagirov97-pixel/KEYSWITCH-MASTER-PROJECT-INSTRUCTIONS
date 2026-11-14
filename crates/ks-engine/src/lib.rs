//! KeySwitch operation pipelines: conversion ([`run`]) and Macro Studio ([`run_macro`]).
//!
//! Conversion: `hotkey → gate → read → detect/convert → (language choice) → write →
//! post-check → switch layout`. Macros: see [`macros`].
//!
//! This crate has no OS dependencies and no `unsafe`: everything system-specific sits behind
//! the [`Platform`] trait (implemented by `ks-platform-win`). Results are codes only — no type
//! returned from [`run`] can hold user text; text travels exclusively as [`UserText`].

pub mod macros;
mod user_text;

pub use macros::{
    CodeLanguage, ContentError, MACRO_MAX_CHARS, Macro, MacroAction, MacroBody, MacroInput,
    MacroPermissions, MacroRunner, NoCodeRunner, check_content, run_macro,
};
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
    /// The feature is not available in this build (code macros before the sandbox).
    NotAvailable,
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
    /// A macro's text was inserted at the caret (or replaced the selection).
    Inserted {
        /// The post-check re-read the target and found the inserted text.
        verified: bool,
    },
    /// Nothing to convert (no selection and no word before the caret, no letters, no change)
    /// or an empty macro.
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
            Outcome::Inserted { verified: true } => "MACRO_INSERTED",
            Outcome::Inserted { verified: false } => "MACRO_INSERTED_UNVERIFIED",
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
                Refusal::NotAvailable => "NOT_AVAILABLE",
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
    /// Opaque identity of the captured field (window, process), valid after
    /// [`Platform::begin`]. Lets [`History`] undo the previous conversion in the same field.
    /// `None` (the default): no undo-style toggle.
    fn field_key(&self) -> Option<u64> {
        None
    }
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

impl From<Stop> for Outcome {
    fn from(stop: Stop) -> Self {
        match stop {
            Stop::Refused(r) => Outcome::Refused(r),
            Stop::Failed(f) => Outcome::Failed(f),
            Stop::Cancelled => Outcome::Cancelled,
        }
    }
}

/// Runs one conversion operation. Never panics on user data and never returns text.
/// No undo-style toggle (see [`run_with_history`]).
pub fn run<P: Platform>(platform: &mut P, converter: &Converter, config: &Config) -> Outcome {
    run_with_history(platform, converter, config, &mut History::default())
}

/// As [`run`], remembering the last conversion in `history`: if the next press in the same
/// field finds exactly the text this conversion wrote (before the caret, or selected), it
/// writes back what was there before - a wrong guess is undone with the same hotkey
/// (QA-WF-02). The toggle works again in the other direction.
pub fn run_with_history<P: Platform>(
    platform: &mut P,
    converter: &Converter,
    config: &Config,
    history: &mut History,
) -> Outcome {
    match run_inner(platform, converter, config, history) {
        Ok(outcome) => outcome,
        Err(stop) => stop.into(),
    }
}

/// How long the last conversion can be undone by pressing the hotkey again.
pub const TOGGLE_WINDOW: std::time::Duration = std::time::Duration::from_secs(30);

/// The last conversion, kept for the undo-style toggle. Holds user text only as
/// [`UserText`] (wiped on drop) and at most [`TOGGLE_WINDOW`]; `Debug` shows no text.
#[derive(Default)]
pub struct History {
    last: Option<LastConversion>,
}

struct LastConversion {
    field: u64,
    at: std::time::Instant,
    /// The text before the conversion.
    before: UserText,
    /// The text the conversion wrote.
    after: UserText,
    /// Language of `before`, for the layout switch on the way back.
    before_lang: Option<String>,
}

impl LastConversion {
    /// The text to write back and its language, if `source` in `field` is exactly what this
    /// conversion wrote.
    fn toggle(self, field: Option<u64>, source: &UserText) -> Option<(Option<String>, UserText)> {
        (Some(self.field) == field && self.after.same_as(source))
            .then_some((self.before_lang, self.before))
    }
}

impl std::fmt::Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("History")
            .field("held", &self.last.is_some())
            .finish()
    }
}

impl History {
    /// Forgets the last conversion (macro run, pause, quit).
    pub fn clear(&mut self) {
        self.last = None;
    }

    /// Forgets the last conversion once [`TOGGLE_WINDOW`] has passed.
    pub fn expire(&mut self) {
        if self
            .last
            .as_ref()
            .is_some_and(|l| l.at.elapsed() > TOGGLE_WINDOW)
        {
            self.last = None;
        }
    }

    /// `true` while a conversion can be undone.
    pub fn is_held(&self) -> bool {
        self.last.is_some()
    }

    fn record(
        &mut self,
        field: Option<u64>,
        before: &UserText,
        after: &UserText,
        before_lang: Option<String>,
    ) {
        self.last = field.map(|field| LastConversion {
            field,
            at: std::time::Instant::now(),
            before: UserText::new(before.expose().to_owned()),
            after: UserText::new(after.expose().to_owned()),
            before_lang,
        });
    }
}

/// Captures the target and applies every refusal shared by all operations: the platform's
/// own (secure desktop, self, elevated, console, password, read-only, no text interface)
/// and the excluded-apps list.
pub(crate) fn admit<P: Platform>(platform: &mut P, config: &Config) -> Result<(), Stop> {
    let target = platform.begin()?;
    if target.exe.is_empty() {
        // QA-B1-06: fail closed even if a platform does not refuse unknown executables.
        if !config.excluded.is_empty() {
            return Err(Stop::Refused(Refusal::Unsupported));
        }
    } else if config.excluded.contains(&target.exe) {
        return Err(Stop::Refused(Refusal::ExcludedApp));
    }
    Ok(())
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

/// Language of the last single-language word of `text`, if any.
fn last_word_language(converter: &Converter, text: &str) -> Option<String> {
    text.split_whitespace()
        .rev()
        .find_map(|w| match converter.detect(w) {
            Detection::Single(l) => Some(l),
            _ => None,
        })
}

fn run_inner<P: Platform>(
    platform: &mut P,
    converter: &Converter,
    config: &Config,
    history: &mut History,
) -> Result<Outcome, Stop> {
    // Only the very next press may undo the last conversion: every press consumes it.
    history.expire();
    let previous = history.last.take();
    admit(platform, config)?;
    let field = platform.field_key();

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

    let (to, converted) = match previous.and_then(|p| p.toggle(field, &source)) {
        // The same hotkey again on exactly what the last conversion wrote: undo it.
        Some((lang, before)) => (lang, before),
        None => {
            let (to, converted) = decide(platform, converter, &source, mode)?;
            (Some(to), converted)
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
    let layout_switched = verified && to.as_deref().is_some_and(|l| platform.switch_layout(l));
    let from = last_word_language(converter, source.expose());
    history.record(field, &source, &converted, from);
    Ok(Outcome::Converted {
        verified,
        layout_switched,
    })
}

/// Target language and converted text of `source`, asking the user when unclear.
fn decide<P: Platform>(
    platform: &mut P,
    converter: &Converter,
    source: &UserText,
    mode: ConvertMode,
) -> Result<(String, UserText), Stop> {
    let nothing = || Ok((String::new(), UserText::new(source.expose().to_owned())));
    Ok(match converter.detect(source.expose()) {
        Detection::NoLetters => return nothing(),
        Detection::Mixed(_) => {
            // Letters of several languages: first a per-token decision (only words that are
            // confidently in the wrong layout change); the layout follows the last word.
            let auto = converter.convert_mixed_auto(source.expose(), mode);
            match last_word_language(converter, &auto.text) {
                Some(l) if auto.confidence == Confidence::High => (l, UserText::new(auto.text)),
                _ => {
                    // Unclear tokens: only the user knows which language was meant.
                    let to = ask(platform, converter, Refusal::Mixed)?;
                    let Ok(conversion) = converter.convert_mixed_with(source.expose(), &to, mode)
                    else {
                        return nothing();
                    };
                    (to, UserText::new(conversion.text))
                }
            }
        }
        Detection::Single(from) => {
            let Some(to) = converter
                .languages()
                .find(|l| *l != from)
                .map(str::to_owned)
            else {
                return nothing();
            };
            let Ok(conversion) = converter.convert_with(source.expose(), &from, &to, mode) else {
                return nothing();
            };
            let confidence = conversion.confidence;
            let converted = UserText::new(conversion.text);
            if confidence == Confidence::Low {
                let picked = ask(platform, converter, Refusal::LowConfidence)?;
                if picked == from {
                    // The user confirms the text is already in the right language.
                    return nothing();
                }
                if picked != to {
                    let Ok(other) = converter.convert_with(source.expose(), &from, &picked, mode)
                    else {
                        return nothing();
                    };
                    (picked, UserText::new(other.text))
                } else {
                    (to, converted)
                }
            } else {
                (to, converted)
            }
        }
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
    fn mixed_selection_converts_confident_tokens_without_asking() {
        let mut f = Fake::new("hello цщкдв", (0, 11));
        f.choice = Choice::Cancelled;
        assert_eq!(go(&mut f), OK);
        assert_eq!((f.content().as_str(), f.asked), ("hello world", 0));
        assert_eq!(f.layout.as_deref(), Some("en"));
        // Nothing confidently wrong: the popup decides.
        let mut f = Fake::new("привет world", (0, 12));
        f.choice = Choice::Cancelled;
        assert_eq!(go(&mut f), Outcome::Cancelled);
        assert_eq!((f.content().as_str(), f.asked), ("привет world", 1));
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
        // language selection is converted key for key without asking). `привет world` has no
        // confidently wrong token, so the per-token decision gives way to the popup.
        for (text, sel) in [("привет", (6, 6)), ("привет world", (0, 12))] {
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
        let mut f = Fake::new("привет world", (0, 12));
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
        assert_eq!(
            Outcome::Inserted { verified: true }.code(),
            "MACRO_INSERTED"
        );
        assert_eq!(
            Outcome::Inserted { verified: false }.code(),
            "MACRO_INSERTED_UNVERIFIED"
        );
        assert_eq!(
            Outcome::Refused(Refusal::NotAvailable).code(),
            "NOT_AVAILABLE"
        );
    }
}
