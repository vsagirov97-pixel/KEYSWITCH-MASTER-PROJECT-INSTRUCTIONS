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
use ks_winprobe::layout;
use ks_winprobe::report::{Report, Status};
use ks_winprobe::uia::{ComGuard, GET_TEXT_CALLS, Read, Uia};
use windows::Win32::UI::Accessibility::IUIAutomationElement;

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

/// `true` if a Russian keyboard layout is loaded in this session (without one the pipeline
/// cannot switch and `layout_switched = false` is the expected result).
fn ru_layout_loaded() -> bool {
    layout::loaded_layouts()
        .into_iter()
        .any(|h| h.lang_id() & 0x3FF == 0x19)
}

/// `true` if the `units` characters before the caret of `el` equal `expected`.
fn before_caret_is(u: &Uia, el: &IUIAutomationElement, expected: &str) -> bool {
    let units = expected.encode_utf16().count() as i32;
    matches!(u.read_before_caret(el, units), Ok(Some(Read::Text(t))) if t == expected)
}

fn record(r: &mut Report, o: Outcome) {
    r.field("outcome", o.code())
        .field("ru_layout_loaded", ru_layout_loaded());
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
    // Evidence for the post-check: an element obtained (and used) before the write versus
    // an element obtained after it.
    let diag = ComGuard::mta()
        .ok()
        .and_then(|com| Some((Uia::new().ok()?, com)));
    let old_element = diag.as_ref().and_then(|(u, _)| {
        let el = u.from_hwnd(k).ok()?;
        r.field(
            "old_element_reads_source",
            before_caret_is(u, &el, WRONG_LAYOUT),
        );
        Some(el)
    });
    let o = run_pipeline(&r, &Config::default());
    record(&mut r, o);
    if let (Some((u, _)), Some(old)) = (diag.as_ref(), old_element.as_ref()) {
        r.field(
            "old_element_reads_result",
            before_caret_is(u, old, CONVERTED),
        );
        if let Ok(fresh) = u.from_hwnd(k) {
            r.field(
                "fresh_element_reads_result",
                before_caret_is(u, &fresh, CONVERTED),
            );
        }
    }
    drop(old_element);
    if let Some((u, com)) = diag {
        drop(u);
        drop(com);
    }
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

/// j3: a password field is refused without a single `GetText` and without a change.
///
/// The content of an `ES_PASSWORD` edit cannot be read back from another process: the
/// control answers a cross-process `WM_GETTEXT` with an empty string. So "unchanged" is
/// checked through metadata the control does report — text length and selection — and the
/// readback length is recorded **before** the pipeline runs, to show what the readback
/// itself yields on an untouched field.
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
    let expected_len = WRONG_LAYOUT.encode_utf16().count();
    let len_before = fixture::text_len(k);
    let sel_before = fixture::get_sel(k);
    r.field("len_before", len_before).field(
        "cross_process_readback_units_before",
        fixture::get_text(k).encode_utf16().count(),
    );
    let calls_before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    let o = run_pipeline(&r, &Config::default());
    let calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - calls_before;
    record(&mut r, o);
    let len_after = fixture::text_len(k);
    let sel_after = fixture::get_sel(k);
    r.field("get_text_calls", calls)
        .field("len_after", len_after)
        .field("selection_unchanged", sel_before == sel_after)
        .field(
            "cross_process_readback_units_after",
            fixture::get_text(k).encode_utf16().count(),
        );
    let mut problems = Vec::new();
    if o != Outcome::Refused(Refusal::Password) {
        problems.push("outcome is not PASSWORD");
    }
    if calls != 0 {
        problems.push("GetText was called on a password field");
    }
    if len_before != expected_len {
        problems.push("password field was not set up (length differs before the run)");
    }
    if len_after != len_before || sel_after != sel_before {
        problems.push("password field length or selection changed");
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

/// j7: the keyboard layout follows a verified conversion and stays put after a refusal.
///
/// The test loads en-US and ru-RU itself (as e1 does), starts from en-US in the fixture's
/// thread, and unloads only the layouts it added.
#[test]
fn j7_layout_follows_verified_conversion_only() {
    use std::time::Duration;
    const PRIMARY_EN: u16 = 0x09;
    const PRIMARY_RU: u16 = 0x19;

    let id = "j7.pipeline.layout_switch";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    let thread = ks_winprobe::gate::window_thread(k);
    let primary = || layout::thread_layout(thread).lang_id() & 0x3FF;
    let wait_primary =
        |want: u16| fixture::wait_until(Duration::from_secs(3), || primary() == want);

    let loaded_before = layout::loaded_layouts();
    let initial = layout::thread_layout(thread);
    let en = layout::load("00000409");
    let ru = layout::load("00000419");
    r.field("load_0409", en.is_some())
        .field("load_0419", ru.is_some())
        .field("initial_layout", format!("{:04x}", initial.lang_id()));
    // Puts the fixture back on its initial layout, then unloads what this test added.
    let cleanup = |r: &mut Report| {
        if layout::thread_layout(thread) != initial && layout::request_switch(k, initial) {
            let back = fixture::wait_until(Duration::from_secs(3), || {
                layout::thread_layout(thread) == initial
            });
            r.field("restored_initial_ms", common::ms(back));
        }
        for h in [en, ru].into_iter().flatten() {
            if !loaded_before.contains(&h) {
                r.field("unloaded_added_layout", layout::unload(h));
            }
        }
    };
    let (Some(en), Some(_)) = (en, ru) else {
        cleanup(&mut r);
        return common::finish(
            &r,
            Status::Skip,
            "LoadKeyboardLayoutW failed for 0409 or 0419",
        );
    };
    let to_en = || {
        primary() == PRIMARY_EN
            || (layout::request_switch(k, en) && wait_primary(PRIMARY_EN).is_some())
    };

    let mut problems = Vec::new();
    let mut skip = None;
    'run: {
        // Part 1: a verified conversion EN -> RU switches the target's layout.
        if let Err(why) = prepare(&c, k, WRONG_LAYOUT, (6, 6), &mut r, &env) {
            skip = Some(why);
            break 'run;
        }
        if !to_en() {
            skip = Some("could not put the fixture on the en-US layout first");
            break 'run;
        }
        let o = run_pipeline(&r, &Config::default());
        record(&mut r, o);
        let observed = wait_primary(PRIMARY_RU);
        r.field("target_switched_ms", common::ms(observed));
        match o {
            Outcome::Converted {
                verified: true,
                layout_switched: true,
            } => {}
            Outcome::Converted { verified: true, .. } => {
                problems.push("verified conversion did not report layout_switched");
            }
            _ => problems.push("outcome is not a verified CONVERTED"),
        }
        if fixture::get_text(k) != CONVERTED {
            problems.push("field does not hold the converted word");
        }
        if observed.is_none() {
            problems.push("target thread layout did not become ru after the conversion");
        }

        // Part 2: a refusal (excluded app) leaves text and layout alone.
        if !to_en() {
            problems.push("could not return the fixture to en-US for the refusal part");
            break 'run;
        }
        if let Err(why) = prepare(&c, k, WRONG_LAYOUT, (6, 6), &mut r, &env) {
            skip = Some(why);
            break 'run;
        }
        let config = Config {
            excluded: vec![FIXTURE_EXE_NAME.into()],
            ..Config::default()
        };
        let refused = run_pipeline(&r, &config);
        r.field("refusal_outcome", refused.code());
        // A switch request is asynchronous: give a wrongly posted one time to land.
        let moved = fixture::wait_until(Duration::from_millis(700), || primary() != PRIMARY_EN);
        r.field("layout_changed_after_refusal", moved.is_some());
        if refused != Outcome::Refused(Refusal::ExcludedApp) {
            problems.push("refusal part: outcome is not EXCLUDED_APP");
        }
        if moved.is_some() {
            problems.push("layout changed after a refusal");
        }
        if fixture::get_text(k) != WRONG_LAYOUT {
            problems.push("refused operation changed the field");
        }
    }
    cleanup(&mut r);
    r.field(
        "loaded_layout_count_restored",
        layout::loaded_layouts().len() == loaded_before.len(),
    );
    match skip {
        Some(why) if problems.is_empty() => common::finish(&r, Status::Skip, why),
        _ => common::verdict(&r, &problems),
    }
}
