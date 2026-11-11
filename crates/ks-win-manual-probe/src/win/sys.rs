//! Small Win32 helpers: window identity, timed messages, process facts, OS version,
//! Desktop folder. Everything here returns metadata only.

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_ELEVATION,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenElevation, TokenIntegrityLevel,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, OpenInputDesktop,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::Win32::UI::WindowsAndMessaging::{
    GUITHREADINFO, GWL_STYLE, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowLongPtrW, GetWindowThreadProcessId, RealGetWindowClassW, SEND_MESSAGE_TIMEOUT_FLAGS,
    SMTO_ABORTIFHUNG, SMTO_NORMAL, SendMessageTimeoutW,
};
use windows::core::{PCWSTR, PWSTR, w};

use crate::policy;

/// Window handle that can cross threads (an HWND is a system-wide identifier).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hwnd(pub isize);

impl Hwnd {
    /// Wraps a raw handle.
    pub fn from_raw(h: HWND) -> Self {
        Self(h.0 as isize)
    }
    /// The raw handle for Win32 calls.
    pub fn raw(self) -> HWND {
        HWND(self.0 as *mut c_void)
    }
    /// NULL handle.
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// NUL-terminated UTF-16.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Timeout for every cross-thread message (WS-04: never an unbounded SendMessage).
pub const SEND_TIMEOUT_MS: u32 = 2000;

/// `SendMessageTimeoutW(SMTO_ABORTIFHUNG)`; `None` on timeout/hung/failure.
///
/// # Safety
/// If `msg` interprets `wparam`/`lparam` as pointers, they must be valid for that message
/// and outlive the call (the call is synchronous unless it times out; callers only use it
/// with windows of this process, whose threads keep pumping).
pub unsafe fn send_timeout(h: Hwnd, msg: u32, wparam: usize, lparam: isize) -> Option<isize> {
    let mut result: usize = 0;
    let flags = SEND_MESSAGE_TIMEOUT_FLAGS(SMTO_ABORTIFHUNG.0 | SMTO_NORMAL.0);
    // SAFETY: forwarded from the caller's contract; `result` is a local out-parameter.
    let r = unsafe {
        SendMessageTimeoutW(
            h.raw(),
            msg,
            WPARAM(wparam),
            LPARAM(lparam),
            flags,
            SEND_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    (r.0 != 0).then_some(result as isize)
}

fn utf16_name(buf: &[u16], n: usize) -> String {
    String::from_utf16_lossy(&buf[..n.min(buf.len())])
}

/// `GetClassNameW` (class names are developer identifiers, not user text).
pub fn class_name(h: Hwnd) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: `buf` is a writable local buffer; its length is passed by the wrapper.
    let n = unsafe { GetClassNameW(h.raw(), &mut buf) }.max(0) as usize;
    utf16_name(&buf, n)
}

/// `RealGetWindowClassW` (base class for superclassed controls).
pub fn real_class_name(h: Hwnd) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: as above.
    let n = unsafe { RealGetWindowClassW(h.raw(), &mut buf) } as usize;
    utf16_name(&buf, n)
}

/// `GWL_STYLE` bits.
pub fn style(h: Hwnd) -> u32 {
    // SAFETY: plain query; an invalid handle yields 0.
    unsafe { GetWindowLongPtrW(h.raw(), GWL_STYLE) as u32 }
}

/// Foreground window.
pub fn foreground() -> Hwnd {
    // SAFETY: trivial query.
    Hwnd::from_raw(unsafe { GetForegroundWindow() })
}

/// (thread id, process id) of a window.
pub fn window_thread_process(h: Hwnd) -> (u32, u32) {
    let mut pid = 0u32;
    // SAFETY: `pid` is a local out-parameter.
    let tid = unsafe { GetWindowThreadProcessId(h.raw(), Some(&mut pid)) };
    (tid, pid)
}

/// Window with keyboard focus in GUI thread `tid` (NULL if none / failure).
pub fn focus_of_thread(tid: u32) -> Hwnd {
    let mut gti = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `gti` is a local struct with cbSize set as required.
    if unsafe { GetGUIThreadInfo(tid, &mut gti) }.is_ok() {
        Hwnd::from_raw(gti.hwndFocus)
    } else {
        Hwnd(0)
    }
}

/// `true` if Shift, Ctrl, Alt or Win is physically/logically down right now (WS-05).
pub fn modifiers_down() -> bool {
    [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        // SAFETY: trivial query.
        .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000 != 0)
}

/// Facts about a process token.
#[derive(Clone, Debug, Default)]
pub struct ProcFacts {
    /// Executable file name only (no directory).
    pub exe: Option<String>,
    /// Integrity label.
    pub integrity: Option<&'static str>,
    /// Token is elevated.
    pub elevated: Option<bool>,
    /// First Win32 error met (e.g. 5 = access denied, typical for elevated targets).
    pub error: Option<u32>,
}

fn last_error() -> u32 {
    // SAFETY: reads the thread-local last error.
    unsafe { GetLastError() }.0
}

fn token_facts(process: HANDLE, f: &mut ProcFacts) {
    let mut token = HANDLE::default();
    // SAFETY: `process` is a live handle with query rights; `token` is a local out-param
    // closed below.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.is_err() {
        f.error.get_or_insert(last_error());
        return;
    }
    let mut elev = TOKEN_ELEVATION::default();
    let mut ret = 0u32;
    // SAFETY: buffer is a local TOKEN_ELEVATION of the size passed.
    if unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elev as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        )
    }
    .is_ok()
    {
        f.elevated = Some(elev.TokenIsElevated != 0);
    }
    // TOKEN_MANDATORY_LABEL is followed by the SID in the same buffer; 128 bytes is ample
    // (u64 elements keep the pointer-bearing struct aligned).
    let mut buf = [0u64; 16];
    // SAFETY: `buf` is writable for the size passed; on success it starts with a
    // TOKEN_MANDATORY_LABEL whose Sid points into `buf`, read while `buf` is alive.
    unsafe {
        if GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut c_void),
            std::mem::size_of_val(&buf) as u32,
            &mut ret,
        )
        .is_ok()
        {
            let label = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
            let sid = label.Label.Sid;
            let count = GetSidSubAuthorityCount(sid);
            if !count.is_null() && *count > 0 {
                let rid = GetSidSubAuthority(sid, u32::from(*count - 1));
                if !rid.is_null() {
                    f.integrity = Some(policy::integrity_label(*rid));
                }
            }
        }
        let _ = CloseHandle(token);
    }
}

/// Exe name, integrity and elevation of process `pid` (read-only queries).
pub fn process_facts(pid: u32) -> ProcFacts {
    let mut f = ProcFacts::default();
    // SAFETY: requests a query-only handle; closed below.
    let h = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => {
            f.error = Some(last_error());
            return f;
        }
    };
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    // SAFETY: `buf` is writable for `size` units; `size` is updated with the length.
    if unsafe {
        QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size)
    }
    .is_ok()
    {
        let full = utf16_name(&buf, size as usize);
        f.exe = Some(policy::exe_file_name(&full).to_string());
    } else {
        f.error.get_or_insert(last_error());
    }
    token_facts(h, &mut f);
    // SAFETY: closes the handle opened above exactly once.
    let _ = unsafe { CloseHandle(h) };
    f
}

/// Facts about this process.
pub fn self_facts() -> ProcFacts {
    let mut f = ProcFacts::default();
    // SAFETY: pseudo-handle of the current process; it need not be closed.
    token_facts(unsafe { GetCurrentProcess() }, &mut f);
    f
}

/// (session id, input desktop reachable).
pub fn session() -> (u32, bool) {
    let mut sid = u32::MAX;
    // SAFETY: `sid` is a local out-parameter.
    let _ = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut sid) };
    // SAFETY: constant flags; the handle is closed right away.
    let input =
        match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) } {
            Ok(h) => {
                // SAFETY: closes the handle just opened.
                let _ = unsafe { CloseDesktop(h) };
                true
            }
            Err(_) => false,
        };
    (sid, input)
}

fn reg_sz(root: HKEY, key: PCWSTR, value: PCWSTR) -> Option<String> {
    let mut buf = [0u16; 256];
    let mut cb = std::mem::size_of_val(&buf) as u32;
    // SAFETY: `buf` is writable for `cb` bytes; RRF_RT_REG_SZ guarantees NUL termination.
    let r = unsafe {
        RegGetValueW(
            root,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut c_void),
            Some(&mut cb),
        )
    };
    if r.is_err() {
        return None;
    }
    let n = (cb as usize / 2).saturating_sub(1);
    Some(utf16_name(&buf, n))
}

fn reg_dword(root: HKEY, key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut v = 0u32;
    let mut cb = 4u32;
    // SAFETY: `v` is a writable u32 and `cb` its size.
    let r = unsafe {
        RegGetValueW(
            root,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut v as *mut u32 as *mut c_void),
            Some(&mut cb),
        )
    };
    r.is_ok().then_some(v)
}

/// OS facts from the registry (read-only): product, display version, build.UBR, edition,
/// plus the clipboard-history settings relevant to the Win+V check.
pub fn os_facts() -> Vec<(&'static str, String)> {
    let k = w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let s = |v| reg_sz(HKEY_LOCAL_MACHINE, k, v).unwrap_or_else(|| "?".into());
    let ubr =
        reg_dword(HKEY_LOCAL_MACHINE, k, w!("UBR")).map_or_else(|| "?".into(), |u| u.to_string());
    let opt = |v: Option<u32>| v.map_or_else(|| "unset".into(), |x| x.to_string());
    vec![
        ("product", s(w!("ProductName"))),
        ("display_version", s(w!("DisplayVersion"))),
        ("build", format!("{}.{}", s(w!("CurrentBuild")), ubr)),
        ("edition", s(w!("EditionID"))),
        ("installation_type", s(w!("InstallationType"))),
        (
            "clipboard_history_user",
            opt(reg_dword(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Clipboard"),
                w!("EnableClipboardHistory"),
            )),
        ),
        (
            "clipboard_history_policy",
            opt(reg_dword(
                HKEY_LOCAL_MACHINE,
                w!("SOFTWARE\\Policies\\Microsoft\\Windows\\System"),
                w!("AllowClipboardHistory"),
            )),
        ),
    ]
}

/// The user's Desktop folder (handles OneDrive redirection). Never written to the report.
pub fn desktop_dir() -> Option<std::path::PathBuf> {
    // SAFETY: documented call; the returned buffer is freed with CoTaskMemFree once copied.
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const c_void));
        s.map(std::path::PathBuf::from)
    }
}
