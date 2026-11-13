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
    /// bidirectional control.
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

/// `true` for characters a macro may never contain: controls (Cc) except `\n` / `\t`,
/// U+2028 / U+2029 (they act as line breaks in many editors and would bypass the Enter
/// permission) and the bidirectional controls (spoofing of the visible text).
fn always_forbidden(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(c,
            '\u{2028}' | '\u{2029}' | '\u{061C}' | '\u{200E}' | '\u{200F}'
            | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
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
        // Emoji ZWJ sequences stay allowed.
        assert_eq!(check_content("👩\u{200D}💻", OFF), Ok(()));
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
