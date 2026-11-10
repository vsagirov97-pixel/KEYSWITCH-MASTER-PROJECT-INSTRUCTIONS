//! Write method W3: `SendInput` with `KEYEVENTF_UNICODE`.
//!
//! The writer refuses text containing control characters (`\n`, `\r`, `\t`, other C0/C1)
//! **before** any input is generated: in chat apps Enter sends the message, Tab moves focus.
//! Input is sent in batches; a caller-supplied check runs before every batch (KeySwitch will
//! re-verify the focus fingerprint there).

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
    if text.chars().any(char::is_control) {
        return Err(Refusal::ControlChar);
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
        // SAFETY: every INPUT built by this module is INPUT_KEYBOARD, so `ki` is the
        // initialised union member; other types are skipped.
        let ki = unsafe { i.Anonymous.ki };
        i.r#type == INPUT_KEYBOARD && (ki.wVk == VK_RETURN || ki.wScan == 0x0D || ki.wScan == 0x0A)
    })
}

/// Sends `text`; `before_batch` must return `true` for every batch to proceed.
pub fn send_unicode(text: &str, mut before_batch: impl FnMut() -> bool) -> Result<Sent, SendError> {
    let batches = build_batches(text).map_err(SendError::Refused)?;
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
    Ok(Sent {
        units: text.encode_utf16().count(),
        events,
        batches: batches.len(),
    })
}

/// Sends raw virtual-key down/up events (used only by the hotkey-delivery sub-check).
/// Returns the number of injected events.
pub fn send_vk_chord(vks: &[VIRTUAL_KEY]) -> u32 {
    let mut v: Vec<INPUT> = Vec::new();
    let mk = |vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    for &vk in vks {
        v.push(mk(vk, KEYBD_EVENT_FLAGS(0)));
    }
    for &vk in vks.iter().rev() {
        v.push(mk(vk, KEYEVENTF_KEYUP));
    }
    // SAFETY: `v` holds fully initialised keyboard INPUTs; cbSize is one INPUT.
    unsafe { SendInput(&v, std::mem::size_of::<INPUT>() as i32) }
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
