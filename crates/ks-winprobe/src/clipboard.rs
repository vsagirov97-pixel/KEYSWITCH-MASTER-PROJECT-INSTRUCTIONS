//! Clipboard contract (SYSTEM_ARCHITECTURE §4.1): snapshot by whitelist without touching
//! foreign formats, privacy formats (D-13), sequence number read **under** `OpenClipboard`
//! (WS-01), restore-only-if-unchanged with the original privacy markers (WS-02), delayed
//! rendering with requester attribution (XR-01) and a deadline-bounded snapshot (XR-04).

use std::sync::mpsc;
use std::sync::{Mutex, Once};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    GlobalFree, HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT, SetLastError, WIN32_ERROR,
    WPARAM,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardOwner,
    GetClipboardSequenceNumber, GetOpenClipboardWindow, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::{CF_LOCALE, CF_OEMTEXT, CF_TEXT, CF_UNICODETEXT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowThreadProcessId,
    IsHungAppWindow, MSG, PostQuitMessage, RegisterClassW, TranslateMessage, WINDOW_EX_STYLE,
    WM_CLOSE, WM_DESTROY, WM_RENDERALLFORMATS, WM_RENDERFORMAT, WNDCLASSW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::fixture::Hwnd;

/// Upper bound for a snapshot (SYSTEM_ARCHITECTURE §4.1: 16 MB).
pub const SNAPSHOT_LIMIT_BYTES: usize = 16 * 1024 * 1024;

/// Open clipboard; closed on drop.
#[derive(Debug)]
pub struct Open(());

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: this guard exists only while we hold the clipboard open.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Opens the clipboard with `owner`, retrying for up to ~1 s (another process may hold it).
pub fn open(owner: Hwnd) -> Result<Open, String> {
    open_for(Some(owner), Duration::from_secs(1))
}

/// Opens the clipboard (with `owner` or NULL), retrying every 10 ms for up to `wait`.
pub fn open_for(owner: Option<Hwnd>, wait: Duration) -> Result<Open, String> {
    let start = Instant::now();
    loop {
        // SAFETY: `owner` (if any) is a live window; success is paired with CloseClipboard.
        if unsafe { OpenClipboard(owner.map(Hwnd::raw)) }.is_ok() {
            return Ok(Open(()));
        }
        if start.elapsed() >= wait {
            return Err("OpenClipboard failed".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Current clipboard sequence number (0 = no WINSTA_ACCESSCLIPBOARD).
pub fn seq() -> u32 {
    // SAFETY: trivial query.
    unsafe { GetClipboardSequenceNumber() }
}

/// Format ids currently on the clipboard — enumerated, no data is read.
pub fn format_ids(_open: &Open) -> Vec<u32> {
    let mut out = Vec::new();
    let mut f = 0u32;
    loop {
        // SAFETY: clipboard is open (`_open`); EnumClipboardFormats only lists ids.
        f = unsafe { EnumClipboardFormats(f) };
        if f == 0 {
            break;
        }
        out.push(f);
    }
    out
}

/// The three D-13 privacy formats, in order: Exclude…MonitorProcessing,
/// CanIncludeInClipboardHistory, CanUploadToCloudClipboard. `0` = registration failed.
pub fn privacy_formats() -> [u32; 3] {
    // SAFETY: registering well-known format names (static NUL-terminated strings).
    unsafe {
        [
            RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
            RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
            RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
        ]
    }
}

/// Registered ids of "HTML Format" and "Rich Text Format".
pub fn html_rtf_formats() -> [u32; 2] {
    // SAFETY: registering well-known format names (static NUL-terminated strings).
    unsafe {
        [
            RegisterClipboardFormatW(w!("HTML Format")),
            RegisterClipboardFormatW(w!("Rich Text Format")),
        ]
    }
}

/// Snapshot whitelist (§4.1 + WS-02): text formats, locale, HTML, RTF and the privacy
/// markers (kept so that a restore re-applies them).
pub fn whitelist() -> Vec<u32> {
    let mut v = vec![
        u32::from(CF_UNICODETEXT.0),
        u32::from(CF_TEXT.0),
        u32::from(CF_OEMTEXT.0),
        u32::from(CF_LOCALE.0),
    ];
    v.extend(html_rtf_formats());
    v.extend(privacy_formats());
    v.retain(|&f| f != 0);
    v
}

/// `true` if `format` is available (no data read).
pub fn available(format: u32) -> bool {
    // SAFETY: trivial query.
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

fn set_bytes(format: u32, bytes: &[u8]) -> Result<(), String> {
    // SAFETY: we allocate a movable global block of the exact size, copy `bytes` into it
    // while locked, unlock, and hand it to SetClipboardData. On success the system owns the
    // block; on failure we free it ourselves.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1))
            .map_err(|e| format!("GlobalAlloc: {e}"))?;
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            let _ = GlobalFree(Some(h));
            return Err("GlobalLock failed".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
        if let Err(e) = SetClipboardData(format, Some(HANDLE(h.0))) {
            let _ = GlobalFree(Some(h));
            return Err(format!("SetClipboardData({format}): {e}"));
        }
    }
    Ok(())
}

/// Registers `format` as delayed-rendered (`SetClipboardData(format, NULL)`).
fn set_delayed(format: u32) -> Result<(), String> {
    // SAFETY: clipboard is open by the caller; a NULL handle requests delayed rendering.
    // The documented return value for that case is NULL, so success is judged by the last
    // error (cleared first) and by the format being listed afterwards.
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let r = SetClipboardData(format, None);
        if let Err(e) = r
            && e.code().0 != 0
            && !available(format)
        {
            return Err(format!("SetClipboardData({format}, NULL): {e}"));
        }
    }
    if available(format) {
        Ok(())
    } else {
        Err("delayed format not listed after SetClipboardData".into())
    }
}

fn text_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Renders `text` for a pending `WM_RENDERFORMAT(format)` (no OpenClipboard, by contract).
pub fn render_text(format: u32, text: &str) -> Result<(), String> {
    set_bytes(format, &text_bytes(text))
}

fn put_privacy(private: bool) -> Result<(), String> {
    if private {
        for f in privacy_formats() {
            if f == 0 {
                return Err("RegisterClipboardFormatW failed".into());
            }
            set_bytes(f, &0u32.to_le_bytes())?;
        }
    }
    Ok(())
}

/// Replaces the clipboard with `text` (CF_UNICODETEXT) and, if `private`, the three
/// privacy formats (DWORD 0 each). Returns the sequence number read **while the clipboard
/// is still open** (WS-01): nobody else can write between our last write and this read.
pub fn put_text(open: &Open, text: &str, private: bool) -> Result<u32, String> {
    let _ = open;
    // SAFETY: clipboard is open by us (`open`); EmptyClipboard makes our window the owner.
    unsafe { EmptyClipboard() }.map_err(|e| format!("EmptyClipboard: {e}"))?;
    set_bytes(u32::from(CF_UNICODETEXT.0), &text_bytes(text))?;
    put_privacy(private)?;
    Ok(seq())
}

/// Puts arbitrary `(format, bytes)` items (synthetic writers in tests and restore).
/// Returns the sequence number read under `OpenClipboard` (WS-01).
pub fn put_items(open: &Open, items: &[(u32, Vec<u8>)]) -> Result<u32, String> {
    let _ = open;
    // SAFETY: clipboard is open by us (`open`).
    unsafe { EmptyClipboard() }.map_err(|e| format!("EmptyClipboard: {e}"))?;
    for (f, b) in items {
        set_bytes(*f, b)?;
    }
    Ok(seq())
}

/// Reads CF_UNICODETEXT. Test-only, used on synthetic content to verify restore.
pub fn read_text(_open: &Open) -> Option<String> {
    let bytes = read_bytes(u32::from(CF_UNICODETEXT.0))?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    Some(String::from_utf16_lossy(&units[..end]))
}

/// Copies the HGLOBAL data of `format` (clipboard must be open by the caller).
fn read_bytes(format: u32) -> Option<Vec<u8>> {
    // SAFETY: the clipboard is open (caller); the handle is owned by the system, we only lock
    // it for reading, bound the copy by GlobalSize, then unlock.
    unsafe {
        let h = GetClipboardData(format).ok()?;
        let hg = HGLOBAL(h.0);
        let size = GlobalSize(hg);
        let p = GlobalLock(hg) as *const u8;
        if p.is_null() {
            return None;
        }
        let v = std::slice::from_raw_parts(p, size).to_vec();
        let _ = GlobalUnlock(hg);
        Some(v)
    }
}

/// Clipboard snapshot by whitelist. Holds user data in the product, therefore no `Debug`
/// and no accessor for the bytes other than equality checks.
pub struct Snapshot {
    items: Vec<(u32, Vec<u8>)>,
}

impl Snapshot {
    /// Format ids in the snapshot (metadata).
    pub fn format_ids(&self) -> Vec<u32> {
        self.items.iter().map(|(f, _)| *f).collect()
    }
    /// Total size in bytes (metadata).
    pub fn total_bytes(&self) -> usize {
        self.items.iter().map(|(_, b)| b.len()).sum()
    }
    /// Bytes of `format`, compared without exposing them.
    pub fn bytes_equal(&self, format: u32, expected: &[u8]) -> bool {
        self.items
            .iter()
            .any(|(f, b)| *f == format && b.as_slice() == expected)
    }
    /// `true` if every item of `self` is present with identical bytes in `other`.
    pub fn subset_of(&self, other: &Snapshot) -> bool {
        self.items.iter().all(|(f, b)| other.bytes_equal(*f, b))
    }
}

/// Why a snapshot (and therefore W2) is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapRefusal {
    /// A format outside the whitelist is present (OLE, files, images, private...).
    ForeignFormat(u32),
    /// Whitelisted data exceeds [`SNAPSHOT_LIMIT_BYTES`].
    TooLarge,
    /// A listed whitelisted format could not be read.
    ReadFailed(u32),
    /// The clipboard could not be opened in time.
    OpenFailed,
    /// The clipboard owner did not answer a `WM_NULL` ping / is hung (XR-04).
    OwnerUnresponsive,
    /// The read did not finish before the deadline (XR-04: owner hangs in WM_RENDERFORMAT).
    Deadline,
}

/// Snapshot by whitelist (clipboard open by the caller). Enumerates first; refuses on the
/// first foreign format **before** reading anything. CF_TEXT/CF_OEMTEXT are skipped when
/// CF_UNICODETEXT is present (the system re-synthesises them).
pub fn snapshot(open: &Open) -> Result<Snapshot, SnapRefusal> {
    let ids = format_ids(open);
    let wl = whitelist();
    if let Some(&f) = ids.iter().find(|f| !wl.contains(f)) {
        return Err(SnapRefusal::ForeignFormat(f));
    }
    let has_unicode = ids.contains(&u32::from(CF_UNICODETEXT.0));
    let mut items = Vec::new();
    let mut total = 0usize;
    for f in ids {
        if has_unicode && (f == u32::from(CF_TEXT.0) || f == u32::from(CF_OEMTEXT.0)) {
            continue;
        }
        let b = read_bytes(f).ok_or(SnapRefusal::ReadFailed(f))?;
        total += b.len();
        if total > SNAPSHOT_LIMIT_BYTES {
            return Err(SnapRefusal::TooLarge);
        }
        items.push((f, b));
    }
    Ok(Snapshot { items })
}

/// XR-04: snapshot that never blocks the caller beyond `deadline`.
/// 1. If the clipboard owner is another window: `IsHungAppWindow` and a `WM_NULL` ping
///    (`SMTO_ABORTIFHUNG`, at most 100 ms) — no answer → `OwnerUnresponsive`.
/// 2. The read runs on a worker thread (clipboard opened with NULL); if it does not finish
///    by the deadline → `Deadline`. The worker handle is returned so the caller can observe
///    how long the worker stayed blocked (it holds the clipboard open meanwhile).
pub fn snapshot_guarded(
    deadline: Duration,
) -> (Result<Snapshot, SnapRefusal>, Option<JoinHandle<Duration>>) {
    let start = Instant::now();
    // SAFETY: trivial queries; the handle is only used as a message target.
    let owner = unsafe { GetClipboardOwner() }
        .ok()
        .filter(|h| !h.0.is_null())
        .map(Hwnd::from_raw);
    if let Some(o) = owner {
        // SAFETY: trivial query on a window handle (a stale handle yields FALSE).
        let hung = unsafe { IsHungAppWindow(o.raw()) }.as_bool();
        let ping_ms = (deadline.as_millis() as u32).min(100);
        if hung || !crate::fixture::ping(o, ping_ms) {
            return (Err(SnapRefusal::OwnerUnresponsive), None);
        }
    }
    let (tx, rx) = mpsc::channel::<Result<Snapshot, SnapRefusal>>();
    let remaining = deadline.saturating_sub(start.elapsed());
    let worker = std::thread::spawn(move || {
        let t = Instant::now();
        let res = match open_for(None, remaining) {
            Ok(o) => snapshot(&o),
            Err(_) => Err(SnapRefusal::OpenFailed),
        };
        let _ = tx.send(res);
        t.elapsed()
    });
    match rx.recv_timeout(deadline.saturating_sub(start.elapsed())) {
        Ok(r) => (r, Some(worker)),
        Err(_) => (Err(SnapRefusal::Deadline), Some(worker)),
    }
}

/// Outcome of a guarded restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restore {
    /// Sequence number matched; the snapshot was put back.
    Restored,
    /// Someone wrote after us; restore skipped (their data is kept).
    SkippedSeqChanged,
}

/// Restores CF_UNICODETEXT `snapshot` only if the sequence number (checked **under**
/// `OpenClipboard`) still equals `our_seq` (S-2a API, text only).
pub fn restore_if_unchanged(owner: Hwnd, our_seq: u32, snapshot: &str) -> Result<Restore, String> {
    let o = open(owner)?;
    if seq() != our_seq {
        return Ok(Restore::SkippedSeqChanged);
    }
    put_text(&o, snapshot, false)?;
    Ok(Restore::Restored)
}

/// WS-02: restores every whitelisted item of `snap` byte-for-byte — including the original
/// privacy markers — only if the sequence number under `OpenClipboard` equals `expected`.
pub fn restore_snapshot(owner: Hwnd, expected: u32, snap: &Snapshot) -> Result<Restore, String> {
    let o = open(owner)?;
    if seq() != expected {
        return Ok(Restore::SkippedSeqChanged);
    }
    put_items(&o, &snap.items)?;
    Ok(Restore::Restored)
}

// ---------------------------------------------------------------------------------------
// Delayed rendering owner (W2 consumption signal, XR-01 / XR-02).

/// One `WM_RENDERFORMAT` received by [`DelayedOwner`].
#[derive(Clone, Copy, Debug)]
pub struct Render {
    /// Milliseconds since the delayed put.
    pub at_ms: u64,
    /// Requested format.
    pub format: u32,
    /// `GetOpenClipboardWindow()` at that moment — the window the requester opened the
    /// clipboard with (NULL if it used `OpenClipboard(NULL)`).
    pub requester_hwnd: Hwnd,
    /// Process of `requester_hwnd` (0 = unknown → never attributed to the target).
    pub requester_pid: u32,
    /// Sequence number right after rendering (the requester holds the clipboard open).
    pub seq_after: u32,
    /// Rendering succeeded.
    pub rendered: bool,
}

struct OwnerState {
    payload: String,
    t0: Instant,
    renders: Vec<Render>,
    render_all: u32,
}

static OWNER_STATE: Mutex<Option<OwnerState>> = Mutex::new(None);
static OWNER_CLASS: Once = Once::new();

fn owner_state() -> std::sync::MutexGuard<'static, Option<OwnerState>> {
    OWNER_STATE.lock().unwrap_or_else(|p| p.into_inner())
}

/// An in-process clipboard owner window (own thread + message loop) that offers
/// CF_UNICODETEXT by delayed rendering and records who asked for it. One at a time.
#[derive(Debug)]
pub struct DelayedOwner {
    /// Owner window (hidden popup).
    pub hwnd: Hwnd,
    join: Option<JoinHandle<()>>,
}

impl DelayedOwner {
    /// Creates the owner window on a new thread.
    pub fn spawn() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<Result<Hwnd, String>>();
        let join = std::thread::Builder::new()
            .name("ks-winprobe-clip-owner".into())
            .spawn(move || {
                let r = create_owner_window();
                let ok = r.is_ok();
                let _ = tx.send(r);
                if ok {
                    owner_loop();
                }
            })
            .map_err(|e| format!("spawn owner thread: {e}"))?;
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(hwnd)) => Ok(Self {
                hwnd,
                join: Some(join),
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("owner thread not ready in 10 s".into()),
        }
    }

    /// Becomes the clipboard owner: CF_UNICODETEXT delayed (rendered from `text` on demand)
    /// plus, if `private`, the D-13 markers as real data. Returns the sequence number read
    /// under `OpenClipboard` (WS-01).
    pub fn put_delayed(&self, text: &str, private: bool) -> Result<u32, String> {
        *owner_state() = Some(OwnerState {
            payload: text.to_owned(),
            t0: Instant::now(),
            renders: Vec::new(),
            render_all: 0,
        });
        let o = open(self.hwnd)?;
        // SAFETY: clipboard is open by us with our owner window.
        unsafe { EmptyClipboard() }.map_err(|e| format!("EmptyClipboard: {e}"))?;
        set_delayed(u32::from(CF_UNICODETEXT.0))?;
        put_privacy(private)?;
        let s = seq();
        drop(o);
        Ok(s)
    }

    /// Renders received so far.
    pub fn renders(&self) -> Vec<Render> {
        owner_state()
            .as_ref()
            .map(|s| s.renders.clone())
            .unwrap_or_default()
    }

    /// Number of `WM_RENDERALLFORMATS` received.
    pub fn render_all_count(&self) -> u32 {
        owner_state().as_ref().map_or(0, |s| s.render_all)
    }

    /// Milliseconds since the last delayed put.
    pub fn since_put_ms(&self) -> u64 {
        owner_state()
            .as_ref()
            .map_or(0, |s| s.t0.elapsed().as_millis() as u64)
    }
}

impl Drop for DelayedOwner {
    fn drop(&mut self) {
        // SAFETY: posting WM_CLOSE to our own window; a stale handle only makes it fail.
        let _ = unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(self.hwnd.raw()),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            )
        };
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        *owner_state() = None;
    }
}

const OWNER_CLASS_NAME: PCWSTR = w!("KsWinProbeClipOwner");

fn create_owner_window() -> Result<Hwnd, String> {
    // SAFETY: NULL module name = handle of the current executable.
    let module = unsafe { GetModuleHandleW(PCWSTR::null()) }
        .map_err(|e| format!("GetModuleHandleW: {e}"))?;
    let hinst = HINSTANCE::from(module);
    OWNER_CLASS.call_once(|| {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(owner_wndproc),
            hInstance: hinst,
            lpszClassName: OWNER_CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `wc` is fully initialised with 'static strings; registered once.
        unsafe { RegisterClassW(&wc) };
    });
    // SAFETY: valid class/title strings; hidden popup without parent or creation data.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            OWNER_CLASS_NAME,
            w!("KeySwitch probe clipboard owner"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW(owner): {e}"))?;
    Ok(Hwnd::from_raw(hwnd))
}

fn owner_loop() {
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

extern "system" fn owner_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_RENDERFORMAT => {
            let format = wparam.0 as u32;
            // SAFETY: plain queries; the requester window is only used to look up its pid.
            let requester = unsafe { GetOpenClipboardWindow() }
                .map(Hwnd::from_raw)
                .unwrap_or(Hwnd::from_isize(0));
            let mut pid = 0u32;
            if !requester.is_null() {
                // SAFETY: `pid` is a valid out-parameter.
                unsafe { GetWindowThreadProcessId(requester.raw(), Some(&mut pid)) };
            }
            let mut guard = owner_state();
            if let Some(st) = guard.as_mut() {
                let rendered = render_text(format, &st.payload).is_ok();
                st.renders.push(Render {
                    at_ms: st.t0.elapsed().as_millis() as u64,
                    format,
                    requester_hwnd: requester,
                    requester_pid: pid,
                    seq_after: seq(),
                    rendered,
                });
            }
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => {
            // We are being destroyed while owning the clipboard: deliberately do not render
            // (temporary data must not outlive the operation).
            if let Some(st) = owner_state().as_mut() {
                st.render_all += 1;
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default processing for all other messages of our own window class.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
