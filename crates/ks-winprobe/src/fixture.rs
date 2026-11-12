//! Win32 test fixture: a top-level window owned by a dedicated thread (with its own
//! message loop) that hosts classic `EDIT` controls and a `RICHEDIT50W` control.
//!
//! S-2a: all controls live in the test process. S-2b: the same fixture also runs in a
//! separate process (`src/bin/ks-winprobe-fixture.rs`); the free functions below address a
//! fixture by window handle, so they work across the process boundary too.
//!
//! WS-04: every message is sent with `SendMessageTimeoutW(SMTO_ABORTIFHUNG)` and a bounded
//! timeout — a hung target can never block the caller indefinitely.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Once, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId, Sleep};
use windows::Win32::UI::Controls::RichEdit::MSFTEDIT_CLASS;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_REPLACESEL, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, GetKeyState, SetFocus, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DispatchMessageW, ES_AUTOHSCROLL,
    ES_AUTOVSCROLL, ES_MULTILINE, ES_PASSWORD, ES_WANTRETURN, GetForegroundWindow, GetMessageW,
    GetWindowThreadProcessId, HMENU, KillTimer, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    SMTO_ABORTIFHUNG, SW_SHOW, SendMessageTimeoutW, SetForegroundWindow, SetTimer, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLIPBOARDUPDATE, WM_CLOSE,
    WM_DESTROY, WM_GETTEXT, WM_GETTEXTLENGTH, WM_KEYDOWN, WM_NULL, WM_RENDERALLFORMATS,
    WM_RENDERFORMAT, WM_SETTEXT, WM_TIMER, WNDCLASSW, WS_BORDER, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

/// Window handle that can cross threads. Plain `HWND` holds a raw pointer and is `!Send`;
/// an HWND is only a system-wide identifier, so passing its value between threads is fine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hwnd(isize);

impl Hwnd {
    /// Wraps a raw handle.
    pub fn from_raw(h: HWND) -> Self {
        Self(h.0 as isize)
    }
    /// The raw handle for Win32 calls.
    pub fn raw(self) -> HWND {
        HWND(self.0 as *mut c_void)
    }
    /// Handle value as an integer (to pass it between processes; not a pointer).
    pub fn as_isize(self) -> isize {
        self.0
    }
    /// Handle from an integer value received from another process.
    pub fn from_isize(v: isize) -> Self {
        Self(v)
    }
    /// `true` for the NULL handle.
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// Handles of the fixture controls.
#[derive(Clone, Copy, Debug)]
pub struct Controls {
    /// Top-level window.
    pub main: Hwnd,
    /// `EDIT`, single line.
    pub single: Hwnd,
    /// `EDIT`, `ES_MULTILINE | ES_WANTRETURN`.
    pub multi: Hwnd,
    /// `EDIT`, `ES_PASSWORD`.
    pub password: Hwnd,
    /// `RICHEDIT50W` (Msftedit.dll), multi-line; `None` if the DLL could not be loaded.
    pub rich: Option<Hwnd>,
}

/// How the fixture obtained the foreground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foreground {
    /// Not foreground; `GetForegroundWindow` returned NULL (typical without a desktop).
    NoForegroundWindow,
    /// Not foreground; another window kept it (foreground lock).
    Denied,
    /// `SetForegroundWindow` alone succeeded (or the window already was foreground).
    Direct,
    /// Succeeded only after `AttachThreadInput` to the current foreground thread.
    /// TEST-FIXTURE-ONLY workaround; never to be used by product code.
    ViaAttachThreadInput,
}

impl Foreground {
    /// Fixture window is the foreground window.
    pub fn ok(self) -> bool {
        matches!(self, Foreground::Direct | Foreground::ViaAttachThreadInput)
    }
    /// Stable label for reports.
    pub fn label(self) -> &'static str {
        match self {
            Foreground::NoForegroundWindow => "no_foreground_window",
            Foreground::Denied => "denied",
            Foreground::Direct => "direct",
            Foreground::ViaAttachThreadInput => "attach_thread_input",
        }
    }
}

const MSG_FOCUS: u32 = WM_APP + 1;
const MSG_FOREGROUND: u32 = WM_APP + 2;
/// Posted: the fixture GUI thread sleeps `wparam` ms (P-16 / XR-04 hung target).
const MSG_HANG: u32 = WM_APP + 3;
/// Sent: the fixture takes clipboard ownership with a delayed-rendered CF_UNICODETEXT;
/// returns the clipboard sequence number read under `OpenClipboard` (0 = failed).
const MSG_OWN_DELAYED: u32 = WM_APP + 4;
const CLASS_NAME: PCWSTR = w!("KsWinProbeFixture");
const LATE_TIMER: usize = 1;

/// Default timeout for messages to fixture windows (WS-04).
pub const SEND_TIMEOUT_MS: u32 = 2000;

/// Extra behaviour of a fixture running in the separate fixture process (S-2b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ChildMode {
    /// Plain controls (default; the in-process fixture always uses this).
    Plain = 0,
    /// XR-02: on Ctrl+V (WM_KEYDOWN 'V' with Ctrl) the window reads CF_UNICODETEXT only
    /// after [`LATE_READ_MS`] and reports `late_read units=N`.
    LatePaste = 1,
    /// XR-01(a): a clipboard format listener that immediately reads CF_UNICODETEXT on every
    /// update (forcing delayed rendering) and reports `listener_read units=N`.
    Listener = 2,
    /// XR-04: owns a delayed-rendered CF_UNICODETEXT (on request) and sleeps
    /// [`HUNG_RENDER_MS`] inside WM_RENDERFORMAT before rendering.
    HungOwner = 3,
}

/// XR-02 late reader delay.
pub const LATE_READ_MS: u32 = 1500;
/// XR-04 hung owner: time spent inside WM_RENDERFORMAT.
pub const HUNG_RENDER_MS: u32 = 3000;

static MODE: AtomicU8 = AtomicU8::new(0);
static EVENT_SINK: OnceLock<fn(&str)> = OnceLock::new();

/// Sets the child behaviour; must be called before [`Fixture::spawn`]. Process-wide.
pub fn set_child_mode(mode: ChildMode) {
    MODE.store(mode as u8, Ordering::SeqCst);
}

fn mode() -> u8 {
    MODE.load(Ordering::SeqCst)
}

/// Installs the sink for child events (metadata-only lines). Process-wide, set once.
pub fn set_event_sink(sink: fn(&str)) {
    let _ = EVENT_SINK.set(sink);
}

fn event(line: &str) {
    if let Some(sink) = EVENT_SINK.get() {
        sink(line);
    }
}

/// Running fixture; dropping it closes the window and joins the thread.
#[derive(Debug)]
pub struct Fixture {
    /// Control handles.
    pub controls: Controls,
    /// Id of the fixture (GUI) thread.
    pub thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl Fixture {
    /// Creates the window on a new thread and waits until it is ready.
    pub fn spawn() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<Result<(Controls, u32), String>>();
        let join = std::thread::Builder::new()
            .name("ks-winprobe-fixture".into())
            .spawn(move || match create_windows() {
                Ok(c) => {
                    // SAFETY: trivial call returning the id of the current thread.
                    let tid = unsafe { GetCurrentThreadId() };
                    let _ = tx.send(Ok((c, tid)));
                    message_loop();
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            })
            .map_err(|e| format!("spawn fixture thread: {e}"))?;
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok((controls, thread_id))) => Ok(Self {
                controls,
                thread_id,
                join: Some(join),
            }),
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => Err("fixture thread did not report readiness in 10 s".into()),
        }
    }

    /// Tries to make the fixture the foreground window (performed on the fixture thread).
    pub fn bring_to_foreground(&self) -> Foreground {
        request_foreground(self.controls.main)
    }

    /// `true` if the fixture's top-level window is currently the foreground window.
    pub fn is_foreground(&self) -> bool {
        is_foreground(self.controls.main)
    }

    /// Sets keyboard focus to `control` (on the fixture thread); `true` if focus is there now.
    pub fn focus(&self, control: Hwnd) -> bool {
        request_focus(self.controls.main, control)
    }

    /// Closes the window and joins the thread.
    fn shutdown(&mut self) {
        // SAFETY: posting WM_CLOSE to our own window; a stale handle only makes the call fail.
        let _ = unsafe {
            PostMessageW(
                Some(self.controls.main.raw()),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            )
        };
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Asks the fixture whose top-level window is `main` (any process) to take the foreground.
/// The work (including the fixture-only `AttachThreadInput` workaround) runs on the
/// fixture's own GUI thread.
pub fn request_foreground(main: Hwnd) -> Foreground {
    // SAFETY: private message with integer-only parameters.
    match unsafe { send(main, MSG_FOREGROUND, 0, 0) } {
        Some(1) => Foreground::Direct,
        Some(2) => Foreground::ViaAttachThreadInput,
        Some(3) => Foreground::NoForegroundWindow,
        _ => Foreground::Denied,
    }
}

/// Asks the fixture `main` to focus its child `control` (or itself); `true` on success.
pub fn request_focus(main: Hwnd, control: Hwnd) -> bool {
    // SAFETY: private message; wparam is a window handle value, not dereferenced by us.
    unsafe { send(main, MSG_FOCUS, control.0 as usize, 0) == Some(1) }
}

/// `true` if `main` is the foreground window.
pub fn is_foreground(main: Hwnd) -> bool {
    // SAFETY: trivial query without arguments.
    Hwnd::from_raw(unsafe { GetForegroundWindow() }) == main
}

/// Posts a "sleep `ms` on your GUI thread" request (P-16 / XR-04). `true` if posted.
pub fn post_hang(main: Hwnd, ms: u32) -> bool {
    // SAFETY: posting a private integer-only message; a stale handle only makes it fail.
    unsafe { PostMessageW(Some(main.raw()), MSG_HANG, WPARAM(ms as usize), LPARAM(0)) }.is_ok()
}

/// Asks a [`ChildMode::HungOwner`] fixture to own a delayed-rendered CF_UNICODETEXT.
/// Returns the sequence number it read under `OpenClipboard`, `None` on failure.
pub fn request_own_delayed(main: Hwnd) -> Option<u32> {
    // SAFETY: private integer-only message.
    unsafe { send(main, MSG_OWN_DELAYED, 0, 0) }
        .filter(|&s| s != 0)
        .map(|s| s as u32)
}

/// `WM_NULL` with `SMTO_ABORTIFHUNG` and `timeout_ms`: `true` if the window's thread
/// answered in time (responsiveness ping, P-16 / XR-04).
pub fn ping(h: Hwnd, timeout_ms: u32) -> bool {
    // SAFETY: WM_NULL carries no parameters.
    unsafe { send_timeout(h, WM_NULL, 0, 0, timeout_ms) }.is_some()
}

/// `WM_GETTEXTLENGTH` with an explicit timeout; `None` on timeout / hung target (P-16).
pub fn text_len_timeout(h: Hwnd, timeout_ms: u32) -> Option<usize> {
    // SAFETY: integer-only message.
    unsafe { send_timeout(h, WM_GETTEXTLENGTH, 0, 0, timeout_ms) }.map(|n| n.max(0) as usize)
}

/// `SendMessageTimeoutW(SMTO_ABORTIFHUNG)` with [`SEND_TIMEOUT_MS`] (WS-04).
///
/// # Safety
/// If `msg` interprets `wparam`/`lparam` as pointers, they must point to buffers that are
/// valid for that message and outlive the call. Only integer messages or
/// system-marshalled messages (`WM_GETTEXT`, `WM_SETTEXT`, `EM_REPLACESEL`) may carry
/// pointers: the system copies their buffers, so an abandoned (timed-out) call cannot
/// leave the target writing into our memory.
unsafe fn send(h: Hwnd, msg: u32, wparam: usize, lparam: isize) -> Option<isize> {
    // SAFETY: forwarded from the caller's contract.
    unsafe { send_timeout(h, msg, wparam, lparam, SEND_TIMEOUT_MS) }
}

/// # Safety
/// Same contract as [`send`].
unsafe fn send_timeout(
    h: Hwnd,
    msg: u32,
    wparam: usize,
    lparam: isize,
    timeout_ms: u32,
) -> Option<isize> {
    let mut result = 0usize;
    // SAFETY: `result` is a valid out-parameter for the duration of the call; pointer
    // parameters follow the caller's contract.
    let ok = unsafe {
        SendMessageTimeoutW(
            h.raw(),
            msg,
            WPARAM(wparam),
            LPARAM(lparam),
            SMTO_ABORTIFHUNG,
            timeout_ms,
            Some(&mut result),
        )
    };
    (ok.0 != 0).then_some(result as isize)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `WM_SETTEXT` (resets the undo buffer of Edit controls).
pub fn set_text(h: Hwnd, text: &str) -> bool {
    let buf = wide(text);
    // SAFETY: `buf` is NUL-terminated and alive until the synchronous call returns;
    // WM_SETTEXT is marshalled by the system across processes.
    unsafe { send(h, WM_SETTEXT, 0, buf.as_ptr() as isize) }.is_some_and(|r| r != 0)
}

/// `WM_GETTEXT` of the whole control. Test-only: used on synthetic content.
/// A timed-out call yields an empty string (callers compare against expected content).
pub fn get_text(h: Hwnd) -> String {
    let len = text_len(h);
    let mut buf = vec![0u16; len + 1];
    // SAFETY: `buf` has room for `buf.len()` units (the count passed in wparam) and
    // outlives the synchronous call; WM_GETTEXT is marshalled across processes.
    let n = unsafe { send(h, WM_GETTEXT, buf.len(), buf.as_mut_ptr() as isize) }
        .unwrap_or(0)
        .max(0) as usize;
    String::from_utf16_lossy(&buf[..n.min(len)])
}

/// Length in UTF-16 units (`WM_GETTEXTLENGTH`), without reading the text. 0 on timeout.
pub fn text_len(h: Hwnd) -> usize {
    // SAFETY: integer-only message.
    unsafe { send(h, WM_GETTEXTLENGTH, 0, 0) }
        .unwrap_or(0)
        .max(0) as usize
}

/// `EM_SETSEL(start, end)`; `end = -1` selects to the end.
pub fn set_sel(h: Hwnd, start: i32, end: i32) {
    // SAFETY: integer-only message (sign-extended positions, as the API expects).
    unsafe { send(h, EM_SETSEL, start as isize as usize, end as isize) };
}

/// `EM_GETSEL` → (start, end) in UTF-16 units, decoded from the packed return value
/// (no pointer parameters, so it is safe across processes and on timeout). Positions above
/// 65535 are not representable this way; fixtures stay far below that.
pub fn get_sel(h: Hwnd) -> (u32, u32) {
    // SAFETY: wparam = lparam = 0 (NULL pointers): the result comes in the return value.
    let r = unsafe { send(h, EM_GETSEL, 0, 0) }.unwrap_or(0) as usize as u32;
    (r & 0xFFFF, r >> 16)
}

/// `EM_REPLACESEL(fCanUndo, text)` — write method W1. `false` if the call timed out.
pub fn replace_sel(h: Hwnd, text: &str, can_undo: bool) -> bool {
    let buf = wide(text);
    // SAFETY: `buf` is NUL-terminated and alive until the synchronous call returns;
    // EM_REPLACESEL is marshalled by the system across processes.
    unsafe {
        send(
            h,
            EM_REPLACESEL,
            usize::from(can_undo),
            buf.as_ptr() as isize,
        )
    }
    .is_some()
}

/// Sends an integer-only edit message (`WM_UNDO`, `EM_UNDO`, `EM_CANUNDO`, ...).
/// Only messages from the allow-list are accepted; anything else returns `None`.
pub fn send_simple(h: Hwnd, msg: u32) -> Option<isize> {
    use windows::Win32::UI::Controls::{EM_CANUNDO, EM_EMPTYUNDOBUFFER, EM_UNDO};
    use windows::Win32::UI::WindowsAndMessaging::WM_UNDO;
    if ![WM_UNDO, EM_UNDO, EM_CANUNDO, EM_EMPTYUNDOBUFFER].contains(&msg) {
        return None;
    }
    // SAFETY: allow-listed messages take no pointer parameters (wparam = lparam = 0).
    unsafe { send(h, msg, 0, 0) }
}

/// Polls `cond` every 10 ms until it is true or `timeout` passes; returns elapsed time on success.
pub fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> Option<Duration> {
    let start = Instant::now();
    loop {
        if cond() {
            return Some(start.elapsed());
        }
        if start.elapsed() >= timeout {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

static REGISTER: Once = Once::new();

fn create_windows() -> Result<Controls, String> {
    // SAFETY: NULL module name = handle of the current executable; no ownership transferred.
    let module = unsafe { GetModuleHandleW(PCWSTR::null()) }
        .map_err(|e| format!("GetModuleHandleW: {e}"))?;
    let hinst = HINSTANCE::from(module);
    // Msftedit.dll provides RICHEDIT50W. It stays loaded for the life of the process.
    // SAFETY: loading a system DLL by name; we never free it, so the class stays valid.
    let rich_loaded = unsafe { LoadLibraryW(w!("Msftedit.dll")) }.is_ok();

    REGISTER.call_once(|| {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `wc` is fully initialised and its strings are 'static; registration is
        // process-wide and done once.
        unsafe { RegisterClassW(&wc) };
    });

    let main = create(
        WINDOW_EX_STYLE(0),
        CLASS_NAME,
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        (100, 100, 700, 520),
        None,
        0,
        hinst,
    )?;
    let edit_style = WS_CHILD | WS_VISIBLE | WS_BORDER;
    let single = create(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        edit_style | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
        (10, 10, 660, 28),
        Some(main),
        101,
        hinst,
    )?;
    let multi = create(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        edit_style
            | WS_VSCROLL
            | WINDOW_STYLE((ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32),
        (10, 48, 660, 120),
        Some(main),
        102,
        hinst,
    )?;
    let password = create(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        edit_style | WINDOW_STYLE((ES_PASSWORD | ES_AUTOHSCROLL) as u32),
        (10, 178, 660, 28),
        Some(main),
        103,
        hinst,
    )?;
    let rich = if rich_loaded {
        create(
            WS_EX_CLIENTEDGE,
            MSFTEDIT_CLASS,
            edit_style
                | WS_VSCROLL
                | WINDOW_STYLE((ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32),
            (10, 216, 660, 160),
            Some(main),
            104,
            hinst,
        )
        .ok()
    } else {
        None
    };
    // SAFETY: `main` was just created on this thread.
    let _ = unsafe { ShowWindow(main.raw(), SW_SHOW) };
    if mode() == ChildMode::Listener as u8 {
        // SAFETY: registers our own window (created on this thread) as a clipboard listener;
        // the registration ends when the window is destroyed.
        unsafe { AddClipboardFormatListener(main.raw()) }
            .map_err(|e| format!("AddClipboardFormatListener: {e}"))?;
    }
    Ok(Controls {
        main,
        single,
        multi,
        password,
        rich,
    })
}

fn create(
    ex: WINDOW_EX_STYLE,
    class: PCWSTR,
    style: WINDOW_STYLE,
    (x, y, w, h): (i32, i32, i32, i32),
    parent: Option<Hwnd>,
    id: usize,
    hinst: HINSTANCE,
) -> Result<Hwnd, String> {
    // For child windows `hmenu` carries the control id (Win32 convention).
    let menu = (id != 0).then_some(HMENU(id as *mut c_void));
    // SAFETY: class/title are valid NUL-terminated 'static strings, `parent` (if any) is a
    // live window created on this thread, no creation parameter is passed.
    let hwnd = unsafe {
        CreateWindowExW(
            ex,
            class,
            w!("KeySwitch probe fixture"),
            style,
            x,
            y,
            w,
            h,
            parent.map(Hwnd::raw),
            menu,
            Some(hinst),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW(id {id}): {e}"))?;
    Ok(Hwnd::from_raw(hwnd))
}

fn message_loop() {
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a valid MSG buffer owned by this frame.
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if r == 0 || r == -1 {
            break;
        }
        // SAFETY: `msg` was filled by GetMessageW above.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        MSG_FOCUS => {
            let target = HWND(wparam.0 as *mut c_void);
            // SAFETY: SetFocus/GetFocus act on this (the window's own) thread; `target` is a
            // child of this window. SetFocus's return value is the previous focus (may be
            // NULL), so success is judged by GetFocus afterwards.
            let ok = unsafe {
                let _ = SetFocus(Some(target));
                GetFocus() == target
            };
            LRESULT(isize::from(ok))
        }
        MSG_FOREGROUND => LRESULT(bring_to_foreground(hwnd)),
        MSG_HANG => {
            // SAFETY: deliberate hang of this fixture thread (P-16 / XR-04 target).
            unsafe { Sleep(wparam.0 as u32) };
            LRESULT(0)
        }
        MSG_OWN_DELAYED => LRESULT(own_delayed(hwnd) as isize),
        WM_KEYDOWN if mode() == ChildMode::LatePaste as u8 && wparam.0 == usize::from(b'V') => {
            // SAFETY: GetKeyState reads this thread's key state for the message being
            // processed; SetTimer targets our own window.
            unsafe {
                if GetKeyState(i32::from(VK_CONTROL.0)) < 0 {
                    SetTimer(Some(hwnd), LATE_TIMER, LATE_READ_MS, None);
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == LATE_TIMER => {
            // SAFETY: killing our own timer.
            let _ = unsafe { KillTimer(Some(hwnd), LATE_TIMER) };
            let units = read_clipboard_units(hwnd);
            event(&format!(
                "late_read units={}",
                units.map_or(-1, |n| n as i64)
            ));
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE if mode() == ChildMode::Listener as u8 => {
            let units = read_clipboard_units(hwnd);
            event(&format!(
                "listener_read units={}",
                units.map_or(-1, |n| n as i64)
            ));
            LRESULT(0)
        }
        WM_RENDERFORMAT if mode() == ChildMode::HungOwner as u8 => {
            // SAFETY: deliberate hang inside WM_RENDERFORMAT (XR-04).
            unsafe { Sleep(HUNG_RENDER_MS) };
            // Answer to this WM_RENDERFORMAT with a synthetic string (no OpenClipboard).
            let _ = crate::clipboard::render_text(wparam.0 as u32, "synthetic-hung-owner");
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => LRESULT(0),
        WM_DESTROY => {
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default processing for all other messages of our own window class.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Child fixture: opens the clipboard with `hwnd`, reads CF_UNICODETEXT (forcing delayed
/// rendering by its owner) and returns only the number of UTF-16 units (no content kept).
fn read_clipboard_units(hwnd: HWND) -> Option<usize> {
    // SAFETY: open/close are paired; the data handle is owned by the system and only
    // locked for the length computation (bounded by GlobalSize); nothing is copied out.
    unsafe {
        let mut opened = false;
        for _ in 0..50 {
            if OpenClipboard(Some(hwnd)).is_ok() {
                opened = true;
                break;
            }
            Sleep(10);
        }
        if !opened {
            return None;
        }
        let mut units = None;
        if IsClipboardFormatAvailable(13).is_ok()
            && let Ok(h) = GetClipboardData(13)
        {
            let hg = windows::Win32::Foundation::HGLOBAL(h.0);
            let size = GlobalSize(hg) / 2;
            let p = GlobalLock(hg) as *const u16;
            if !p.is_null() {
                let s = std::slice::from_raw_parts(p, size);
                units = Some(s.iter().position(|&u| u == 0).unwrap_or(s.len()));
                let _ = GlobalUnlock(hg);
            }
        }
        let _ = CloseClipboard();
        units
    }
}

/// Child fixture (XR-04): become the clipboard owner with a delayed CF_UNICODETEXT.
fn own_delayed(hwnd: HWND) -> u32 {
    // SAFETY: open/close paired on this thread; SetClipboardData(.., None) registers a
    // delayed-rendered format owned by `hwnd`.
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return 0;
        }
        let mut seq = 0;
        if EmptyClipboard().is_ok() {
            let _ = SetClipboardData(13, None);
            if IsClipboardFormatAvailable(13).is_ok() {
                seq = GetClipboardSequenceNumber();
            }
        }
        let _ = CloseClipboard();
        seq
    }
}

/// Returns 1 = direct, 2 = via AttachThreadInput, 3 = no foreground window, 0 = denied.
fn bring_to_foreground(main: HWND) -> isize {
    // SAFETY: all calls below are plain user32/kernel32 queries or requests on windows of
    // this thread; AttachThreadInput is always undone before returning. This is a
    // TEST-FIXTURE workaround for the foreground lock on CI and must not be copied into
    // product code (which never steals the foreground).
    unsafe {
        if GetForegroundWindow() == main {
            return 1;
        }
        let _ = SetForegroundWindow(main);
        if GetForegroundWindow() == main {
            return 1;
        }
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return 3;
        }
        let fg_tid = GetWindowThreadProcessId(fg, None);
        let me = GetCurrentThreadId();
        if fg_tid == 0 || fg_tid == me {
            return 0;
        }
        let attached = AttachThreadInput(me, fg_tid, true).as_bool();
        let _ = BringWindowToTop(main);
        let _ = SetForegroundWindow(main);
        if attached {
            let _ = AttachThreadInput(me, fg_tid, false);
        }
        if GetForegroundWindow() == main { 2 } else { 0 }
    }
}
