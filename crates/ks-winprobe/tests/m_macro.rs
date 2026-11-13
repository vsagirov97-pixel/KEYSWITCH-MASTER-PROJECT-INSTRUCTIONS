//! Beta 2 step 1 scenarios m: Macro Studio **text macros** through the product pipeline
//! (`ks_engine::run_macro` over `ks_platform_win::adapter::WinPlatform`, and the worker's
//! macro job) against the fixture in a separate process. The trigger is hotkey-equivalent:
//! the test calls the entry point the macro hotkey calls.
//!
//! WARNING: m2 / m4c replace the clipboard of the machine they run on. Enabled only with
//! `KS_WINPROBE=1` (CI runner); all content is synthetic; reports carry metadata only.
#![cfg(windows)]

mod common;

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use ks_engine::{
    Config, Macro, MacroBody, MacroPermissions, NoCodeRunner, Outcome, Refusal, UserText,
};
use ks_platform_win::adapter::{self, WinPlatform};
use ks_platform_win::profiles::{self, Profile, ReadMode};
use ks_platform_win::w2::{ClipboardHold, Settled};
use ks_winprobe::child::ChildFixture;
use ks_winprobe::clipboard;
use ks_winprobe::fixture::{self, Hwnd};
use ks_winprobe::gate::Gate;
use ks_winprobe::hotkey;
use ks_winprobe::input;
use ks_winprobe::report::{Report, Status};
use ks_winprobe::uia::{ComGuard, GET_TEXT_CALLS, Uia};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_F15, VK_MENU, VK_SHIFT};

const USER: &str = "synthetic-user-clipboard";
const CF_UNICODETEXT: u32 = 13;

/// Profile row that enables W2 for the fixture executable.
const FIXTURE_PASTE: Profile = Profile {
    exe: "ks-winprobe-fixture.exe",
    paste: true,
    read: ReadMode::Uia,
    focus_classes: &[],
    signal_confirmed: false,
    verified_on_real_app: false,
};

fn text_macro(text: &str, allow_enter_tab: bool) -> Macro {
    Macro {
        body: MacroBody::Text(UserText::new(text.into())),
        permissions: MacroPermissions { allow_enter_tab },
    }
}

/// One macro operation with an explicit profile table and clipboard hold.
fn run(r: &Report, m: &Macro, profiles: &[Profile], hold: &ClipboardHold) -> (Outcome, Settled) {
    let fail = |e: &str| -> ! {
        common::finish(r, Status::Fail, e);
        unreachable!("finish panics on FAIL")
    };
    let com = ComGuard::mta().unwrap_or_else(|e| fail(&e));
    let uia = Uia::new().unwrap_or_else(|e| fail(&e));
    let gate = Gate::new();
    let result = {
        let mut platform = WinPlatform::with_clipboard(&uia, &gate, profiles, hold.clone());
        let o = ks_engine::run_macro(&mut platform, m, &mut NoCodeRunner, &Config::default());
        (o, platform.clipboard_settled())
    };
    drop(uia);
    drop(com);
    result
}

/// Default profiles (none for the fixture: W1 for `Edit`, W3 otherwise).
fn run_default(r: &Report, m: &Macro) -> Outcome {
    run(r, m, profiles::DEFAULTS, &ClipboardHold::new()).0
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

fn units(s: &str) -> i32 {
    s.encode_utf16().count() as i32
}

/// Line breaks in one form (`\r\n` and `\r` as `\n`), as RichEdit and Edit report them
/// differently.
fn unify(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// The field holds exactly `expected` (line breaks unified): no extra character, no extra
/// line break — i.e. no Enter reached the field.
fn field_is(k: Hwnd, expected: &str) -> bool {
    let want = unify(expected);
    fixture::wait_until(Duration::from_secs(2), || {
        unify(&fixture::get_text(k)) == want
    })
    .is_some()
}

fn record(r: &mut Report, o: Outcome) {
    r.field("outcome", o.code());
    if let Outcome::Inserted { verified } = o {
        r.field("verified", verified);
    }
}

fn text_item(s: &str) -> (u32, Vec<u8>) {
    (
        CF_UNICODETEXT,
        s.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
}

fn put_user(owner: Hwnd) -> bool {
    clipboard::open(owner)
        .and_then(|o| clipboard::put_items(&o, &[text_item(USER)]))
        .is_ok()
}

fn clipboard_is(owner: Hwnd, expected: &str) -> bool {
    clipboard::open(owner)
        .ok()
        .and_then(|o| clipboard::read_text(&o))
        .as_deref()
        == Some(expected)
}

/// m1: a short macro is inserted at the caret of a classic EDIT (W1) and replaces a
/// selection; verified; the keyboard layout is not touched.
#[test]
fn m1_short_text_in_edit() {
    let id = "m1.macro.edit_short";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    let mut problems = Vec::new();
    if let Err(why) = prepare(&c, k, "Dear ", (5, 5), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let layout_before = ks_winprobe::layout::thread_layout(ks_winprobe::gate::window_thread(k));
    let o = run_default(&r, &text_macro("colleagues,", false));
    record(&mut r, o);
    if o != (Outcome::Inserted { verified: true }) {
        problems.push("caret insert: outcome is not a verified MACRO_INSERTED");
    }
    if !field_is(k, "Dear colleagues,") {
        problems.push("caret insert: field does not hold prefix + macro");
    }
    let layout_after = ks_winprobe::layout::thread_layout(ks_winprobe::gate::window_thread(k));
    r.field("layout_unchanged", layout_before == layout_after);
    if layout_before != layout_after {
        problems.push("the keyboard layout changed");
    }
    // Selection replacement.
    if let Err(why) = prepare(&c, k, "one two three", (4, 7), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_default(&r, &text_macro("2", false));
    r.field("replace_outcome", o.code());
    if o != (Outcome::Inserted { verified: true }) || !field_is(k, "one 2 three") {
        problems.push("selection was not replaced by the macro");
    }
    common::verdict(&r, &problems);
}

/// m2: a long macro (> 200 characters) cannot be typed: refused in an unprofiled non-`Edit`
/// field (W3) without a change, and pasted (W2) in a profiled one with the user's clipboard
/// restored.
#[test]
fn m2_long_text_via_w2_restores_clipboard() {
    let id = "m2.macro.w2_long_restored";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let user = common::fixture(&r);
    let u = user.controls.main;
    let c = common::child(&r, "edit");
    let Some(k) = c.controls.rich else {
        return common::finish(&r, Status::Skip, "RICHEDIT50W is not available");
    };
    let long = "Synthetic macro text 0123456789. ".repeat(40);
    r.field("macro_units", units(&long));
    let mut problems = Vec::new();

    // Unprofiled: W3 only, which refuses more than 200 units.
    if let Err(why) = prepare(&c, k, "start ", (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_default(&r, &text_macro(&long, false));
    r.field("w3_outcome", o.code());
    if o != Outcome::Refused(Refusal::TooLong) {
        problems.push("unprofiled long macro is not refused as TOO_LONG");
    }
    if !field_is(k, "start ") {
        problems.push("unprofiled field changed although the macro was refused");
    }

    // Profiled: W2 paste, signal + positive post-check -> clipboard restored.
    if !put_user(u) {
        return common::finish(
            &r,
            Status::Fail,
            "could not set the synthetic user clipboard",
        );
    }
    if let Err(why) = prepare(&c, k, "start ", (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let hold = ClipboardHold::new();
    let (o, settled) = run(&r, &text_macro(&long, false), &[FIXTURE_PASTE], &hold);
    record(&mut r, o);
    let arrived = field_is(k, &format!("start {long}"));
    let user_back = clipboard_is(u, USER);
    r.field("settled", format!("{settled:?}"))
        .field("text_arrived", arrived)
        .field("user_clipboard_back", user_back)
        .field("held", hold.is_held());
    if o != (Outcome::Inserted { verified: true }) {
        problems.push("W2: outcome is not a verified MACRO_INSERTED");
    }
    if !arrived {
        problems.push("W2: field does not hold prefix + long macro");
    }
    if settled != Settled::Restored || !user_back {
        problems.push("W2: user clipboard was not restored");
    }
    if hold.is_held() {
        problems.push("W2: a snapshot is held although the paste was confirmed");
    }
    let _ = put_user(u);
    common::verdict(&r, &problems);
}

/// m3: a password field is refused without a single `GetText` and without a change.
#[test]
fn m3_password_field_refused() {
    let id = "m3.macro.password_refused";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.password;
    if let Err(why) = prepare(&c, k, "secret", (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let len_before = fixture::text_len(k);
    let sel_before = fixture::get_sel(k);
    let calls_before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    let o = run_default(&r, &text_macro("inserted", false));
    let calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - calls_before;
    record(&mut r, o);
    let len_after = fixture::text_len(k);
    let sel_after = fixture::get_sel(k);
    r.field("get_text_calls", calls)
        .field("len_before", len_before)
        .field("len_after", len_after)
        .field("selection_unchanged", sel_before == sel_after);
    let mut problems = Vec::new();
    if o != Outcome::Refused(Refusal::Password) {
        problems.push("outcome is not PASSWORD");
    }
    if calls != 0 {
        problems.push("GetText was called on a password field");
    }
    if len_before != 6 {
        problems.push("password field was not set up");
    }
    if len_after != len_before || sel_after != sel_before {
        problems.push("password field length or selection changed");
    }
    common::verdict(&r, &problems);
}

/// m4: `\n` / `\t` (D-18). Without the slot permission: refused, nothing changed. With it:
/// written as characters into a multi-line EDIT (W1, also mid-line: the post-check reads
/// only the covering lines, QA-B1-08) and pasted into a profiled field (W2); refused where
/// it could only be typed (W3) or where the field has no lines (single-line EDIT). The field
/// content is compared exactly, so a premature Enter (an extra line break) would fail.
#[test]
fn m4_line_breaks_need_permission_and_never_press_enter() {
    let id = "m4.macro.enter_permission";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let multi = c.controls.multi;
    let single = c.controls.single;
    let text = "line one\nline\ttwo";
    let mut problems = Vec::new();

    // a) no permission: refused before the field is touched.
    if let Err(why) = prepare(&c, multi, "head ", (5, 5), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_default(&r, &text_macro(text, false));
    r.field("a_no_permission", o.code());
    if o != Outcome::Refused(Refusal::ForbiddenChars) || !field_is(multi, "head ") {
        problems.push("a: line break without permission was not refused cleanly");
    }

    // b) permission, multi-line EDIT, caret at the end.
    if let Err(why) = prepare(&c, multi, "head ", (5, 5), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_default(&r, &text_macro(text, true));
    r.field("b_edit_end", o.code());
    if o != (Outcome::Inserted { verified: true }) {
        problems.push("b: permitted multi-line macro is not a verified MACRO_INSERTED");
    }
    if !field_is(multi, "head line one\r\nline\ttwo") {
        problems.push("b: multi-line EDIT does not hold exactly the macro text");
    }

    // b2) permission, multi-line EDIT, caret in the middle of the second line.
    let initial = "first\r\nsecond tail";
    let caret = units("first\r\nsecond ");
    if let Err(why) = prepare(&c, multi, initial, (caret, caret), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_default(&r, &text_macro("x\ny ", true));
    r.field("b2_edit_mid_line", o.code());
    if o != (Outcome::Inserted { verified: true }) {
        problems.push("b2: mid-line multi-line insert is not a verified MACRO_INSERTED");
    }
    if !field_is(multi, "first\r\nsecond x\r\ny tail") {
        problems.push("b2: multi-line EDIT does not hold the expected text");
    }

    // c) permission, single-line EDIT: refused, unchanged.
    if let Err(why) = prepare(&c, single, "single", (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_default(&r, &text_macro(text, true));
    r.field("c_single_line", o.code());
    if o != Outcome::Refused(Refusal::ForbiddenChars) || !field_is(single, "single") {
        problems.push("c: line break into a single-line EDIT was not refused cleanly");
    }

    if let Some(rich) = c.controls.rich {
        // d) permission, unprofiled RichEdit: only W3 is possible, which never sends Enter.
        if let Err(why) = prepare(&c, rich, "rich ", (5, 5), &mut r, &env) {
            return common::finish(&r, Status::Skip, why);
        }
        let o = run_default(&r, &text_macro(text, true));
        r.field("d_w3_refused", o.code());
        if o != Outcome::Refused(Refusal::ForbiddenChars) || !field_is(rich, "rich ") {
            problems.push("d: W3 target changed or did not refuse the line break");
        }

        // e) permission, profiled RichEdit: W2 paste of the lines, clipboard restored.
        let user = common::fixture(&r);
        let u = user.controls.main;
        if put_user(u) {
            if let Err(why) = prepare(&c, rich, "rich ", (5, 5), &mut r, &env) {
                return common::finish(&r, Status::Skip, why);
            }
            let hold = ClipboardHold::new();
            let (o, settled) = run(&r, &text_macro(text, true), &[FIXTURE_PASTE], &hold);
            let user_back = clipboard_is(u, USER);
            r.field("e_w2", o.code())
                .field("e_settled", format!("{settled:?}"))
                .field("e_user_clipboard_back", user_back);
            if !matches!(o, Outcome::Inserted { .. }) {
                problems.push("e: permitted multi-line W2 paste was not inserted");
            }
            if !field_is(rich, "rich line one\nline\ttwo") {
                problems.push("e: RichEdit does not hold exactly the macro text");
            }
            if settled == Settled::Restored && !user_back {
                problems.push("e: clipboard reported restored but is not the user's");
            }
            let _ = put_user(u);
        } else {
            r.field("e_w2", "skipped: no synthetic user clipboard");
        }
    } else {
        r.field("rich", "RICHEDIT50W is not available");
    }
    common::verdict(&r, &problems);
}

/// m5: the macro hotkey goes through `RegisterHotKey` like the conversion hotkey: the same
/// combination is reported as a conflict (`ERROR_HOTKEY_ALREADY_REGISTERED`), a different
/// one coexists with it. Delivery of an injected chord is recorded as a fact.
#[test]
fn m5_macro_hotkey_conflict() {
    let id = "m5.macro.hotkey_conflict";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let mods = hotkey::MOD_CONTROL_BIT | hotkey::MOD_ALT_BIT | hotkey::MOD_SHIFT_BIT;
    let f14 = 0x85; // VK_F14
    let conversion = match hotkey::register(mods, f14, || {}) {
        Ok(h) => h,
        Err(code) => {
            r.field("first_error", code);
            let status = if env.desktop.unusable_reason().is_some() {
                Status::Skip
            } else {
                Status::Fail
            };
            return common::finish(&r, status, "RegisterHotKey(Ctrl+Alt+Shift+F14) failed");
        }
    };
    let mut problems: Vec<&'static str> = Vec::new();
    match hotkey::register(mods, f14, || {}) {
        Ok(_) => problems.push("same combination for the macro was not reported as a conflict"),
        Err(code) => {
            r.field("duplicate_error", code);
            if code != hotkey::ERROR_HOTKEY_ALREADY_REGISTERED {
                problems.push("duplicate failed with an unexpected error code");
            }
        }
    }
    let (tx, rx) = mpsc::channel::<()>();
    let macro_hotkey = hotkey::register(mods, u32::from(VK_F15.0), move || {
        let _ = tx.send(());
    });
    r.field("distinct_registered", macro_hotkey.is_ok());
    if macro_hotkey.is_err() {
        problems.push("a distinct macro hotkey could not be registered next to the first");
    }
    if macro_hotkey.is_ok() && env.desktop.unusable_reason().is_none() {
        let injected = input::send_vk_chord(&[VK_CONTROL, VK_MENU, VK_SHIFT, VK_F15]);
        let got = rx.recv_timeout(Duration::from_secs(2)).is_ok();
        r.field("injected_events", injected)
            .field("macro_callback_observed", got);
    }
    drop(macro_hotkey);
    drop(conversion);
    common::verdict(&r, &problems);
}

/// m6: the worker's macro job (what the macro hotkey triggers): the slot comes from the
/// closure, the outcome is reported, the conversion job is not involved.
#[test]
fn m6_worker_macro_job() {
    let id = "m6.macro.worker_job";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    if let Err(why) = prepare(&c, k, "ab", (2, 2), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let converter = ks_core::builtin().expect("built-in layouts parse");
    let (tx, rx) = mpsc::channel::<Outcome>();
    let Some(worker) = adapter::spawn_worker(
        converter,
        || Some(Config::default()),
        || Some(text_macro("cd", false)),
        NoCodeRunner,
        move |o| {
            let _ = tx.send(o);
        },
    ) else {
        return common::finish(&r, Status::Fail, "worker thread did not start");
    };
    worker.trigger_macro();
    let o = rx.recv_timeout(Duration::from_secs(10));
    let mut problems = Vec::new();
    match o {
        Ok(o) => {
            record(&mut r, o);
            if o != (Outcome::Inserted { verified: true }) {
                problems.push("worker macro job did not insert verified");
            }
        }
        Err(_) => problems.push("no outcome from the worker"),
    }
    if !field_is(k, "abcd") {
        problems.push("field does not hold the macro text");
    }
    drop(worker);
    common::verdict(&r, &problems);
}
