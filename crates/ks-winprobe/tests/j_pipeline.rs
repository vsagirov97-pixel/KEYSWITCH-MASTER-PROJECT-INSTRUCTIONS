//! Beta 1 scenario j: the **product pipeline** (`ks_engine::run` over
//! `ks_platform_win::adapter::WinPlatform`) against the fixture in a separate process.
//! The trigger is hotkey-equivalent: the test calls the same entry point the hotkey worker
//! calls. Synthetic strings only; reports carry outcome codes and booleans.
#![cfg(windows)]

mod common;

use std::sync::atomic::Ordering;

use ks_engine::{Config, Outcome, Refusal};
use ks_platform_win::adapter::WinPlatform;
use ks_winprobe::child::ChildFixture;
use ks_winprobe::fixture::{self, Hwnd};
use ks_winprobe::gate::Gate;
use ks_winprobe::report::{Report, Status};
use ks_winprobe::uia::{ComGuard, GET_TEXT_CALLS, Uia};

const WRONG_LAYOUT: &str = "ghbdtn";
const CONVERTED: &str = "привет";
const FIXTURE_EXE_NAME: &str = "ks-winprobe-fixture.exe";

/// Runs one pipeline operation against whatever has the focus now.
fn run_pipeline(r: &Report, config: &Config) -> Outcome {
    let com = ComGuard::mta().unwrap_or_else(|e| {
        common::finish(r, Status::Fail, &e);
        unreachable!()
    });
    let uia = Uia::new().unwrap_or_else(|e| {
        common::finish(r, Status::Fail, &e);
        unreachable!()
    });
    let converter = ks_core::builtin().expect("built-in layouts parse");
    let gate = Gate::new();
    let outcome = {
        let mut platform = WinPlatform::new(&uia, &gate);
        ks_engine::run(&mut platform, &converter, config)
    };
    drop(uia);
    drop(com);
    outcome
}

/// Fills `control`, puts the selection, focuses it in the foreground child.
fn prepare(
    c: &ChildFixture,
    control: Hwnd,
    text: &str,
    sel: (i32, i32),
    r: &mut Report,
    env: &common::Env,
) -> Result<(), &'static str> {
    if !fixture::set_text(control, text) && !text.is_empty() {
        return Err("WM_SETTEXT failed");
    }
    common::child_target(c, control, r, env)?;
    fixture::set_sel(control, sel.0, sel.1);
    Ok(())
}

fn is_converted(o: Outcome) -> bool {
    matches!(o, Outcome::Converted { .. })
}

fn record(r: &mut Report, o: Outcome) {
    r.field("outcome", o.code());
    if let Outcome::Converted {
        verified,
        layout_switched,
    } = o
    {
        r.field("verified", verified)
            .field("layout_switched", layout_switched);
    }
}

/// j1: caret after a word typed in the wrong layout in a classic EDIT (W1 path).
#[test]
fn j1_edit_last_word_converted() {
    let id = "j1.pipeline.edit_last_word";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    if let Err(why) = prepare(&c, k, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    record(&mut r, o);
    let mut problems = Vec::new();
    if !is_converted(o) {
        problems.push("outcome is not CONVERTED");
    }
    if fixture::get_text(k) != CONVERTED {
        problems.push("field does not hold the converted word");
    }
    if matches!(
        o,
        Outcome::Converted {
            verified: false,
            ..
        }
    ) {
        problems.push("post-check did not confirm the write");
    }
    common::verdict(&r, &problems);
}

/// j2: only the selection is replaced, the rest of the field is untouched.
#[test]
fn j2_edit_selection_converted() {
    let id = "j2.pipeline.edit_selection";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    if let Err(why) = prepare(&c, k, "abc ghbdtn xyz", (4, 10), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    record(&mut r, o);
    let mut problems = Vec::new();
    if !is_converted(o) {
        problems.push("outcome is not CONVERTED");
    }
    if fixture::get_text(k) != "abc привет xyz" {
        problems.push("field is not prefix + converted selection + suffix");
    }
    common::verdict(&r, &problems);
}

/// j3: a password field is refused without a single `GetText` and without a change.
#[test]
fn j3_password_refused() {
    let id = "j3.pipeline.password_refused";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.password;
    if let Err(why) = prepare(&c, k, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let calls_before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    let o = run_pipeline(&r, &Config::default());
    let calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - calls_before;
    record(&mut r, o);
    r.field("get_text_calls", calls);
    let mut problems = Vec::new();
    if o != Outcome::Refused(Refusal::Password) {
        problems.push("outcome is not PASSWORD");
    }
    if calls != 0 {
        problems.push("GetText was called on a password field");
    }
    if fixture::get_text(k) != WRONG_LAYOUT {
        problems.push("password field content changed");
    }
    common::verdict(&r, &problems);
}

/// j4: nothing to convert — empty field, and a caret right after a line break.
#[test]
fn j4_nothing_without_selection_or_word() {
    let id = "j4.pipeline.nothing";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let mut problems = Vec::new();
    let single = c.controls.single;
    if let Err(why) = prepare(&c, single, "", (0, 0), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    r.field("empty_outcome", o.code());
    if o != Outcome::Nothing {
        problems.push("empty field: outcome is not NOTHING");
    }
    if fixture::text_len(single) != 0 {
        problems.push("empty field changed");
    }
    let multi = c.controls.multi;
    let text = "ghbdtn\r\n";
    if let Err(why) = prepare(&c, multi, text, (8, 8), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    r.field("after_newline_outcome", o.code());
    if o != Outcome::Nothing {
        problems.push("caret after a line break: outcome is not NOTHING");
    }
    if fixture::get_text(multi) != text {
        problems.push("multi-line field changed");
    }
    common::verdict(&r, &problems);
}

/// j5: an excluded executable is refused and left untouched.
#[test]
fn j5_excluded_app_refused() {
    let id = "j5.pipeline.excluded_app";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    if let Err(why) = prepare(&c, k, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let config = Config {
        excluded: vec![FIXTURE_EXE_NAME.into()],
        ..Config::default()
    };
    let o = run_pipeline(&r, &config);
    record(&mut r, o);
    let mut problems = Vec::new();
    if o != Outcome::Refused(Refusal::ExcludedApp) {
        problems.push("outcome is not EXCLUDED_APP");
    }
    if fixture::get_text(k) != WRONG_LAYOUT {
        problems.push("excluded app content changed");
    }
    common::verdict(&r, &problems);
}

/// j6: RichEdit (not class `Edit`) goes through W3 (`SendInput` Unicode).
#[test]
fn j6_richedit_w3_converted() {
    let id = "j6.pipeline.richedit_w3";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let Some(k) = c.controls.rich else {
        return common::finish(&r, Status::Skip, "RICHEDIT50W is not available");
    };
    if let Err(why) = prepare(&c, k, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    record(&mut r, o);
    let mut problems = Vec::new();
    if !is_converted(o) {
        problems.push("outcome is not CONVERTED");
    }
    let arrived = fixture::wait_until(std::time::Duration::from_secs(2), || {
        fixture::get_text(k) == CONVERTED
    });
    r.field("arrived_ms", common::ms(arrived));
    if arrived.is_none() {
        problems.push("field does not hold the converted word");
    }
    common::verdict(&r, &problems);
}
