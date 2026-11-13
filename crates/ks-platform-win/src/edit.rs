//! Write method W1: `EM_REPLACESEL` (D-30: only for windows whose class is exactly `Edit`,
//! see [`crate::gate::w1_permitted`]; the caller must have checked that).
//!
//! WS-04: the message is sent with `SendMessageTimeoutW(SMTO_ABORTIFHUNG)` and a bounded
//! timeout, so a hung target cannot block KeySwitch. `EM_REPLACESEL` is a system-marshalled
//! message: the system copies the buffer into the target process (no injection, no shared
//! memory), so an abandoned call cannot leave the target reading our memory.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Controls::{EM_GETSEL, EM_REPLACESEL};
use windows::Win32::UI::WindowsAndMessaging::{
    SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETTEXT, WM_GETTEXTLENGTH,
};

use crate::Hwnd;

/// Timeout of the W1 call.
pub const SEND_TIMEOUT_MS: u32 = 2000;

/// Replaces the current selection of edit control `h` with `text` (undoable in the target).
/// `false` if the call timed out or the target is hung.
pub fn replace_selection(h: Hwnd, text: &str) -> bool {
    let mut buf: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let mut result = 0usize;
    // SAFETY: `buf` is NUL-terminated and alive until the synchronous call returns; `result`
    // is a valid out-parameter; EM_REPLACESEL is marshalled by the system across processes.
    let ok = unsafe {
        SendMessageTimeoutW(
            h.raw(),
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(buf.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            SEND_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    // The buffer held user text: overwrite it before freeing.
    buf.fill(0);
    std::hint::black_box(&buf);
    ok.0 != 0
}

/// Upper bound of the text W1 post-check reads back (UTF-16 units). `EM_GETSEL` reports
/// positions in 16 bits through its return value, so longer texts are not checked here.
pub const VERIFY_MAX_UNITS: usize = 0xFFFF;

/// One bounded, system-marshalled message to `h`; `None` on timeout or a hung target.
fn send(h: Hwnd, msg: u32, wparam: usize, lparam: isize) -> Option<usize> {
    let mut result = 0usize;
    // SAFETY: plain message without pointers unless the caller passes a live buffer that
    // outlives this synchronous call; `result` is a valid out-parameter.
    let ok = unsafe {
        SendMessageTimeoutW(
            h.raw(),
            msg,
            WPARAM(wparam),
            LPARAM(lparam),
            SMTO_ABORTIFHUNG,
            SEND_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    (ok.0 != 0).then_some(result)
}

/// W1 post-check in the edit control's own units (`\r\n` = 2): `Some(true)` if the text
/// right before the caret (`EM_GETSEL` end) equals `expected`. `None` if it cannot be read
/// (timeout, text longer than [`VERIFY_MAX_UNITS`], caret outside the text). The copy of the
/// field's text is overwritten before it is freed.
pub fn text_before_caret_is(h: Hwnd, expected: &str) -> Option<bool> {
    let sel = send(h, EM_GETSEL, 0, 0)?;
    if sel as u32 == u32::MAX {
        return None;
    }
    let caret = (sel >> 16) & 0xFFFF;
    let len = send(h, WM_GETTEXTLENGTH, 0, 0)?;
    if len > VERIFY_MAX_UNITS || caret > len {
        return None;
    }
    let want: Vec<u16> = expected.encode_utf16().collect();
    let mut buf = vec![0u16; len + 1];
    // WM_GETTEXT is marshalled by the system across processes (standard Edit class); the
    // buffer is alive and `len + 1` units long for the whole synchronous call.
    let copied = send(h, WM_GETTEXT, buf.len(), buf.as_mut_ptr() as isize);
    let result = copied.map(|n| {
        let n = n.min(len);
        caret <= n && caret >= want.len() && buf[caret - want.len()..caret] == want[..]
    });
    // Both buffers held user text: overwrite them before freeing.
    buf.fill(0);
    std::hint::black_box(&buf);
    let mut want = want;
    want.fill(0);
    std::hint::black_box(&want);
    result
}
