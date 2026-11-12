//! Shared helpers for the S-2a probe tests. Synthetic strings only; reports carry metadata.
#![allow(dead_code)] // Each test binary uses a different subset.

use std::sync::MutexGuard;

use ks_winprobe::child::ChildFixture;
use ks_winprobe::desktop::{self, DesktopInfo};
use ks_winprobe::fixture::{Fixture, Foreground, Hwnd};
use ks_winprobe::report::{Report, Status, skip_disabled};
use ks_winprobe::{probe_enabled, serial};

/// Per-test environment: serialisation guard + desktop facts.
pub struct Env {
    _guard: MutexGuard<'static, ()>,
    /// Session / input-desktop facts.
    pub desktop: DesktopInfo,
}

/// `None` (and a SKIP line) unless the probe is enabled with `KS_WINPROBE=1`.
pub fn start(id: &'static str) -> Option<Env> {
    if !probe_enabled() {
        skip_disabled(id);
        return None;
    }
    Some(Env {
        _guard: serial(),
        desktop: desktop::probe(),
    })
}

/// New report pre-filled with the desktop facts.
pub fn report(id: &'static str, env: &Env) -> Report {
    let mut r = Report::new(id);
    r.field("session_id", env.desktop.session_id)
        .field("input_desktop", env.desktop.input_desktop);
    r
}

/// Emits the line; a FAIL also fails the test. The panic message carries no text content.
pub fn finish(r: &Report, status: Status, reason: &str) {
    r.emit(status, reason);
    assert!(
        status != Status::Fail,
        "probe scenario failed, see the KS-PROBE line above"
    );
}

/// Spawns the fixture or fails the scenario (the error is a Win32 code, not text).
pub fn fixture(r: &Report) -> Fixture {
    match Fixture::spawn() {
        Ok(f) => f,
        Err(e) => {
            finish(r, Status::Fail, &format!("fixture: {e}"));
            unreachable!("finish panics on FAIL")
        }
    }
}

/// Tries to make the fixture foreground; records how.
pub fn foreground(fx: &Fixture, r: &mut Report) -> Foreground {
    let fg = fx.bring_to_foreground();
    r.field("foreground", fg.label());
    fg
}

/// PASS if `problems` is empty, FAIL otherwise (reason = the static problem labels).
pub fn verdict(r: &Report, problems: &[&'static str]) {
    if problems.is_empty() {
        finish(r, Status::Pass, "");
    } else {
        finish(r, Status::Fail, &problems.join("; "));
    }
}

/// Path of the S-2b fixture executable (built by Cargo for integration tests).
pub const FIXTURE_EXE: &str = env!("CARGO_BIN_EXE_ks-winprobe-fixture");

/// Spawns the separate fixture process in `mode` or fails the scenario.
pub fn child(r: &Report, mode: &str) -> ChildFixture {
    match ChildFixture::spawn(std::path::Path::new(FIXTURE_EXE), mode) {
        Ok(c) => c,
        Err(e) => {
            finish(r, Status::Fail, &format!("child fixture: {e}"));
            unreachable!("finish panics on FAIL")
        }
    }
}

/// Makes the child fixture foreground and focuses `control`; `Err(reason)` = SKIP.
pub fn child_target(
    c: &ChildFixture,
    control: Hwnd,
    r: &mut Report,
    env: &Env,
) -> Result<(), &'static str> {
    if let Some(why) = env.desktop.unusable_reason() {
        return Err(why);
    }
    let fg = c.bring_to_foreground();
    r.field("child_foreground", fg.label());
    if !fg.ok() {
        return Err("child fixture could not become the foreground window");
    }
    if !c.focus(control) {
        return Err("SetFocus in the child fixture failed");
    }
    Ok(())
}

/// Labels for fields: elapsed milliseconds or -1.
pub fn ms(d: Option<std::time::Duration>) -> i64 {
    d.map_or(-1, |d| d.as_millis() as i64)
}
