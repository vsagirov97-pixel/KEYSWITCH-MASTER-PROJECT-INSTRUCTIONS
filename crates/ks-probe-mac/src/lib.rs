//! Spike M-2a: macOS integration probe (see `docs/MACOS_PLATFORM_MAP.md` §8).
//!
//! Not a product crate. It exists to confirm or refute platform hypotheses on GitHub
//! Actions macOS runners against our own AppKit fixture (`ks-mac-fixture`).
//!
//! Rules this crate follows (CLAUDE.md, D-27):
//! - no `CGEventTap`, no keyboard monitors, no Input Monitoring permission;
//!   hotkeys only via Carbon `RegisterEventHotKey`;
//! - text read from fields is held in [`text::ProbeText`] (no `Debug`/`Display`) and is
//!   never printed; reports contain only lengths, booleans, error codes and AX metadata;
//! - all strings used are synthetic constants from [`synthetic`];
//! - secure text fields are detected by role/subrole and their value is never read.
//!
//! On non-macOS targets this library is empty.
#![cfg(target_os = "macos")]

pub mod ax;
pub mod carbon;
pub mod events;
pub mod pasteboard;
pub mod report;
pub mod synthetic;
pub mod text;
