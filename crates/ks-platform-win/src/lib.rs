//! KeySwitch Windows integration (product code).
//!
//! The mechanisms here were proven by spikes S-2a/S-2b (`crates/ks-winprobe`, whose tests now
//! exercise this crate): target fingerprint and generation gate, password / elevated /
//! secure-desktop / self / console refusals, guarded UI Automation reads, write methods W1
//! (`EM_REPLACESEL`, class `Edit` only, D-30) and W3 (`SendInput` with `KEYEVENTF_UNICODE`),
//! keyboard layout switch and `RegisterHotKey` (D-14). Shell integration added for Beta 1:
//! tray icon, HKCU `Run` autostart, single-instance guard, start-up hardening.
//!
//! Ground rules (CLAUDE.md, SECURITY_REVIEW):
//! - no keyboard hooks, no injection into other processes, no thread-input attachment, no
//!   networking, no privilege elevation;
//! - user text is never logged or formatted; errors carry API names and codes only;
//! - this is the only product crate with `unsafe`; every block has a `// SAFETY:` comment.
//!
//! The W2 clipboard method ([`clipboard`], [`w2`]; D-26 / CISO Q-1 contract, verified in S-2b)
//! is used only for applications listed in the [`profiles`] table.
//!
//! On non-Windows targets only [`policy`], [`profiles`] and [`tray_icon`] (pure code and data)
//! are compiled.

pub mod policy;
pub mod profiles;
pub mod tray_icon;

#[cfg(windows)]
pub mod adapter;
#[cfg(windows)]
pub mod autostart;
#[cfg(windows)]
pub mod clipboard;
#[cfg(windows)]
pub mod desktop;
#[cfg(windows)]
pub mod edit;
#[cfg(windows)]
pub mod gate;
#[cfg(windows)]
pub mod hotkey;
#[cfg(windows)]
pub mod input;
#[cfg(windows)]
pub mod instance;
#[cfg(windows)]
pub mod layout;
#[cfg(windows)]
pub mod popup;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod tray;
#[cfg(windows)]
pub mod uia;
#[cfg(windows)]
pub mod w2;

#[cfg(windows)]
pub use hwnd::Hwnd;

#[cfg(windows)]
mod hwnd {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HWND;

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
}
