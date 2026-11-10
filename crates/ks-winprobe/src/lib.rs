//! Spike S-2a: Windows integration probe.
//!
//! This crate is **not product code**. It contains a Win32 test fixture (a window with
//! classic `EDIT` and `RICHEDIT50W` controls, created inside the test process) and thin
//! wrappers over UI Automation, `EM_REPLACESEL`, `SendInput(KEYEVENTF_UNICODE)`, the
//! clipboard, keyboard layouts and `RegisterHotKey`. Integration tests in `tests/` use them
//! to record real behaviour on a Windows CI runner.
//!
//! Ground rules (CLAUDE.md, SECURITY_REVIEW):
//! - no keyboard hooks, no injection into other processes, no networking;
//! - only synthetic strings are used, and no text content is ever printed — reports carry
//!   lengths, counts, ids and booleans only;
//! - every `unsafe` block carries a `// SAFETY:` justification.
//!
//! On non-Windows targets the crate compiles to (almost) nothing.

pub mod report;

#[cfg(windows)]
pub mod clipboard;
#[cfg(windows)]
pub mod desktop;
#[cfg(windows)]
pub mod fixture;
#[cfg(windows)]
pub mod hotkey;
#[cfg(windows)]
pub mod input;
#[cfg(windows)]
pub mod layout;
#[cfg(windows)]
pub mod uia;

use std::sync::{Mutex, MutexGuard};

/// Environment variable that enables the probe tests. Without it every test reports SKIP,
/// so `cargo test --workspace` in other CI jobs never drives the desktop in parallel.
pub const ENABLE_ENV: &str = "KS_WINPROBE";

/// `true` when the probe tests were explicitly enabled (`KS_WINPROBE=1`).
pub fn probe_enabled() -> bool {
    std::env::var_os(ENABLE_ENV).is_some_and(|v| v == "1")
}

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialises tests that touch shared desktop state (foreground, focus, clipboard, layouts).
pub fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
