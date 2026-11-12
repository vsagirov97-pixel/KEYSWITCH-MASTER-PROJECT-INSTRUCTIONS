//! Write method W1: `EM_REPLACESEL` (D-30: only for windows whose class is exactly `Edit`,
//! see [`crate::gate::w1_permitted`]; the caller must have checked that).
//!
//! WS-04: the message is sent with `SendMessageTimeoutW(SMTO_ABORTIFHUNG)` and a bounded
//! timeout, so a hung target cannot block KeySwitch. `EM_REPLACESEL` is a system-marshalled
//! message: the system copies the buffer into the target process (no injection, no shared
//! memory), so an abandoned call cannot leave the target reading our memory.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Controls::EM_REPLACESEL;
use windows::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW};

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
