//! Win32 test fixture: a top-level window owned by a dedicated thread (with its own
//! message loop) that hosts classic `EDIT` controls and a `RICHEDIT50W` control.
//!
//! All controls live in the test process. Other threads talk to them with `SendMessageW`
//! (synchronous, processed by the fixture thread) — exactly how a real target would be
//! addressed, minus the process boundary.

use std::ffi::c_void;
use std::sync::Once;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Controls::RichEdit::MSFTEDIT_CLASS;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_REPLACESEL, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DispatchMessageW, ES_AUTOHSCROLL,
    ES_AUTOVSCROLL, ES_MULTILINE, ES_PASSWORD, ES_WANTRETURN, GetForegroundWindow, GetMessageW,
    GetWindowThreadProcessId, HMENU, MSG, PostMessageW, PostQuitMessage, RegisterClassW, SW_SHOW,
    SendMessageW, SetForegroundWindow, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_CLOSE, WM_DESTROY, WM_GETTEXT, WM_GETTEXTLENGTH, WM_SETTEXT, WNDCLASSW, WS_BORDER,
    WS_CHILD, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_VSCROLL,
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
const CLASS_NAME: PCWSTR = w!("KsWinProbeFixture");

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
        // SAFETY: private message with integer-only parameters.
        let r = unsafe { send(self.controls.main, MSG_FOREGROUND, 0, 0) };
        match r {
            1 => Foreground::Direct,
            2 => Foreground::ViaAttachThreadInput,
            3 => Foreground::NoForegroundWindow,
            _ => Foreground::Denied,
        }
    }

    /// `true` if the fixture's top-level window is currently the foreground window.
    pub fn is_foreground(&self) -> bool {
        // SAFETY: trivial query without arguments.
        Hwnd::from_raw(unsafe { GetForegroundWindow() }) == self.controls.main
    }

    /// Sets keyboard focus to `control` (on the fixture thread); `true` if focus is there now.
    pub fn focus(&self, control: Hwnd) -> bool {
        // SAFETY: private message; wparam is a window handle value, not dereferenced.
        unsafe { send(self.controls.main, MSG_FOCUS, control.0 as usize, 0) == 1 }
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

/// Synchronous `SendMessageW` to a fixture window.
///
/// # Safety
/// If `msg` interprets `wparam`/`lparam` as pointers, they must point to buffers that are
/// valid for that message and outlive the call (SendMessage is synchronous).
unsafe fn send(h: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
    // SAFETY: forwarded from the caller's contract; the target window belongs to this
    // process and its thread pumps messages.
    unsafe { SendMessageW(h.raw(), msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))) }.0
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `WM_SETTEXT` (resets the undo buffer of Edit controls).
pub fn set_text(h: Hwnd, text: &str) -> bool {
    let buf = wide(text);
    // SAFETY: `buf` is NUL-terminated and alive until the synchronous call returns.
    unsafe { send(h, WM_SETTEXT, 0, buf.as_ptr() as isize) != 0 }
}

/// `WM_GETTEXT` of the whole control. Test-only: used on synthetic content.
pub fn get_text(h: Hwnd) -> String {
    let len = text_len(h);
    let mut buf = vec![0u16; len + 1];
    // SAFETY: `buf` has room for `buf.len()` units (the count passed in wparam) and
    // outlives the synchronous call.
    let n = unsafe { send(h, WM_GETTEXT, buf.len(), buf.as_mut_ptr() as isize) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n.min(len)])
}

/// Length in UTF-16 units (`WM_GETTEXTLENGTH`), without reading the text.
pub fn text_len(h: Hwnd) -> usize {
    // SAFETY: integer-only message.
    unsafe { send(h, WM_GETTEXTLENGTH, 0, 0) }.max(0) as usize
}

/// `EM_SETSEL(start, end)`; `end = -1` selects to the end.
pub fn set_sel(h: Hwnd, start: i32, end: i32) {
    // SAFETY: integer-only message (sign-extended positions, as the API expects).
    unsafe { send(h, EM_SETSEL, start as isize as usize, end as isize) };
}

/// `EM_GETSEL` → (start, end) in UTF-16 units.
pub fn get_sel(h: Hwnd) -> (u32, u32) {
    let mut start = 0u32;
    let mut end = 0u32;
    // SAFETY: pointers to two local u32s that outlive the synchronous call.
    unsafe {
        send(
            h,
            EM_GETSEL,
            &mut start as *mut u32 as usize,
            &mut end as *mut u32 as isize,
        )
    };
    (start, end)
}

/// `EM_REPLACESEL(fCanUndo, text)` — write method W1.
pub fn replace_sel(h: Hwnd, text: &str, can_undo: bool) {
    let buf = wide(text);
    // SAFETY: `buf` is NUL-terminated and alive until the synchronous call returns.
    unsafe {
        send(
            h,
            EM_REPLACESEL,
            usize::from(can_undo),
            buf.as_ptr() as isize,
        )
    };
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
    Some(unsafe { send(h, msg, 0, 0) })
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
        WM_DESTROY => {
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default processing for all other messages of our own window class.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
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
