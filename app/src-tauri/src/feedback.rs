//! Feedback for the last operation: shown only when it was refused or failed (no success
//! notice), plus the "clipboard kept" hint after an unconfirmed clipboard paste (D-26).
//!
//! The messages are the UI's own: they are read at compile time from `app/src/i18n/*.ts`
//! (keys `status.reason.<CODE>`), so the tray, the notice and the settings window cannot
//! drift apart. Nothing here can carry text from the user's application — only codes.

use ks_engine::Outcome;
use serde::Serialize;

use crate::settings::UiLanguage;

const EN: &str = include_str!("../../src/i18n/en.ts");
const RU: &str = include_str!("../../src/i18n/ru.ts");

/// Code of the hint shown when the previous clipboard is kept for the explicit restore.
pub const CLIPBOARD_HELD: &str = "CLIPBOARD_HELD";

/// Coarse result class, as `LastOperation.kind` in `app/src/api.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Converted,
    /// A macro's text was inserted.
    Inserted,
    Nothing,
    Cancelled,
    Refused,
    Failed,
}

/// Result of the last operation on the IPC boundary. Codes and flags only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastOperation {
    pub kind: Kind,
    /// `ks_engine::Outcome::code()`.
    pub code: &'static str,
    /// The clipboard from before the operation can still be restored from the tray menu.
    pub clipboard_held: bool,
}

impl LastOperation {
    pub fn new(outcome: Outcome, clipboard_held: bool) -> Self {
        let kind = match outcome {
            Outcome::Converted { .. } => Kind::Converted,
            Outcome::Inserted { .. } => Kind::Inserted,
            Outcome::Nothing => Kind::Nothing,
            Outcome::Cancelled => Kind::Cancelled,
            Outcome::Refused(_) => Kind::Refused,
            Outcome::Failed(_) => Kind::Failed,
        };
        Self {
            kind,
            code: outcome.code(),
            clipboard_held,
        }
    }

    /// The code the user should be told about, if any: the reason of a refusal or failure,
    /// or the "clipboard kept" hint. `None` for success, "nothing to fix" and a dismissed
    /// language choice.
    pub fn notice_code(&self) -> Option<&'static str> {
        match self.kind {
            Kind::Refused | Kind::Failed => Some(self.code),
            Kind::Converted | Kind::Inserted if self.clipboard_held => Some(CLIPBOARD_HELD),
            Kind::Converted | Kind::Inserted | Kind::Nothing | Kind::Cancelled => None,
        }
    }
}

/// The UI message for reason `code` in `language`.
pub fn text(language: UiLanguage, code: &str) -> Option<&'static str> {
    let dictionary = match language {
        UiLanguage::Ru => RU,
        UiLanguage::En => EN,
    };
    extract(dictionary, code)
}

/// Finds `"status.reason.<code>": "<message>",` in a dictionary source. Messages with
/// escapes are not supported (and rejected) — the dictionaries have none for these keys.
fn extract<'a>(source: &'a str, code: &str) -> Option<&'a str> {
    let key = format!("\"status.reason.{code}\": \"");
    let start = source.find(&key)? + key.len();
    let rest = &source[start..];
    let message = &rest[..rest.find("\",")?];
    let plain = !message.is_empty() && !message.contains(['\\', '\n', '"']);
    plain.then_some(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ks_engine::{Failure, Refusal};

    /// Every outcome the engine can report as a refusal or failure.
    const PROBLEMS: [Outcome; 19] = [
        Outcome::Refused(Refusal::Password),
        Outcome::Refused(Refusal::Elevated),
        Outcome::Refused(Refusal::SecureDesktop),
        Outcome::Refused(Refusal::SelfTarget),
        Outcome::Refused(Refusal::Console),
        Outcome::Refused(Refusal::ExcludedApp),
        Outcome::Refused(Refusal::ReadOnly),
        Outcome::Refused(Refusal::TooLong),
        Outcome::Refused(Refusal::LowConfidence),
        Outcome::Refused(Refusal::Mixed),
        Outcome::Refused(Refusal::Unsupported),
        Outcome::Refused(Refusal::ForbiddenChars),
        Outcome::Refused(Refusal::Clipboard),
        Outcome::Refused(Refusal::ClipboardListener),
        Outcome::Refused(Refusal::NotAvailable),
        Outcome::Failed(Failure::TargetChanged),
        Outcome::Failed(Failure::ModifiersHeld),
        Outcome::Failed(Failure::Read),
        Outcome::Failed(Failure::Write),
    ];

    #[test]
    fn every_refusal_and_failure_has_a_message_in_both_languages() {
        for outcome in PROBLEMS {
            let code = LastOperation::new(outcome, false)
                .notice_code()
                .expect("a problem is always reported");
            for language in [UiLanguage::Ru, UiLanguage::En] {
                assert!(text(language, code).is_some(), "{code}");
            }
        }
        for language in [UiLanguage::Ru, UiLanguage::En] {
            assert!(text(language, CLIPBOARD_HELD).is_some());
        }
    }

    #[test]
    fn success_nothing_and_cancel_are_silent() {
        let ok = Outcome::Converted {
            verified: true,
            layout_switched: true,
        };
        let inserted = Outcome::Inserted { verified: true };
        for outcome in [ok, inserted, Outcome::Nothing, Outcome::Cancelled] {
            assert_eq!(LastOperation::new(outcome, false).notice_code(), None);
        }
        // A conversion that left the previous clipboard held is the one exception.
        assert_eq!(
            LastOperation::new(ok, true).notice_code(),
            Some(CLIPBOARD_HELD)
        );
    }

    #[test]
    fn extraction_reads_only_the_requested_key() {
        let source = "  \"status.reason.A\": \"First.\",\n  \"status.reason.AB\": \"Second.\",\n";
        assert_eq!(extract(source, "A"), Some("First."));
        assert_eq!(extract(source, "AB"), Some("Second."));
        assert_eq!(extract(source, "B"), None);
        assert_eq!(extract("\"status.reason.E\": \"a\\\"b\",", "E"), None);
    }
}
