//! Autostart through the per-user `Run` key (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
//! Per-user only: no elevation, no services, no scheduled tasks.

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{PCWSTR, w};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_run_key() -> Result<HKEY, u32> {
    let mut key = HKEY::default();
    // SAFETY: constant NUL-terminated key path; `key` is a valid out-parameter; the key is
    // closed by every caller.
    let r = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if r.is_ok() { Ok(key) } else { Err(r.0) }
}

/// Writes (`Some(command line)`) or removes (`None`) the value `name` under the `Run` key.
/// `Err(win32_error)` if the registry could not be changed.
pub fn set(name: &str, command: Option<&str>) -> Result<(), u32> {
    let key = open_run_key()?;
    let name_w = wide(name);
    let result = match command {
        Some(cmd) => {
            let data: Vec<u8> = wide(cmd).iter().flat_map(|u| u.to_le_bytes()).collect();
            // SAFETY: `name_w` is NUL-terminated; `data` is a NUL-terminated UTF-16LE string
            // passed with its byte length; both outlive the call.
            unsafe { RegSetValueExW(key, PCWSTR(name_w.as_ptr()), None, REG_SZ, Some(&data)) }
        }
        None => {
            // SAFETY: `name_w` is NUL-terminated and outlives the call.
            let r = unsafe { RegDeleteValueW(key, PCWSTR(name_w.as_ptr())) };
            if r == ERROR_FILE_NOT_FOUND {
                windows::Win32::Foundation::ERROR_SUCCESS
            } else {
                r
            }
        }
    };
    // SAFETY: `key` was opened above and is closed exactly once.
    let _ = unsafe { RegCloseKey(key) };
    if result.is_ok() {
        Ok(())
    } else {
        Err(result.0)
    }
}

/// `true` if the value `name` exists under the `Run` key.
pub fn is_enabled(name: &str) -> bool {
    let Ok(key) = open_run_key() else {
        return false;
    };
    let name_w = wide(name);
    // SAFETY: existence query without data buffers; `name_w` is NUL-terminated.
    let r = unsafe { RegQueryValueExW(key, PCWSTR(name_w.as_ptr()), None, None, None, None) };
    // SAFETY: `key` was opened above and is closed exactly once.
    let _ = unsafe { RegCloseKey(key) };
    r.is_ok()
}
