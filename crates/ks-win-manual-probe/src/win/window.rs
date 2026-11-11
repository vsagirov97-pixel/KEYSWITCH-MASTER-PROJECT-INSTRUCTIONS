//! The tool's own test window: a top-level window on a dedicated thread (own message loop)
//! with classic `EDIT` controls (single-line, multi-line, password) and a `RICHEDIT50W`.
//! All writes in fixture mode target these controls only. Cross-thread messages use
//! `SendMessageTimeoutW` (WS-04). Foreground is only *requested* with
//! `SetForegroundWindow`; if Windows refuses, the owner is asked to click the window.

use std::ffi::c_void;
use std::sync::Once;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Controls::RichEdit::MSFTEDIT_CLASS;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_REPLACESEL, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_PASSWORD, ES_READONLY, ES_WANTRETURN, GetMessageW, GetSystemMetrics, HMENU,
    MSG, PostMessageW, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SW_SHOW, SetForegroundWindow,
    ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_DESTROY,
    WM_GETTEXT, WM_GETTEXTLENGTH, WM_SETTEXT, WNDCLASSW, WS_BORDER, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

use super::sys::{self, Hwnd, send_timeout, wide};
use crate::policy;

/// Handles of the test controls.
#[derive(Clone, Copy, Debug)]
pub struct Controls {
    /// Top-level window.
    pub main: Hwnd,
    /// `EDIT`, single line.
    pub single: Hwnd,
    /// `EDIT`, multi-line.
    pub multi: Hwnd,
    /// `EDIT`, `ES_PASSWORD`.
    pub password: Hwnd,
    /// `RICHEDIT50W`, if Msftedit.dll loaded.
    pub rich: Option<Hwnd>,
}

const MSG_FOCUS: u32 = WM_APP + 1;
const MSG_FOREGROUND: u32 = WM_APP + 2;
const CLASS_NAME: PCWSTR = w!("KsManualProbeWindow");

/// Running test window; dropping it closes the window and joins its thread.
#[derive(Debug)]
pub struct TestWindow {
    /// Control handles.
    pub controls: Controls,
    /// GUI thread id.
    pub thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl TestWindow {
    /// Creates the window on a new thread.
    pub fn spawn() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<Result<(Controls, u32), String>>();
        let join = std::thread::Builder::new()
            .name("ks-manual-window".into())
            .spawn(move || match create_windows() {
                Ok(c) => {
                    // SAFETY: trivial call.
                    let tid = unsafe { GetCurrentThreadId() };
                    let _ = tx.send(Ok((c, tid)));
                    message_loop();
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            })
            .map_err(|e| format!("spawn window thread: {e}"))?;
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
            Err(_) => Err("window thread did not start in 10 s".into()),
        }
    }

    /// Asks Windows to make the window foreground (no input-attach workaround).
    pub fn request_foreground(&self) -> bool {
        // SAFETY: private integer-only message to our own window.
        let _ = unsafe { send_timeout(self.controls.main, MSG_FOREGROUND, 0, 0) };
        self.is_foreground()
    }

    /// `true` if our top-level window is the foreground window.
    pub fn is_foreground(&self) -> bool {
        sys::foreground() == self.controls.main
    }

    /// Sets focus to `control` on the window thread; `true` if focus is there.
    pub fn focus(&self, control: Hwnd) -> bool {
        // SAFETY: private message; wparam carries a handle value, not dereferenced.
        let r = unsafe { send_timeout(self.controls.main, MSG_FOCUS, control.0 as usize, 0) };
        r == Some(1)
    }

    /// `true` if `control` has keyboard focus in our GUI thread.
    pub fn has_focus(&self, control: Hwnd) -> bool {
        sys::focus_of_thread(self.thread_id) == control
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        // SAFETY: posting WM_CLOSE to our own window.
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

/// `WM_SETTEXT` on our control (synthetic text only).
pub fn set_text(h: Hwnd, text: &str) -> bool {
    let buf = wide(text);
    // SAFETY: `buf` is NUL-terminated and outlives the synchronous call.
    unsafe { send_timeout(h, WM_SETTEXT, 0, buf.as_ptr() as isize) }.is_some_and(|r| r != 0)
}

/// `WM_GETTEXTLENGTH`.
pub fn text_len(h: Hwnd) -> usize {
    // SAFETY: integer-only message.
    unsafe { send_timeout(h, WM_GETTEXTLENGTH, 0, 0) }
        .unwrap_or(0)
        .max(0) as usize
}

/// `WM_GETTEXT` of our own control; used only to verify synthetic content.
pub fn get_text(h: Hwnd) -> String {
    let len = text_len(h);
    let mut buf = vec![0u16; len + 1];
    // SAFETY: `buf` holds `buf.len()` units (passed in wparam) and outlives the call.
    let n = unsafe { send_timeout(h, WM_GETTEXT, buf.len(), buf.as_mut_ptr() as isize) }
        .unwrap_or(0)
        .max(0) as usize;
    String::from_utf16_lossy(&buf[..n.min(len)])
}

/// `EM_SETSEL`.
pub fn set_sel(h: Hwnd, start: i32, end: i32) {
    // SAFETY: integer-only message.
    let _ = unsafe { send_timeout(h, EM_SETSEL, start as isize as usize, end as isize) };
}

/// `EM_GETSEL`.
pub fn get_sel(h: Hwnd) -> (u32, u32) {
    let mut s = 0u32;
    let mut e = 0u32;
    // SAFETY: pointers to locals; the message targets our own window in this process.
    let _ = unsafe {
        send_timeout(
            h,
            EM_GETSEL,
            &mut s as *mut u32 as usize,
            &mut e as *mut u32 as isize,
        )
    };
    (s, e)
}

/// Why W1 was refused for a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum W1Refusal {
    /// Class is not exactly `Edit` (D-30 / XR-03: RichEdit interprets RTF).
    NotEditClass,
    /// `ES_PASSWORD`.
    Password,
    /// `ES_READONLY`.
    ReadOnly,
}

/// D-30 guard: both class names exactly `Edit`, not password, not read-only.
pub fn w1_guard(h: Hwnd) -> Result<(), W1Refusal> {
    if !policy::is_edit_class(&sys::class_name(h))
        || !policy::is_edit_class(&sys::real_class_name(h))
    {
        return Err(W1Refusal::NotEditClass);
    }
    let st = sys::style(h);
    if st & (ES_PASSWORD as u32) != 0 {
        return Err(W1Refusal::Password);
    }
    if st & (ES_READONLY as u32) != 0 {
        return Err(W1Refusal::ReadOnly);
    }
    Ok(())
}

/// W1: `EM_REPLACESEL(fCanUndo = TRUE)` after the guard. `Ok(false)` = message timed out.
pub fn w1_replace(h: Hwnd, text: &str) -> Result<bool, W1Refusal> {
    w1_guard(h)?;
    let buf = wide(text);
    // SAFETY: `buf` is NUL-terminated and outlives the synchronous call.
    Ok(unsafe { send_timeout(h, EM_REPLACESEL, 1, buf.as_ptr() as isize) }.is_some())
}

/// Integer-only edit message from an allow-list (`WM_UNDO`, `EM_UNDO`, `EM_CANUNDO`).
pub fn send_simple(h: Hwnd, msg: u32) -> Option<isize> {
    use windows::Win32::UI::Controls::{EM_CANUNDO, EM_UNDO};
    use windows::Win32::UI::WindowsAndMessaging::WM_UNDO;
    if ![WM_UNDO, EM_UNDO, EM_CANUNDO].contains(&msg) {
        return None;
    }
    // SAFETY: allow-listed messages take no pointer parameters.
    unsafe { send_timeout(h, msg, 0, 0) }
}

/// Polls `cond` every 10 ms until true or `timeout`.
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
    // SAFETY: NULL name = this executable's module handle.
    let module = unsafe { GetModuleHandleW(PCWSTR::null()) }
        .map_err(|e| format!("GetModuleHandleW: {e}"))?;
    let hinst = HINSTANCE::from(module);
    // SAFETY: loads a system DLL by name; never freed, so the class stays valid.
    let rich_loaded = unsafe { LoadLibraryW(w!("Msftedit.dll")) }.is_ok();
    REGISTER.call_once(|| {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `wc` is initialised and its strings are 'static.
        unsafe { RegisterClassW(&wc) };
    });
    // Place the window at the right edge so the console stays visible on the left.
    // SAFETY: trivial query.
    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let x = (screen_w - 720).max(0);
    let main = create(
        WINDOW_EX_STYLE(0),
        CLASS_NAME,
        w!("KeySwitch probe - test window (do not type here)"),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        (x, 60, 700, 440),
        None,
        0,
        hinst,
    )?;
    let es = WS_CHILD | WS_VISIBLE | WS_BORDER;
    let single = create(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        es | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
        (10, 10, 660, 28),
        Some(main),
        101,
        hinst,
    )?;
    let multi = create(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        es | WS_VSCROLL | WINDOW_STYLE((ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32),
        (10, 48, 660, 100),
        Some(main),
        102,
        hinst,
    )?;
    let password = create(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        es | WINDOW_STYLE((ES_PASSWORD | ES_AUTOHSCROLL) as u32),
        (10, 158, 660, 28),
        Some(main),
        103,
        hinst,
    )?;
    let rich = if rich_loaded {
        create(
            WS_EX_CLIENTEDGE,
            MSFTEDIT_CLASS,
            w!(""),
            es | WS_VSCROLL | WINDOW_STYLE((ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32),
            (10, 196, 660, 140),
            Some(main),
            104,
            hinst,
        )
        .ok()
    } else {
        None
    };
    // SAFETY: `main` was created on this thread.
    let _ = unsafe { ShowWindow(main.raw(), SW_SHOW) };
    Ok(Controls {
        main,
        single,
        multi,
        password,
        rich,
    })
}

#[allow(clippy::too_many_arguments)]
fn create(
    ex: WINDOW_EX_STYLE,
    class: PCWSTR,
    title: PCWSTR,
    style: WINDOW_STYLE,
    (x, y, w, h): (i32, i32, i32, i32),
    parent: Option<Hwnd>,
    id: usize,
    hinst: HINSTANCE,
) -> Result<Hwnd, String> {
    let menu = (id != 0).then_some(HMENU(id as *mut c_void));
    // SAFETY: class/title are valid 'static NUL-terminated strings; `parent` is a live window
    // of this thread; no creation parameter.
    let hwnd = unsafe {
        CreateWindowExW(
            ex,
            class,
            title,
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
        // SAFETY: `msg` is a local buffer.
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if r == 0 || r == -1 {
            break;
        }
        // SAFETY: `msg` was filled by GetMessageW.
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
            // SAFETY: SetFocus/GetFocus on this window's own thread; `target` is our child.
            let ok = unsafe {
                let _ = SetFocus(Some(target));
                GetFocus() == target
            };
            LRESULT(isize::from(ok))
        }
        MSG_FOREGROUND => {
            // SAFETY: a plain request for our own window; Windows may refuse it.
            let _ = unsafe { SetForegroundWindow(hwnd) };
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: ends this thread's loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default processing for our own window class.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
