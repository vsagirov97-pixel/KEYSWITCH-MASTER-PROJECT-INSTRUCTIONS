//! Target fingerprint, operation generation gate and modifier check (WS-05, C-6′), the
//! `SelfTarget` refusal and window class/style queries (WS-03, D-30).
//!
//! Before **every** W3 batch the writer re-checks, in this order: the operation generation
//! (a newer hotkey press / cancel invalidates the running operation), held modifiers (a held
//! Ctrl would turn characters into shortcuts) and the full target fingerprint (foreground
//! HWND, focused HWND, pid and process start time — the start time defeats pid reuse).

use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GUITHREADINFO, GWL_STYLE, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowLongPtrW, GetWindowThreadProcessId, RealGetWindowClassW,
};

use crate::Hwnd;
use crate::policy;

/// Identity of the write target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    /// Foreground top-level window.
    pub foreground: Hwnd,
    /// Focused window of the foreground thread (`GetGUIThreadInfo.hwndFocus`).
    pub focus: Hwnd,
    /// Owning process id.
    pub pid: u32,
    /// Process creation time (FILETIME as u64); 0 if it could not be queried.
    pub start_time: u64,
}

/// Why a fingerprint could not be captured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// No foreground window.
    NoForeground,
    /// `GetGUIThreadInfo` failed or reported no focused window.
    NoFocus,
    /// The process start time could not be read (fail closed).
    NoStartTime,
}

/// Process creation time of `pid`, `None` if it cannot be opened/queried.
pub fn process_start_time(pid: u32) -> Option<u64> {
    // SAFETY: the handle is opened with the minimal query right, used for GetProcessTimes
    // with valid out-parameters, and closed exactly once.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let (mut c, mut e, mut k, mut u) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let r = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
        let _ = CloseHandle(h);
        r.ok()?;
        Some((u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime))
    }
}

/// Captures the current target fingerprint.
pub fn capture() -> Result<Fingerprint, CaptureError> {
    // SAFETY: trivial query.
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return Err(CaptureError::NoForeground);
    }
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out-parameter.
    let tid = unsafe { GetWindowThreadProcessId(fg, Some(&mut pid)) };
    let mut gti = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `gti` is a correctly sized, writable GUITHREADINFO.
    if tid == 0 || unsafe { GetGUIThreadInfo(tid, &mut gti) }.is_err() || gti.hwndFocus.0.is_null()
    {
        return Err(CaptureError::NoFocus);
    }
    let start_time = process_start_time(pid).ok_or(CaptureError::NoStartTime)?;
    Ok(Fingerprint {
        foreground: Hwnd::from_raw(fg),
        focus: Hwnd::from_raw(gti.hwndFocus),
        pid,
        start_time,
    })
}

/// Monotonic operation generation. Starting an operation takes a token; any newer hotkey
/// press, cancel or focus event bumps the generation and invalidates older tokens.
#[derive(Debug, Default)]
pub struct Gate {
    generation: AtomicU64,
}

/// Token of one operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token(u64);

impl Gate {
    /// New gate at generation 0.
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
        }
    }
    /// Starts an operation (bumps and returns the new generation).
    pub fn begin(&self) -> Token {
        Token(self.generation.fetch_add(1, Ordering::SeqCst) + 1)
    }
    /// Invalidates every running operation.
    pub fn bump(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    /// `true` while `t` is the current generation.
    pub fn is_current(&self, t: Token) -> bool {
        self.generation.load(Ordering::SeqCst) == t.0
    }
}

/// Why a batch must not be sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abort {
    /// The operation's generation is no longer current.
    Generation,
    /// Shift/Ctrl/Alt/Win is held (physically or injected).
    Modifiers,
    /// Foreground/focus/pid/start time differ from the captured fingerprint (or capture
    /// failed now).
    TargetChanged,
    /// The previous batch was not observed in the target in time (see `i4`: `SendInput`
    /// is asynchronous, unconsumed input follows a foreground change).
    NotDelivered,
}

impl Abort {
    /// Stable label for reports.
    pub fn label(self) -> &'static str {
        match self {
            Abort::Generation => "generation",
            Abort::Modifiers => "modifiers",
            Abort::TargetChanged => "target_changed",
            Abort::NotDelivered => "not_delivered",
        }
    }
}

/// `true` if Shift, Ctrl, Alt or a Win key is currently down (`GetAsyncKeyState`; no hook).
pub fn modifiers_down() -> bool {
    [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        // SAFETY: trivial query of the async key state.
        .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } < 0)
}

/// The full pre-batch check (WS-05).
pub fn pre_batch(expected: &Fingerprint, gate: &Gate, token: Token) -> Result<(), Abort> {
    if !gate.is_current(token) {
        return Err(Abort::Generation);
    }
    if modifiers_down() {
        return Err(Abort::Modifiers);
    }
    match capture() {
        Ok(now) if now == *expected => Ok(()),
        _ => Err(Abort::TargetChanged),
    }
}

/// Target refusals decided from the fingerprint alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetRefusal {
    /// The focused window belongs to this (KeySwitch) process.
    SelfTarget,
}

/// Refuses KeySwitch's own windows as a target.
pub fn classify(fp: &Fingerprint) -> Result<(), TargetRefusal> {
    // SAFETY: trivial query.
    if fp.pid == unsafe { GetCurrentProcessId() } {
        return Err(TargetRefusal::SelfTarget);
    }
    Ok(())
}

fn class_string(buf: &[u16], n: usize) -> String {
    String::from_utf16_lossy(&buf[..n.min(buf.len())])
}

/// `(GetClassNameW, RealGetWindowClassW)` of `h` — class names, not user text.
pub fn class_names(h: Hwnd) -> (String, String) {
    let mut a = [0u16; 256];
    let mut b = [0u16; 256];
    // SAFETY: the buffers are writable and their lengths are passed via the slices.
    let (na, nb) = unsafe {
        (
            GetClassNameW(h.raw(), &mut a).max(0) as usize,
            RealGetWindowClassW(h.raw(), &mut b) as usize,
        )
    };
    (class_string(&a, na), class_string(&b, nb))
}

/// Id of the thread that owns window `h` (0 if the handle is invalid).
pub fn window_thread(h: Hwnd) -> u32 {
    // SAFETY: trivial query; the process id out-parameter is optional.
    unsafe { GetWindowThreadProcessId(h.raw(), None) }
}

/// `GWL_STYLE` of `h` (works across processes).
pub fn window_style(h: Hwnd) -> u32 {
    // SAFETY: trivial query; an invalid handle yields 0.
    unsafe { GetWindowLongPtrW(h.raw(), GWL_STYLE) as u32 }
}

/// WS-03: `true` if the window class is an edit/RichEdit and its style has `ES_PASSWORD`.
pub fn hwnd_is_password(h: Hwnd) -> bool {
    let (class, real) = class_names(h);
    let style = window_style(h);
    policy::style_says_password(&class, style) || policy::style_says_password(&real, style)
}

/// D-30: is W1 permitted for `h`?
pub fn w1_permitted(h: Hwnd) -> Result<(), policy::W1Refusal> {
    let (class, real) = class_names(h);
    policy::w1_permitted(&class, &real, window_style(h))
}
