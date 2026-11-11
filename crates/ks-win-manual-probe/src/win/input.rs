//! W3: `SendInput(KEYEVENTF_UNICODE)` into the tool's OWN window only.
//!
//! - text is refused before any input is built if it contains a forbidden character
//!   (policy::is_forbidden_char: controls, Zl/Zp, Cf — WS-06);
//! - there is deliberately no virtual-key sender in this tool (WS-07);
//! - before every batch the caller's check must confirm the target (our foreground window,
//!   our focused control, no modifier keys held — WS-05).

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY,
};

use crate::policy;

/// UTF-16 units per batch.
pub const BATCH_UNITS: usize = 32;

/// Outcome of a send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendResult {
    /// Everything sent.
    Sent {
        /// Events injected.
        events: u32,
        /// Batches.
        batches: usize,
    },
    /// Refused by the text policy; nothing sent.
    Refused,
    /// Pre-batch check failed after `batches_sent` batches.
    TargetLost {
        /// Batches already sent.
        batches_sent: usize,
    },
    /// SendInput injected fewer events than requested.
    Blocked {
        /// Events injected in total.
        injected: u32,
    },
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

fn batches(text: &str) -> Option<Vec<Vec<INPUT>>> {
    if !policy::is_writable_text(text) {
        return None;
    }
    let mut out: Vec<Vec<INPUT>> = Vec::new();
    let mut cur: Vec<INPUT> = Vec::new();
    let mut buf = [0u16; 2];
    for ch in text.chars() {
        let units = ch.encode_utf16(&mut buf);
        if cur.len() / 2 + units.len() > BATCH_UNITS {
            out.push(std::mem::take(&mut cur));
        }
        for &u in units.iter() {
            cur.push(key(u, KEYEVENTF_UNICODE));
            cur.push(key(u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Some(out)
}

/// Sends synthetic `text`; `check` runs before every batch and must return `true`.
pub fn send_unicode(text: &str, mut check: impl FnMut() -> bool) -> SendResult {
    let Some(bs) = batches(text) else {
        return SendResult::Refused;
    };
    let mut events = 0u32;
    for (n, b) in bs.iter().enumerate() {
        if !check() {
            return SendResult::TargetLost { batches_sent: n };
        }
        // SAFETY: `b` holds fully initialised keyboard INPUTs; cbSize is one INPUT.
        let injected = unsafe { SendInput(b, std::mem::size_of::<INPUT>() as i32) };
        events += injected;
        if injected as usize != b.len() {
            return SendResult::Blocked { injected: events };
        }
    }
    SendResult::Sent {
        events,
        batches: bs.len(),
    }
}
