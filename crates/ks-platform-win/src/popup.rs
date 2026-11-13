//! Language-choice popup for low-confidence and mixed text.
//!
//! A small topmost window that never takes activation (`WS_EX_NOACTIVATE`, shown with
//! `SW_SHOWNOACTIVATE`), placed near the caret (or the mouse cursor). The focus stays in the
//! target, so the target fingerprint does not change. Keys `1`, `2` and `Esc` are temporary
//! `RegisterHotKey` registrations of the calling thread (no keyboard hook, D-14); they exist
//! only while the popup is open, at most [`TIMEOUT`].
//!
//! The popup shows language codes only — never the user's text.
//!
//! [`notice`] reuses the same kind of window for a brief refusal / error message.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{ClientToScreen, DEFAULT_GUI_FONT, GetStockObject};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GUITHREADINFO, GetCursorPos,
    GetGUIThreadInfo, GetMessageW, KillTimer, MSG, PM_REMOVE, PeekMessageW, SW_SHOWNOACTIVATE,
    SendMessageW, SetTimer, ShowWindow, WINDOW_STYLE, WM_HOTKEY, WM_SETFONT, WM_TIMER, WS_BORDER,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::Hwnd;
use crate::gate;

/// The popup closes itself (= cancel) after this long.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// How often the popup re-checks the timeout and the target.
const TICK_MS: u32 = 100;
const WIDTH: i32 = 240;
const HEIGHT: i32 = 30;
/// `SS_CENTER | SS_CENTERIMAGE`: text centred horizontally and vertically.
const STATIC_CENTERED: u32 = 0x0001 | 0x0200;
/// Hotkey ids, private to the popup's thread.
const ID_FIRST: i32 = 0x4B01;
const ID_SECOND: i32 = 0x4B02;
const ID_ESC: i32 = 0x4B03;

/// The user's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Key `1` (index 0) or `2` (index 1).
    Picked(usize),
    /// Esc, timeout, or `still_valid` turned false.
    Cancelled,
    /// The popup could not be shown or a key could not be registered.
    Unavailable,
}

static CURRENT: AtomicIsize = AtomicIsize::new(0);

/// The popup window while one is open (for tests and diagnostics).
pub fn current() -> Option<Hwnd> {
    let h = Hwnd::from_isize(CURRENT.load(Ordering::SeqCst));
    (!h.is_null()).then_some(h)
}

/// Screen point just below the caret of `focus`'s thread, or the mouse cursor.
fn anchor(focus: Hwnd) -> (i32, i32) {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a properly sized, writable GUITHREADINFO.
    let ok = unsafe { GetGUIThreadInfo(gate::window_thread(focus), &mut info) }.is_ok();
    let caret: RECT = info.rcCaret;
    if ok && !info.hwndCaret.0.is_null() {
        let mut p = POINT {
            x: caret.left,
            y: caret.bottom,
        };
        // SAFETY: `p` is a valid in/out point; a stale window handle only makes it fail.
        if unsafe { ClientToScreen(info.hwndCaret, &mut p) }.as_bool() {
            return (p.x, p.y + 4);
        }
    }
    let mut p = POINT::default();
    // SAFETY: `p` is a valid out-parameter.
    if unsafe { GetCursorPos(&mut p) }.is_ok() {
        return (p.x + 12, p.y + 16);
    }
    (40, 40)
}

/// Shows the popup for two language codes and waits for `1`, `2` or `Esc` on the calling
/// thread. `still_valid` is polled every 100 ms; `false` cancels (target or generation
/// changed). Every exit path removes the window and the key registrations.
pub fn ask(
    labels: &[&str],
    focus: Hwnd,
    timeout: Duration,
    still_valid: &dyn Fn() -> bool,
) -> Answer {
    let [first, second] = labels else {
        return Answer::Unavailable;
    };
    // Language codes only (`RU`, `EN`): metadata, not user text.
    let caption: Vec<u16> = format!(
        "1 \u{2014} {}      2 \u{2014} {}      Esc",
        first.to_uppercase(),
        second.to_uppercase()
    )
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
    let (x, y) = anchor(focus);
    // SAFETY: system class `STATIC`; `caption` is NUL-terminated and outlives the call; no
    // parent, menu or creation data. The window belongs to the calling thread.
    let created = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("STATIC"),
            PCWSTR(caption.as_ptr()),
            WS_POPUP | WS_BORDER | WINDOW_STYLE(STATIC_CENTERED),
            x,
            y,
            WIDTH,
            HEIGHT,
            None,
            None,
            None,
            None,
        )
    };
    let Ok(hwnd) = created else {
        return Answer::Unavailable;
    };
    // SAFETY: stock objects are owned by the system and never freed; WM_SETFONT on our own
    // window takes the font handle by value.
    unsafe {
        let font = GetStockObject(DEFAULT_GUI_FONT);
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(windows::Win32::Foundation::WPARAM(font.0 as usize)),
            Some(windows::Win32::Foundation::LPARAM(1)),
        );
    }
    let keys = [
        (ID_FIRST, u32::from(b'1')),
        (ID_SECOND, u32::from(b'2')),
        (ID_ESC, u32::from(VK_ESCAPE.0)),
    ];
    let mut registered = 0;
    for (id, vk) in keys {
        // SAFETY: thread-bound registration (NULL window): WM_HOTKEY is posted to this
        // thread's queue; unregistered below on every path.
        if unsafe { RegisterHotKey(None, id, MOD_NOREPEAT, vk) }.is_err() {
            break;
        }
        registered += 1;
    }
    let mut answer = Answer::Unavailable;
    if registered == keys.len() {
        CURRENT.store(Hwnd::from_raw(hwnd).as_isize(), Ordering::SeqCst);
        // SAFETY: our own window; SW_SHOWNOACTIVATE never changes the active window. A
        // thread timer (NULL window) posts WM_TIMER to this thread's queue.
        let timer = unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetTimer(None, 0, TICK_MS, None)
        };
        answer = wait(timer, timeout, still_valid);
        // SAFETY: the timer id returned by SetTimer for this thread.
        let _ = unsafe { KillTimer(None, timer) };
    }
    for (id, _) in keys.iter().take(registered) {
        // SAFETY: ids registered above by this thread.
        let _ = unsafe { UnregisterHotKey(None, *id) };
    }
    CURRENT.store(0, Ordering::SeqCst);
    // SAFETY: our own window, destroyed exactly once; then leftover WM_HOTKEY messages of
    // the registrations above are removed from this thread's queue.
    unsafe {
        let _ = DestroyWindow(hwnd);
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE).as_bool() {}
    }
    answer
}

fn wait(timer: usize, timeout: Duration, still_valid: &dyn Fn() -> bool) -> Answer {
    let start = Instant::now();
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a valid MSG buffer owned by this frame.
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if r == 0 || r == -1 {
            return Answer::Cancelled;
        }
        if msg.hwnd.0.is_null() && msg.message == WM_HOTKEY {
            match msg.wParam.0 as i32 {
                ID_FIRST => return Answer::Picked(0),
                ID_SECOND => return Answer::Picked(1),
                ID_ESC => return Answer::Cancelled,
                _ => {}
            }
        } else if msg.hwnd.0.is_null() && msg.message == WM_TIMER && msg.wParam.0 == timer {
            if start.elapsed() >= timeout || !still_valid() {
                return Answer::Cancelled;
            }
        } else {
            // SAFETY: `msg` was filled by GetMessageW above.
            unsafe { DispatchMessageW(&msg) };
        }
    }
}

/// How long a [`notice`] stays on screen.
pub const NOTICE: Duration = Duration::from_millis(2500);

/// Shows `text` for `duration` in a small topmost window that never takes activation, near
/// the caret of the foreground window (or the mouse cursor). Returns at once: the window
/// lives on its own short-lived thread. `text` is a reason message chosen by the caller —
/// never user text.
pub fn notice(text: &str, duration: Duration) {
    let caption: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let chars = i32::try_from(text.chars().count()).unwrap_or(i32::MAX / 8);
    let width = (chars.saturating_mul(7) + 28).clamp(120, 620);
    let millis = u32::try_from(duration.as_millis()).unwrap_or(u32::MAX);
    let _ = std::thread::Builder::new()
        .name("ks-notice".into())
        .spawn(move || {
            // A NULL handle means "the foreground thread" for the caret lookup.
            let (x, y) = anchor(Hwnd::from_isize(0));
            // SAFETY: system class `STATIC`; `caption` is NUL-terminated and outlives the
            // call; no parent, menu or creation data. The window belongs to this thread.
            let created = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                    w!("STATIC"),
                    PCWSTR(caption.as_ptr()),
                    WS_POPUP | WS_BORDER | WINDOW_STYLE(STATIC_CENTERED),
                    x,
                    y,
                    width,
                    HEIGHT,
                    None,
                    None,
                    None,
                    None,
                )
            };
            let Ok(hwnd) = created else {
                return;
            };
            // SAFETY: stock font handle passed by value to our own window; SW_SHOWNOACTIVATE
            // never changes the active window; a thread timer posts WM_TIMER to this queue.
            let timer = unsafe {
                let font = GetStockObject(DEFAULT_GUI_FONT);
                SendMessageW(
                    hwnd,
                    WM_SETFONT,
                    Some(windows::Win32::Foundation::WPARAM(font.0 as usize)),
                    Some(windows::Win32::Foundation::LPARAM(1)),
                );
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                SetTimer(None, 0, millis, None)
            };
            let mut msg = MSG::default();
            loop {
                // SAFETY: `msg` is a valid MSG buffer owned by this frame.
                let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
                if r == 0 || r == -1 {
                    break;
                }
                if msg.hwnd.0.is_null() && msg.message == WM_TIMER && msg.wParam.0 == timer {
                    break;
                }
                // SAFETY: `msg` was filled by GetMessageW above.
                unsafe { DispatchMessageW(&msg) };
            }
            // SAFETY: the timer and the window were created above by this thread.
            unsafe {
                let _ = KillTimer(None, timer);
                let _ = DestroyWindow(hwnd);
            }
        });
}
