//! Write method W3: `SendInput` with `KEYEVENTF_UNICODE`.
//!
//! The writer refuses text containing control characters (`\n`, `\r`, `\t`, other C0/C1),
//! format characters (Cf: bidi overrides/isolates, zero-width, BOM...) and U+2028/U+2029
//! (WS-06) **before** any input is generated: in chat apps Enter sends the message, Tab moves
//! focus, bidi controls can disguise the visible text.
//! Input is sent in batches; a caller-supplied check runs before every batch (KeySwitch will
//! re-verify the focus fingerprint there).

use crate::policy::CharClass;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_RETURN,
};

/// Max UTF-16 units per batch (each unit = key down + key up = 2 INPUT events).
pub const BATCH_UNITS: usize = 32;

/// Why the writer refused before sending anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Text contains a control character (`\n`, `\r`, `\t`, ...).
    ControlChar,
    /// Text contains a format character (general category Cf), WS-06.
    FormatChar,
    /// Text contains U+2028 / U+2029 (categories Zl / Zp), WS-06.
    Separator,
    /// Nothing to write.
    Empty,
}

/// Failure while sending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendError {
    /// Refused up front; nothing was sent.
    Refused(Refusal),
    /// The pre-batch check failed after `batches_sent` batches.
    TargetLost {
        /// Batches already injected.
        batches_sent: usize,
    },
    /// `SendInput` injected fewer events than requested (UIPI, no input desktop, ...).
    Blocked {
        /// Events injected in total, including the short batch.
        injected: u32,
        /// Events requested in total up to and including the short batch.
        requested: u32,
    },
}

/// Result of a successful send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sent {
    /// UTF-16 units written.
    pub units: usize,
    /// INPUT events injected.
    pub events: u32,
    /// Number of batches.
    pub batches: usize,
}

/// Builds the INPUT events for `text`, grouped in batches that never split a surrogate pair.
pub fn build_batches(text: &str) -> Result<Vec<Vec<INPUT>>, Refusal> {
    if text.is_empty() {
        return Err(Refusal::Empty);
    }
    match crate::policy::first_forbidden(text) {
        Some(CharClass::Control) => return Err(Refusal::ControlChar),
        Some(CharClass::Format) => return Err(Refusal::FormatChar),
        Some(CharClass::Separator) => return Err(Refusal::Separator),
        None => {}
    }
    let mut batches: Vec<Vec<INPUT>> = Vec::new();
    let mut cur: Vec<INPUT> = Vec::new();
    let mut buf = [0u16; 2];
    for ch in text.chars() {
        let units = ch.encode_utf16(&mut buf);
        if cur.len() / 2 + units.len() > BATCH_UNITS {
            batches.push(std::mem::take(&mut cur));
        }
        for &u in units.iter() {
            cur.push(key(u, KEYEVENTF_UNICODE));
            cur.push(key(u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    Ok(batches)
}

fn key(unit: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// `true` if any event would produce Enter (VK_RETURN, or a Unicode CR/LF scan value).
pub fn contains_return(batches: &[Vec<INPUT>]) -> bool {
    batches.iter().flatten().any(|i| {
        if i.r#type != INPUT_KEYBOARD {
            return false;
        }
        // SAFETY: (WS-09) the type tag was checked just above, so `ki` is the union member
        // that was initialised for this INPUT; it is `Copy` plain data.
        let ki = unsafe { i.Anonymous.ki };
        ki.wVk == VK_RETURN || ki.wScan == 0x0D || ki.wScan == 0x0A
    })
}

/// Sends `text`; `before_batch` must return `true` for every batch to proceed.
pub fn send_unicode(text: &str, mut before_batch: impl FnMut() -> bool) -> Result<Sent, SendError> {
    let mut batches = build_batches(text).map_err(SendError::Refused)?;
    let result = send_batches(&batches, &mut before_batch);
    // The events carry the text (one UTF-16 unit each): overwrite them before freeing.
    for input in batches.iter_mut().flatten() {
        *input = INPUT::default();
    }
    std::hint::black_box(&batches);
    result.map(|events| Sent {
        units: text.encode_utf16().count(),
        events,
        batches: batches.len(),
    })
}

fn send_batches(
    batches: &[Vec<INPUT>],
    before_batch: &mut impl FnMut() -> bool,
) -> Result<u32, SendError> {
    let mut events = 0u32;
    let mut requested = 0u32;
    for (n, batch) in batches.iter().enumerate() {
        if !before_batch() {
            return Err(SendError::TargetLost { batches_sent: n });
        }
        requested += batch.len() as u32;
        // SAFETY: `batch` is a slice of fully initialised keyboard INPUTs and cbSize is the
        // size of one INPUT, as the API requires.
        let injected = unsafe { SendInput(batch, std::mem::size_of::<INPUT>() as i32) };
        events += injected;
        if injected as usize != batch.len() {
            return Err(SendError::Blocked {
                injected: events,
                requested,
            });
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_control_chars_before_building() {
        assert_eq!(build_batches("a\nb").err(), Some(Refusal::ControlChar));
        assert_eq!(build_batches("a\r").err(), Some(Refusal::ControlChar));
        assert_eq!(build_batches("\tb").err(), Some(Refusal::ControlChar));
        assert_eq!(build_batches("").err(), Some(Refusal::Empty));
        assert_eq!(build_batches("a\u{2028}").err(), Some(Refusal::Separator));
        assert_eq!(build_batches("a\u{202E}b").err(), Some(Refusal::FormatChar));
        assert_eq!(build_batches("a\u{2066}b").err(), Some(Refusal::FormatChar));
    }

    #[test]
    fn batches_do_not_split_surrogates() {
        let s = format!("{}\u{1F600}", "a".repeat(BATCH_UNITS - 1));
        let b = build_batches(&s).expect("valid text");
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].len(), 2 * (BATCH_UNITS - 1));
        assert_eq!(b[1].len(), 4);
        assert!(!contains_return(&b));
    }
}
