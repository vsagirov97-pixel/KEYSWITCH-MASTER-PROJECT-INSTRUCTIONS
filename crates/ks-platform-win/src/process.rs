//! Process facts used by the gate (integrity level, executable name) and start-up hardening.
//!
//! Other processes are only ever opened with `PROCESS_QUERY_LIMITED_INFORMATION` (metadata
//! queries); nothing is read from or written to their memory.

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::System::Diagnostics::Debug::{
    SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX, SetErrorMode,
};
use windows::Win32::System::ErrorReporting::{WER_FAULT_REPORTING_FLAG_NOHEAP, WerSetFlags};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::PWSTR;

/// Start-up hardening (T-09): no system error dialogs for this process, and Windows Error
/// Reporting must not collect heap data (the heap may hold user text during an operation).
/// Call once at the very start of `main`.
pub fn harden() {
    // SAFETY: process-wide flag setters without pointer parameters.
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
        let _ = WerSetFlags(WER_FAULT_REPORTING_FLAG_NOHEAP);
    }
}

/// Integrity RID of the current process (0x2000 medium, 0x3000 high...); `None` if unknown.
pub fn current_rid() -> Option<u32> {
    // SAFETY: opens our own process token for query and closes it exactly once.
    unsafe {
        let mut tok = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut tok).ok()?;
        let r = token_rid(tok);
        let _ = CloseHandle(tok);
        r
    }
}

/// Integrity RID of process `pid`; `None` if the process or its token cannot be queried
/// (typical for elevated and protected processes — callers must fail closed).
pub fn process_rid(pid: u32) -> Option<u32> {
    // SAFETY: the process is opened with the minimal query right; both handles are closed
    // exactly once; the token is only queried.
    unsafe {
        let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut tok = HANDLE::default();
        let opened = OpenProcessToken(proc, TOKEN_QUERY, &mut tok);
        let _ = CloseHandle(proc);
        opened.ok()?;
        let r = token_rid(tok);
        let _ = CloseHandle(tok);
        r
    }
}

/// `true` if the target process runs at a higher integrity level than KeySwitch, or if
/// either level cannot be determined (fail closed, D-05).
pub fn is_elevated_target(pid: u32) -> bool {
    match (process_rid(pid), current_rid()) {
        (Some(target), Some(me)) => target > me,
        _ => true,
    }
}

/// # Safety
/// `tok` must be a valid token handle opened with `TOKEN_QUERY`.
unsafe fn token_rid(tok: HANDLE) -> Option<u32> {
    let mut len = 0u32;
    // SAFETY: size query with no buffer (expected to fail with the needed length).
    let _ = unsafe { GetTokenInformation(tok, TokenIntegrityLevel, None, 0, &mut len) };
    if len == 0 {
        return None;
    }
    // u64 backing store keeps the TOKEN_MANDATORY_LABEL suitably aligned.
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` has at least `len` writable bytes; on success it holds a
    // TOKEN_MANDATORY_LABEL whose SID points into the same buffer, which outlives the reads.
    unsafe {
        GetTokenInformation(
            tok,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut c_void),
            len,
            &mut len,
        )
        .ok()?;
        let tml = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
        let sid = tml.Label.Sid;
        let n = *GetSidSubAuthorityCount(sid);
        if n == 0 {
            return None;
        }
        Some(*GetSidSubAuthority(sid, u32::from(n) - 1))
    }
}

/// Lower-cased executable file name of process `pid` (`notepad.exe`); `None` if unknown.
pub fn exe_name(pid: u32) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: the handle is opened with the minimal query right and closed exactly once;
    // `buf`/`len` describe a writable buffer of `len` UTF-16 units.
    let ok = unsafe {
        let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let r =
            QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(proc);
        r.is_ok()
    };
    if !ok {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..(len as usize).min(buf.len())]);
    let name = path.rsplit(['\\', '/']).next()?.to_lowercase();
    (!name.is_empty()).then_some(name)
}
