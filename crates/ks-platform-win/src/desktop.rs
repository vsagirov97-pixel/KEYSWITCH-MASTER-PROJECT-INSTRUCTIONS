//! Detection of an interactive input desktop (needed for SendInput, focus, hotkey delivery).

use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, OpenInputDesktop,
};
use windows::Win32::System::Threading::GetCurrentProcessId;

/// Facts about the session the test process runs in.
#[derive(Clone, Copy, Debug)]
pub struct DesktopInfo {
    /// Terminal-services session id; 0 means the non-interactive services session.
    pub session_id: u32,
    /// `OpenInputDesktop` succeeded, i.e. this process can reach the desktop receiving input.
    pub input_desktop: bool,
}

impl DesktopInfo {
    /// `Some(reason)` if UI-driving tests cannot run meaningfully here.
    pub fn unusable_reason(&self) -> Option<&'static str> {
        if self.session_id == 0 {
            Some("process runs in session 0 (no interactive desktop)")
        } else if !self.input_desktop {
            Some("OpenInputDesktop failed (no access to the input desktop)")
        } else {
            None
        }
    }
}

/// Probes the current session.
pub fn probe() -> DesktopInfo {
    let mut session_id = u32::MAX;
    // SAFETY: `session_id` is a valid, writable u32 for the duration of the call.
    let session_ok = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session_id) };
    if session_ok.is_err() {
        session_id = u32::MAX;
    }
    // SAFETY: plain call with constant flags; the returned handle is closed right below.
    let desk = unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) };
    let input_desktop = match desk {
        Ok(h) => {
            // SAFETY: `h` was just returned by OpenInputDesktop and is closed exactly once.
            let _ = unsafe { CloseDesktop(h) };
            true
        }
        Err(_) => false,
    };
    DesktopInfo {
        session_id,
        input_desktop,
    }
}
