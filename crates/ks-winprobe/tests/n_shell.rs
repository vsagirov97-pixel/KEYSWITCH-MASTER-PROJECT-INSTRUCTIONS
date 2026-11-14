//! Beta 2 scenarios n: shell integration of the product modules, without the WebView2 UI.
//!
//! - n1: autostart through `ks_platform_win::autostart` (the module behind the app's
//!   `set_autostart`): the HKCU `Run` value `KeySwitch` is written on enable and removed on
//!   disable, checked independently with `reg.exe`; no other value of the key changes (value
//!   lines compared by name and data; the key-path header line is not compared, `reg.exe`
//!   omits it for a key without values); the original state is restored afterwards (also on
//!   panic). Evidence on a difference: counts and value NAMES only, never data.
//! - n2: single instance through `ks_platform_win::instance` across two processes: a second
//!   launch (this test binary re-run as a child) is told it is second — the app's `main`
//!   then returns — and the first instance receives the activation signal.
//!
//! The third scenario of this set (settings file corrupt / missing -> safe defaults) lives
//! in `app/src-tauri/src/settings.rs` (`corrupt_or_missing_file_gives_safe_defaults`): the
//! settings code is part of the app binary crate, which a test crate cannot link. It runs in
//! the `Windows build` job (`cargo test --workspace`). The installed executable is checked in
//! the same job (silent install / single instance with a damaged settings file / uninstall).
//!
//! WARNING: n1 writes the per-user autostart entry of the machine it runs on. Enabled only
//! with `KS_WINPROBE=1` (CI runner); it refuses to run if a `KeySwitch` value already exists,
//! so a real installation is never touched. Reports carry metadata only.
#![cfg(windows)]

mod common;

use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use ks_platform_win::{autostart, instance};
use ks_winprobe::report::{Report, Status};

/// Name of the `Run` value; must equal `APP_NAME` in `app/src-tauri/src/os.rs`.
const APP_NAME: &str = "KeySwitch";
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Environment variable that turns this binary into the n2 "second launch" child.
const CHILD_ENV: &str = "KS_N2_CHILD_INSTANCE";
const CHILD_SECOND: i32 = 10;
const CHILD_FIRST: i32 = 11;

/// `reg query` of the whole `Run` key, or of one value (`Some(name)`): exit success and the
/// output lines (lossy; only compared, never reported).
fn reg_query(value: Option<&str>) -> Option<(bool, Vec<String>)> {
    let mut cmd = Command::new("reg.exe");
    cmd.args(["query", RUN_KEY]);
    if let Some(v) = value {
        cmd.args(["/v", v]);
    }
    let out = cmd.output().ok()?;
    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim_end().to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    Some((out.status.success(), lines))
}

/// Snapshot of the `Run` key as `reg.exe` lists it: value lines parsed by name, our value
/// excluded. The key-path header line and subkey lines are kept apart: `reg.exe` prints the
/// header only when the key has something to list, so it is not part of "other values".
struct RunSnapshot {
    query_ok: bool,
    /// `reg.exe` printed the key-path header line.
    header: bool,
    /// Other value name (lower-case: registry names are case-insensitive) -> rest of the
    /// line (type and data). Data is only compared, never reported.
    values: std::collections::BTreeMap<String, String>,
    /// Subkey lines (full paths); compared only.
    subkeys: Vec<String>,
    /// Lines that are neither header, value nor subkey (should be empty).
    unparsed: usize,
}

/// Splits a `reg.exe` value line `    <name>    REG_<type>    <data>` into name and rest.
fn split_value_line(line: &str) -> Option<(String, String)> {
    let body = line.strip_prefix("    ")?;
    let pos = body.find("    REG_")?;
    let name = body[..pos].trim().to_owned();
    Some((name, body[pos..].trim().to_owned()))
}

fn run_snapshot() -> Option<RunSnapshot> {
    let (query_ok, lines) = reg_query(None)?;
    let key_upper = RUN_KEY
        .replacen("HKCU", "HKEY_CURRENT_USER", 1)
        .to_uppercase();
    let mut snap = RunSnapshot {
        query_ok,
        header: false,
        values: Default::default(),
        subkeys: Vec::new(),
        unparsed: 0,
    };
    for line in lines {
        if line.to_uppercase() == key_upper {
            snap.header = true;
        } else if let Some((name, rest)) = split_value_line(&line) {
            if !name.eq_ignore_ascii_case(APP_NAME) {
                snap.values.insert(name.to_lowercase(), rest);
            }
        } else if line.to_uppercase().starts_with(&format!("{key_upper}\\")) {
            snap.subkeys.push(line);
        } else {
            snap.unparsed += 1;
        }
    }
    Some(snap)
}

/// Names (never data) of other values that were added, removed or changed between snapshots.
fn changed_names(a: &RunSnapshot, b: &RunSnapshot) -> Vec<String> {
    let mut names: Vec<String> = a
        .values
        .iter()
        .filter(|(k, v)| b.values.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect();
    names.extend(
        b.values
            .keys()
            .filter(|k| !a.values.contains_key(*k))
            .cloned(),
    );
    names
}

/// "Other values unchanged": same other values (name and data) and same subkeys. The header
/// line and the query exit code are reported but not compared (they depend on whether the
/// key has any value at all, i.e. on our own value).
fn others_equal(a: &RunSnapshot, b: &RunSnapshot) -> bool {
    a.values == b.values && a.subkeys == b.subkeys
}

/// Report keys of [`diff_fields`]: query_ok, header, other_count, subkey_count,
/// unparsed_lines, changed_count, changed_names, subkeys_equal.
type DiffKeys = [&'static str; 8];
const ENABLED_KEYS: DiffKeys = [
    "enabled_query_ok",
    "enabled_header",
    "enabled_other_count",
    "enabled_subkey_count",
    "enabled_unparsed_lines",
    "enabled_changed_count",
    "enabled_changed_names",
    "enabled_subkeys_equal",
];
const DISABLED_KEYS: DiffKeys = [
    "disabled_query_ok",
    "disabled_header",
    "disabled_other_count",
    "disabled_subkey_count",
    "disabled_unparsed_lines",
    "disabled_changed_count",
    "disabled_changed_names",
    "disabled_subkeys_equal",
];

/// Adds the names-only evidence (value names, never data) of comparing `b` with `a`.
fn diff_fields(r: &mut Report, k: &DiffKeys, a: &RunSnapshot, b: &RunSnapshot) {
    let names = changed_names(a, b);
    r.field(k[0], b.query_ok)
        .field(k[1], b.header)
        .field(k[2], b.values.len())
        .field(k[3], b.subkeys.len())
        .field(k[4], b.unparsed)
        .field(k[5], names.len())
        .field(k[6], names.join("|"))
        .field(k[7], a.subkeys == b.subkeys);
}

/// Data of our value as `reg.exe` shows it (`None` if absent or not `REG_SZ`).
fn our_data() -> Option<String> {
    let (ok, lines) = reg_query(Some(APP_NAME))?;
    if !ok {
        return None;
    }
    let ours = format!("{APP_NAME} ");
    lines
        .iter()
        .find(|l| l.trim_start().starts_with(&ours))
        .and_then(|l| l.split_once("REG_SZ"))
        .map(|(_, data)| data.trim().to_owned())
}

/// Removes our value when dropped (the state before the test was "absent").
struct RestoreAbsent;

impl Drop for RestoreAbsent {
    fn drop(&mut self) {
        let _ = autostart::set(APP_NAME, None);
    }
}

#[test]
fn n1_autostart_run_value_written_and_removed() {
    let id = "n1.shell.autostart_run_value";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let before_exists = autostart::is_enabled(APP_NAME);
    let before_reg = reg_query(Some(APP_NAME)).map(|(ok, _)| ok);
    r.field("value_existed_before", before_exists);
    if before_exists || before_reg == Some(true) {
        return common::finish(
            &r,
            Status::Skip,
            "a KeySwitch Run value already exists; not touching a real installation",
        );
    }
    let Some(others_before) = run_snapshot() else {
        return common::finish(&r, Status::Fail, "reg.exe could not be started");
    };
    r.field("before_query_ok", others_before.query_ok)
        .field("before_header", others_before.header)
        .field("before_other_count", others_before.values.len())
        .field("before_subkey_count", others_before.subkeys.len())
        .field("before_unparsed_lines", others_before.unparsed);
    let _restore = RestoreAbsent;
    let mut problems: Vec<&'static str> = Vec::new();

    // Enable: same command shape as `os::set_autostart` (quoted exe + --background).
    let exe = std::env::current_exe().map(|p| p.display().to_string());
    let Ok(exe) = exe else {
        return common::finish(&r, Status::Fail, "current_exe unavailable");
    };
    let command = format!("\"{exe}\" --background");
    let enabled = autostart::set(APP_NAME, Some(&command));
    r.field("enable_ok", enabled.is_ok());
    if let Err(code) = enabled {
        r.field("enable_error", code);
        problems.push("enable failed");
    }
    r.field("enabled_is_enabled", autostart::is_enabled(APP_NAME));
    if !autostart::is_enabled(APP_NAME) {
        problems.push("is_enabled false after enable");
    }
    let data = our_data();
    r.field("enabled_reg_present", data.is_some());
    match data {
        None => problems.push("reg.exe does not see the value after enable"),
        // reg.exe prints in the console code page: compare only when the path is ASCII.
        Some(d) if command.is_ascii() && d != command => {
            problems.push("value data differs from the written command line");
        }
        Some(_) => {}
    }
    let others_enabled = run_snapshot();
    let same = others_enabled
        .as_ref()
        .is_some_and(|s| others_equal(&others_before, s));
    r.field("enabled_others_unchanged", same);
    if let Some(s) = &others_enabled {
        diff_fields(&mut r, &ENABLED_KEYS, &others_before, s);
    }
    if !same {
        problems.push("other Run values changed on enable");
    }

    // Disable, twice (the second call must be a no-op success).
    let disabled = autostart::set(APP_NAME, None);
    let disabled_again = autostart::set(APP_NAME, None);
    r.field("disable_ok", disabled.is_ok())
        .field("disable_again_ok", disabled_again.is_ok());
    if disabled.is_err() || disabled_again.is_err() {
        problems.push("disable failed");
    }
    let still = autostart::is_enabled(APP_NAME);
    let reg_still = reg_query(Some(APP_NAME)).map(|(ok, _)| ok);
    r.field("disabled_is_enabled", still)
        .field("disabled_reg_present", reg_still.unwrap_or(true));
    if still || reg_still != Some(false) {
        problems.push("value still present after disable");
    }
    let others_after = run_snapshot();
    let same = others_after
        .as_ref()
        .is_some_and(|s| others_equal(&others_before, s));
    r.field("disabled_others_unchanged", same);
    if let Some(s) = &others_after {
        diff_fields(&mut r, &DISABLED_KEYS, &others_before, s);
    }
    if !same {
        problems.push("other Run values changed on disable");
    }
    common::verdict(&r, &problems);
}

/// The n2 child: runs only when the parent sets [`CHILD_ENV`]; exits with the role it got.
#[test]
fn n2_child_second_launch() {
    let Some(name) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let name = name.to_string_lossy().into_owned();
    let code = match instance::acquire(&name) {
        instance::Acquire::Second => CHILD_SECOND,
        instance::Acquire::First(_) => CHILD_FIRST,
    };
    std::process::exit(code);
}

#[test]
fn n2_single_instance_second_launch_exits_and_signals() {
    let id = "n2.shell.single_instance";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    // A probe-only name, so a running KeySwitch on the machine is not involved.
    let name = format!("KeySwitch.probe-n2-{}", std::process::id());
    let first = match instance::acquire(&name) {
        instance::Acquire::First(i) => i,
        instance::Acquire::Second => {
            return common::finish(&r, Status::Fail, "fresh name reported as second");
        }
    };
    let (tx, rx) = mpsc::channel();
    first.on_activate(move || {
        let _ = tx.send(());
    });
    const NOT_SECOND: &str = "a later launch was not told it is second";
    const NOT_SIGNALLED: &str = "the first instance did not receive the activation signal";
    let mut problems: Vec<&'static str> = Vec::new();
    let Ok(exe) = std::env::current_exe() else {
        return common::finish(&r, Status::Fail, "current_exe unavailable");
    };
    for launch in 0..2 {
        let status = Command::new(&exe)
            .args([
                "n2_child_second_launch",
                "--exact",
                "--test-threads=1",
                "--quiet",
            ])
            .env(CHILD_ENV, &name)
            .env_remove(ks_winprobe::ENABLE_ENV)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let code = status.ok().and_then(|s| s.code());
        let signalled = rx.recv_timeout(Duration::from_secs(5)).is_ok();
        if launch == 0 {
            r.field("second_exit_code", code.unwrap_or(-1))
                .field("first_signalled", signalled);
        } else {
            r.field("third_exit_code", code.unwrap_or(-1))
                .field("first_signalled_again", signalled);
        }
        if code != Some(CHILD_SECOND) && !problems.contains(&NOT_SECOND) {
            problems.push(NOT_SECOND);
        }
        if !signalled && !problems.contains(&NOT_SIGNALLED) {
            problems.push(NOT_SIGNALLED);
        }
    }
    common::verdict(&r, &problems);
}
