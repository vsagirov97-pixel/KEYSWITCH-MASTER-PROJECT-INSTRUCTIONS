//! Write method W1: `EM_REPLACESEL` (D-30: only for windows whose class is exactly `Edit`,
//! see [`crate::gate::w1_permitted`]; the caller must have checked that).
//!
//! WS-04: the message is sent with `SendMessageTimeoutW(SMTO_ABORTIFHUNG)` and a bounded
//! timeout, so a hung target cannot block KeySwitch. `EM_REPLACESEL` is a system-marshalled
//! message: the system copies the buffer into the target process (no injection, no shared
//! memory), so an abandoned call cannot leave the target reading our memory.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Controls::{
    EM_GETLINE, EM_GETSEL, EM_LINEFROMCHAR, EM_LINEINDEX, EM_LINELENGTH, EM_REPLACESEL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETTEXTLENGTH,
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
/// this way (timeout, text longer than [`VERIFY_MAX_UNITS`], caret outside the text, more
/// than [`VERIFY_MAX_LINES`] lines, an unexpected line-break shape); the caller then falls
/// back to UI Automation.
///
/// QA-B1-08: only the lines that cover `[caret - len(expected), caret)` are read
/// (`EM_LINEFROMCHAR` / `EM_LINEINDEX` / `EM_LINELENGTH` / `EM_GETLINE`), and of the last
/// line only the part before the caret — never the whole field. Every copy is overwritten
/// before it is freed.
pub fn text_before_caret_is(h: Hwnd, expected: &str) -> Option<bool> {
    let sel = send(h, EM_GETSEL, 0, 0)?;
    if sel as u32 == u32::MAX {
        return None;
    }
    let caret = (sel >> 16) & 0xFFFF;
    let mut want: Vec<u16> = expected.encode_utf16().collect();
    let result = tail_matches(h, caret, &want);
    // The expected text is user text: overwrite it before freeing.
    want.fill(0);
    std::hint::black_box(&want);
    result
}

/// Upper bound of lines the W1 post-check reads; a longer tail goes to the UIA check.
pub const VERIFY_MAX_LINES: usize = 256;

const CR: u16 = 0x0D;
const LF: u16 = 0x0A;

/// Compares the `want.len()` units before `caret` with `want`, line by line.
fn tail_matches(h: Hwnd, caret: usize, want: &[u16]) -> Option<bool> {
    let len = send(h, WM_GETTEXTLENGTH, 0, 0)?;
    if len > VERIFY_MAX_UNITS || caret > len {
        return None;
    }
    if want.len() > caret {
        return Some(false);
    }
    if want.is_empty() {
        return Some(true);
    }
    let start = caret - want.len();
    let first = send(h, EM_LINEFROMCHAR, start, 0)?;
    let last = send(h, EM_LINEFROMCHAR, caret, 0)?;
    if last < first || last - first >= VERIFY_MAX_LINES {
        return None;
    }
    // End (exclusive) of the previous line's characters.
    let mut prev_end: Option<usize> = None;
    for line in first..=last {
        let idx = send(h, EM_LINEINDEX, line, 0)?;
        if idx > len {
            // -1: no such line (the text changed under us).
            return None;
        }
        let line_end = idx.checked_add(send(h, EM_LINELENGTH, idx, 0)?)?;
        if line_end > len {
            return None;
        }
        match prev_end {
            // The first line must contain the start of the compared range.
            None if idx > start => return None,
            None => {}
            Some(pe) if idx < pe => return None,
            Some(pe) => match break_matches(pe, idx, start, caret, want) {
                Some(true) => {}
                other => return other,
            },
        }
        let (from, to) = (start.max(idx), caret.min(line_end));
        if to > from && !line_part_matches(h, line, idx, from, to, &want[from - start..to - start])?
        {
            return Some(false);
        }
        prev_end = Some(line_end);
    }
    // The caret sits inside a line break (between `\r` and `\n`): not decidable here.
    if prev_end.is_some_and(|pe| caret > pe) {
        return None;
    }
    Some(true)
}

/// The line break between the end of one line (`pe`) and the start of the next (`idx`):
/// 0 units (soft wrap), 1 (`\r` or `\n`) or 2 (`\r\n`). Checks the part of it that lies in
/// `[start, caret)` against `want`. `None` for any other shape.
fn break_matches(pe: usize, idx: usize, start: usize, caret: usize, want: &[u16]) -> Option<bool> {
    let gap = idx - pe;
    if gap > 2 {
        return None;
    }
    let ok = (pe.max(start)..idx.min(caret)).all(|p| {
        let c = want[p - start];
        match gap {
            1 => c == CR || c == LF,
            _ => c == if p == pe { CR } else { LF },
        }
    });
    Some(ok)
}

/// Reads units `[idx, to)` of line `line` (which starts at `idx`) with `EM_GETLINE` and
/// compares `[from, to)` with `want`. The copy is overwritten before it is freed.
fn line_part_matches(
    h: Hwnd,
    line: usize,
    idx: usize,
    from: usize,
    to: usize,
    want: &[u16],
) -> Option<bool> {
    let units = to - idx;
    // EM_GETLINE: the first unit of the buffer holds its size; `units <= VERIFY_MAX_UNITS`.
    let size = u16::try_from(units).ok()?;
    let mut buf = vec![0u16; units];
    buf[0] = size;
    // EM_GETLINE is marshalled by the system across processes (standard Edit class); the
    // buffer is alive and `units` long for the whole synchronous call, and the control
    // copies at most the size written into its first unit.
    let copied = send(h, EM_GETLINE, line, buf.as_mut_ptr() as isize);
    let result = copied.map(|n| n >= units && buf[from - idx..] == want[..]);
    // The buffer held user text: overwrite it before freeing.
    buf.fill(0);
    std::hint::black_box(&buf);
    result
}
