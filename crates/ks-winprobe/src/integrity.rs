//! Integrity levels (P-18) and a Low-IL helper launcher for the "hotkey injected from a
//! lower integrity level" check (S-2a security review, f2).
//!
//! The launcher only ever **lowers** the integrity level of a copy of our own token
//! (documented pattern "Designing Applications to Run at a Low Integrity Level"); it never
//! raises privileges and never touches another process.

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Security::{
    AllocateAndInitializeSid, DuplicateTokenEx, FreeSid, GetLengthSid, GetSidSubAuthority,
    GetSidSubAuthorityCount, GetTokenInformation, PSID, SECURITY_MANDATORY_LABEL_AUTHORITY,
    SID_AND_ATTRIBUTES, SecurityImpersonation, SetTokenInformation, TOKEN_ADJUST_DEFAULT,
    TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel,
    TokenPrimary,
};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateProcessAsUserW, GetCurrentProcess, GetExitCodeProcess,
    OpenProcessToken, PROCESS_INFORMATION, STARTUPINFOW, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR};

/// `SE_GROUP_INTEGRITY` (winnt.h); defined here to avoid another `windows` feature.
const SE_GROUP_INTEGRITY: u32 = 0x20;

/// `SECURITY_MANDATORY_LOW_RID`.
pub const LOW_RID: u32 = 0x1000;

/// Integrity RID of the current process (0x1000 low, 0x2000 medium, 0x3000 high, 0x4000
/// system); `None` if it cannot be read.
pub fn current_rid() -> Option<u32> {
    // SAFETY: opens our own process token for query and closes it.
    unsafe {
        let mut tok = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut tok).ok()?;
        let r = token_rid(tok);
        let _ = CloseHandle(tok);
        r
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
    // TOKEN_MANDATORY_LABEL whose SID points into the same buffer.
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

/// Runs `cmdline` (full command line, first token = exe path) at **Low** integrity, waits up
/// to `wait_ms` and returns its exit code. `Err` describes which step failed (Win32 error).
pub fn run_low_il(cmdline: &str, wait_ms: u32) -> Result<u32, String> {
    let mut cmd: Vec<u16> = cmdline.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: every handle opened here is closed before returning; the SID is freed; the
    // TOKEN_MANDATORY_LABEL passed to SetTokenInformation points to the live SID; `cmd` is a
    // writable NUL-terminated buffer as CreateProcessAsUserW requires.
    unsafe {
        let mut tok = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_ASSIGN_PRIMARY,
            &mut tok,
        )
        .map_err(|e| format!("OpenProcessToken: {e}"))?;
        let mut dup = HANDLE::default();
        let r = DuplicateTokenEx(
            tok,
            TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_ASSIGN_PRIMARY,
            None,
            SecurityImpersonation,
            TokenPrimary,
            &mut dup,
        );
        let _ = CloseHandle(tok);
        r.map_err(|e| format!("DuplicateTokenEx: {e}"))?;
        let mut sid = PSID::default();
        if let Err(e) = AllocateAndInitializeSid(
            &SECURITY_MANDATORY_LABEL_AUTHORITY,
            1,
            LOW_RID,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut sid,
        ) {
            let _ = CloseHandle(dup);
            return Err(format!("AllocateAndInitializeSid: {e}"));
        }
        let tml = TOKEN_MANDATORY_LABEL {
            Label: SID_AND_ATTRIBUTES {
                Sid: sid,
                Attributes: SE_GROUP_INTEGRITY,
            },
        };
        let set = SetTokenInformation(
            dup,
            TokenIntegrityLevel,
            &tml as *const TOKEN_MANDATORY_LABEL as *const c_void,
            std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(sid),
        );
        let _ = FreeSid(sid);
        if let Err(e) = set {
            let _ = CloseHandle(dup);
            return Err(format!("SetTokenInformation(Low): {e}"));
        }
        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        let created = CreateProcessAsUserW(
            Some(dup),
            PCWSTR::null(),
            Some(PWSTR(cmd.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &si,
            &mut pi,
        );
        let _ = CloseHandle(dup);
        created.map_err(|e| format!("CreateProcessAsUserW: {e}"))?;
        let waited = WaitForSingleObject(pi.hProcess, wait_ms);
        let mut code = u32::MAX;
        let got = GetExitCodeProcess(pi.hProcess, &mut code);
        let _ = CloseHandle(pi.hThread);
        let _ = CloseHandle(pi.hProcess);
        if waited != WAIT_OBJECT_0 {
            return Err("low-IL helper did not exit in time".into());
        }
        got.map_err(|e| format!("GetExitCodeProcess: {e}"))?;
        Ok(code)
    }
}
