//! Macro Studio pipeline (Beta 2 step 1: text macros; code macros behind [`MacroRunner`]).
//!
//! `hotkey → content policy → gate (same refusals as conversion) → [runner, code only] →
//! content policy of the result → write (insert at caret / replace selection) → post-check`.
//! No layout switch.
//!
//! The macro's content is the user's own text and travels only as [`UserText`]. Code is
//! never executed here: a code macro is handed to a [`MacroRunner`] (the sandbox host plugs
//! in later, CLAUDE.md rule 9) and only the *action* it returns is applied, after the same
//! content policy as a text macro. Until then [`NoCodeRunner`] refuses with
//! [`Refusal::NotAvailable`].

use crate::{Config, Outcome, Platform, Refusal, Stop, UserText, admit};

/// Maximum length of a macro's text, in characters.
pub const MACRO_MAX_CHARS: usize = 5000;

/// Per-slot permissions (D-07, D-18). Everything is off by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MacroPermissions {
    /// The text may contain `\n` and `\t` (D-18). They are inserted as characters, never
    /// sent as Enter / Tab key presses.
    pub allow_enter_tab: bool,
}

/// Why a macro text is not acceptable. Codes only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentError {
    /// More than [`MACRO_MAX_CHARS`] characters.
    TooLong,
    /// A control character other than `\n` / `\t`, a line / paragraph separator or a
    /// format character (bidi control, zero-width character, BOM...).
    ForbiddenChar,
    /// `\n` or `\t` without the slot's permission.
    EnterTabNotPermitted,
}

impl ContentError {
    /// Stable code for the UI boundary.
    pub fn code(self) -> &'static str {
        match self {
            ContentError::TooLong => "content_too_long",
            ContentError::ForbiddenChar => "content_forbidden_chars",
            ContentError::EnterTabNotPermitted => "content_enter_tab_not_permitted",
        }
    }

    fn refusal(self) -> Refusal {
        match self {
            ContentError::TooLong => Refusal::TooLong,
            ContentError::ForbiddenChar | ContentError::EnterTabNotPermitted => {
                Refusal::ForbiddenChars
            }
        }
    }
}

/// Inclusive ranges of general category Cf (Unicode 16.0, DerivedGeneralCategory.txt).
/// Kept as a table so the crate needs no Unicode-data dependency.
const CF_RANGES: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

/// `true` for a character of general category Cf (format: bidi overrides/isolates,
/// zero-width space/joiners, BOM, soft hyphen, tags...). Shared with the write policy of
/// `ks-platform-win` (WS-06), so a macro that can be saved is never refused at insert time
/// for its characters.
pub fn is_format_char(c: char) -> bool {
    let u = u32::from(c);
    CF_RANGES.iter().any(|&(lo, hi)| (lo..=hi).contains(&u))
}

/// `true` for characters a macro may never contain - the same set the write policy refuses
/// (WS-06), except `\n` / `\t` (D-18, behind the slot's permission): controls (Cc),
/// U+2028 / U+2029 (they act as line breaks in many editors and would bypass the Enter
/// permission) and every format character (Cf: bidi controls spoof the visible text,
/// zero-width characters hide content).
fn always_forbidden(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(c, '\u{2028}' | '\u{2029}')
        || is_format_char(c)
}

/// Content policy of a macro text (checked when the slot is saved and again before every
/// write: the settings file and a runner's result are untrusted).
pub fn check_content(text: &str, permissions: MacroPermissions) -> Result<(), ContentError> {
    let mut chars = 0usize;
    for c in text.chars() {
        chars += 1;
        if chars > MACRO_MAX_CHARS {
            return Err(ContentError::TooLong);
        }
        if always_forbidden(c) {
            return Err(ContentError::ForbiddenChar);
        }
        if (c == '\n' || c == '\t') && !permissions.allow_enter_tab {
            return Err(ContentError::EnterTabNotPermitted);
        }
    }
    Ok(())
}

/// Language of a code macro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeLanguage {
    JavaScript,
    Python,
}

/// What a slot holds.
pub enum MacroBody {
    /// Text inserted as is.
    Text(UserText),
    /// Source code for the sandbox. Never executed by this crate.
    Code {
        language: CodeLanguage,
        source: UserText,
    },
}

/// One macro slot, ready to run.
pub struct Macro {
    pub body: MacroBody,
    pub permissions: MacroPermissions,
}

/// What a code macro asks the engine to do. Applied only after the content policy.
pub enum MacroAction {
    /// Insert the text at the caret / replace the selection.
    Insert(UserText),
    /// Do nothing.
    Nothing,
}

/// Context handed to a [`MacroRunner`]. Filled by the pipeline only as far as the runner
/// asks for it (D-07: data minimisation).
pub struct MacroInput<'a> {
    /// The selection of the target, if [`MacroRunner::wants_selection`] and there is one.
    pub selection: Option<&'a UserText>,
}

/// Executes code macros outside the engine (the sandbox host). The engine never sees how;
/// it only applies the returned [`MacroAction`] after its own policy checks.
pub trait MacroRunner {
    /// `true` if the runner needs the target's selection as input.
    fn wants_selection(&self) -> bool {
        false
    }
    /// Runs `source`. A refusal ends the operation without any change.
    fn run(
        &mut self,
        language: CodeLanguage,
        source: &UserText,
        input: &MacroInput<'_>,
    ) -> Result<MacroAction, Refusal>;
}

/// Stub runner until the sandbox is integrated: every code macro is refused.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoCodeRunner;

impl MacroRunner for NoCodeRunner {
    fn run(
        &mut self,
        _language: CodeLanguage,
        _source: &UserText,
        _input: &MacroInput<'_>,
    ) -> Result<MacroAction, Refusal> {
        Err(Refusal::NotAvailable)
    }
}

/// Runs the macro in `slot` against the focused field. Never panics on user data and
/// never returns text.
pub fn run_macro<P: Platform, R: MacroRunner + ?Sized>(
    platform: &mut P,
    slot: &Macro,
    runner: &mut R,
    config: &Config,
) -> Outcome {
    match run_macro_inner(platform, slot, runner, config) {
        Ok(outcome) => outcome,
        Err(stop) => stop.into(),
    }
}

fn run_macro_inner<P: Platform, R: MacroRunner + ?Sized>(
    platform: &mut P,
    slot: &Macro,
    runner: &mut R,
    config: &Config,
) -> Result<Outcome, Stop> {
    let produced;
    let text = match &slot.body {
        MacroBody::Text(text) => {
            // Checked before the target is even looked at.
            check_content(text.expose(), slot.permissions)
                .map_err(|e| Stop::Refused(e.refusal()))?;
            admit(platform, config)?;
            text
        }
        MacroBody::Code { language, source } => {
            admit(platform, config)?;
            let selection = if runner.wants_selection() {
                platform
                    .read_selection(MACRO_MAX_CHARS.saturating_mul(2) + 1)?
                    .filter(|t| !t.is_empty())
            } else {
                None
            };
            let input = MacroInput {
                selection: selection.as_ref(),
            };
            match runner.run(*language, source, &input) {
                Ok(MacroAction::Insert(text)) => produced = text,
                Ok(MacroAction::Nothing) => return Ok(Outcome::Nothing),
                Err(refusal) => return Err(Stop::Refused(refusal)),
            }
            check_content(produced.expose(), slot.permissions)
                .map_err(|e| Stop::Refused(e.refusal()))?;
            &produced
        }
    };
    if text.is_empty() {
        return Ok(Outcome::Nothing);
    }
    platform.write(text)?;
    let verified = platform.verify(text);
    Ok(Outcome::Inserted { verified })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFF: MacroPermissions = MacroPermissions {
        allow_enter_tab: false,
    };
    const ON: MacroPermissions = MacroPermissions {
        allow_enter_tab: true,
    };

    #[test]
    fn content_policy() {
        assert_eq!(check_content("", OFF), Ok(()));
        assert_eq!(check_content("Hello, мир 👋🏽", OFF), Ok(()));
        assert_eq!(
            check_content("a\nb", OFF),
            Err(ContentError::EnterTabNotPermitted)
        );
        assert_eq!(
            check_content("a\tb", OFF),
            Err(ContentError::EnterTabNotPermitted)
        );
        assert_eq!(check_content("a\nb\tc", ON), Ok(()));
        for bad in [
            "a\rb",
            "a\u{0}b",
            "\u{1b}[2J",
            "\u{85}",
            "a\u{2028}b",
            "\u{202E}x",
        ] {
            assert_eq!(check_content(bad, ON), Err(ContentError::ForbiddenChar));
        }
        // Zero-width and other format characters, as the write policy (WS-06) refuses them
        // at insert time: U+200B..U+200D, U+2060, U+FEFF, soft hyphen, tags.
        for bad in [
            "a\u{200B}b",
            "a\u{200C}b",
            "👩\u{200D}💻",
            "a\u{2060}b",
            "\u{FEFF}a",
            "a\u{00AD}b",
            "a\u{E0041}",
            "a\u{061C}b",
        ] {
            assert_eq!(check_content(bad, ON), Err(ContentError::ForbiddenChar));
        }
        assert_eq!(check_content("Добрый день! 👋 ok", OFF), Ok(()));
        let max = "ж".repeat(MACRO_MAX_CHARS);
        assert_eq!(check_content(&max, OFF), Ok(()));
        assert_eq!(
            check_content(&format!("{max}x"), OFF),
            Err(ContentError::TooLong)
        );
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(ContentError::TooLong.code(), "content_too_long");
        assert_eq!(
            ContentError::ForbiddenChar.code(),
            "content_forbidden_chars"
        );
        assert_eq!(
            ContentError::EnterTabNotPermitted.code(),
            "content_enter_tab_not_permitted"
        );
    }
}
