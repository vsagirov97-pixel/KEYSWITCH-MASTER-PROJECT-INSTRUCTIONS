//! BL-QA-B1 scenario k: independent QA of the Beta 1 **product pipeline**
//! (`ks_engine::run` over `ks_platform_win::adapter::WinPlatform`) against the fixture in a
//! separate process. Same harness as scenario j. Synthetic strings only; KS-PROBE lines carry
//! outcome codes, flags and lengths - never text.
//!
//! k01 multi-line EDIT, caret right after a line break in the middle of the text
//! k02 multi-line EDIT, caret after a word in the middle of the text
//! k03 multi-line EDIT, selection spanning a line break (line break must survive)
//! k04 RichEdit (W3), selection spanning a line break -> refused, unchanged
//! k05 read-only EDIT -> refused, unchanged
//! k06 selection over the limit -> refused, unchanged; exactly at the limit -> converted
//! k07 one-step undo restores the original (caret mode and selection mode)
//! k08 two operations in a row (repeat on the converted word, then a new word)
//! k09 focus moves to another window right before the write (W1) -> nothing written anywhere
//! k10 same for W3 (SendInput would land in whatever has the focus)
//! k11 target process exits before the read / before the write -> clean stop
//! k12 excluded-app match is case-insensitive (executable with a mixed-case file name)
#![cfg(windows)]

mod common;

use std::cell::Cell;
use std::time::Duration;

use ks_engine::{Config, Failure, Outcome, Platform, Refusal, Stop, TargetInfo, UserText};
use ks_platform_win::adapter::WinPlatform;
use ks_winprobe::child::ChildFixture;
use ks_winprobe::fixture::{self, Hwnd};
use ks_winprobe::gate::Gate;
use ks_winprobe::report::{Report, Status};
use ks_winprobe::uia::{ComGuard, Uia};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Controls::{EM_CANUNDO, EM_SETREADONLY, EM_UNDO};
use windows::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW};

const WRONG: &str = "ghbdtn";
const RIGHT: &str = "привет";

/// Where the test interferes with the operation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum At {
    Never,
    BeforeRead,
    BeforeWrite,
}

/// The product platform with one test hook fired right before a pipeline step.
struct Hooked<'a, 'h> {
    inner: WinPlatform<'a>,
    at: At,
    hook: Option<Box<dyn FnOnce() + 'h>>,
}

impl Hooked<'_, '_> {
    fn fire(&mut self, at: At) {
        if self.at == at
            && let Some(hook) = self.hook.take()
        {
            hook();
        }
    }
}

impl Platform for Hooked<'_, '_> {
    fn begin(&mut self) -> Result<TargetInfo, Stop> {
        self.inner.begin()
    }
    fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        self.fire(At::BeforeRead);
        self.inner.read_selection(max_units)
    }
    fn read_before_caret(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        self.inner.read_before_caret(max_units)
    }
    fn select_before_caret(&mut self, units: usize) -> Result<(), Stop> {
        self.inner.select_before_caret(units)
    }
    fn restore_caret(&mut self, selected: &UserText) {
        self.inner.restore_caret(selected);
    }
    fn write(&mut self, text: &UserText) -> Result<(), Stop> {
        self.fire(At::BeforeWrite);
        self.inner.write(text)
    }
    fn verify(&mut self, expected: &UserText) -> bool {
        self.inner.verify(expected)
    }
    fn switch_layout(&mut self, lang: &str) -> bool {
        self.inner.switch_layout(lang)
    }
}

/// Runs one pipeline operation against whatever has the focus now, with an optional hook.
fn run_hooked<'h>(
    r: &Report,
    config: &Config,
    at: At,
    hook: Option<Box<dyn FnOnce() + 'h>>,
) -> Outcome {
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
        let mut platform = Hooked {
            inner: WinPlatform::new(&uia, &gate),
            at,
            hook,
        };
        ks_engine::run(&mut platform, &converter, config)
    };
    drop(uia);
    drop(com);
    outcome
}

fn run_pipeline(r: &Report, config: &Config) -> Outcome {
    run_hooked(r, config, At::Never, None)
}

/// Fills `control`, focuses it in the foreground child, then puts the selection.
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

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

fn is_converted(o: Outcome) -> bool {
    matches!(o, Outcome::Converted { .. })
}

fn is_verified(o: Outcome) -> bool {
    matches!(o, Outcome::Converted { verified: true, .. })
}

/// `EM_SETREADONLY(TRUE)`; `true` if the control accepted it.
fn set_read_only(h: Hwnd) -> bool {
    let mut result = 0usize;
    // SAFETY: integer-only message (wparam = TRUE, lparam = 0); `result` is a valid
    // out-parameter for the duration of the synchronous call.
    let ok = unsafe {
        SendMessageTimeoutW(
            h.raw(),
            EM_SETREADONLY,
            WPARAM(1),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            2000,
            Some(&mut result),
        )
    };
    ok.0 != 0 && result != 0
}

/// k01: caret right after a line break in the middle of a multi-line EDIT: nothing to do.
#[test]
fn k01_multiline_caret_after_newline() {
    let id = "k01.qa.multiline_caret_after_newline";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.multi;
    let text = "abc\r\nghbdtn\r\nxyz";
    // Caret between "\r\n" and the second line.
    if let Err(why) = prepare(&c, k, text, (5, 5), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    let unchanged = fixture::get_text(k) == text;
    r.field("outcome", o.code())
        .field("unchanged", unchanged)
        .field("len_after", fixture::text_len(k));
    let mut problems = Vec::new();
    if o != Outcome::Nothing {
        problems.push("outcome is not NOTHING");
    }
    if !unchanged {
        problems.push("field changed");
    }
    common::verdict(&r, &problems);
}

/// k02: caret after a wrong-layout word on the middle line: only that word is replaced.
#[test]
fn k02_multiline_caret_mid_text() {
    let id = "k02.qa.multiline_caret_mid_text";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.multi;
    let text = "abc\r\nghbdtn\r\nxyz";
    // Caret right after "ghbdtn" (3 + 2 + 6 = 11).
    if let Err(why) = prepare(&c, k, text, (11, 11), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    let after = fixture::get_text(k);
    let expected = after == "abc\r\nпривет\r\nxyz";
    r.field("outcome", o.code())
        .field("verified", is_verified(o))
        .field("expected_content", expected)
        .field("unchanged", after == text)
        .field("len_after", units(&after));
    let mut problems = Vec::new();
    if !is_converted(o) {
        problems.push("outcome is not CONVERTED");
    }
    if !expected && after != text {
        problems.push("field is neither the original nor the expected text (corruption)");
    } else if !expected {
        problems.push("word on the middle line was not converted");
    }
    if is_converted(o) && !is_verified(o) {
        problems.push("post-check did not confirm the write");
    }
    common::verdict(&r, &problems);
}

/// k03: a selection that spans a line break in a multi-line EDIT (W1). Whatever the outcome,
/// the line break must stay `\r\n` and nothing outside the selection may change.
#[test]
fn k03_multiline_selection_keeps_line_break() {
    let id = "k03.qa.multiline_selection_crlf";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.multi;
    let text = "abc ghbdtn\r\nvbh xyz";
    // Selection = "ghbdtn\r\nvbh" (4..15).
    if let Err(why) = prepare(&c, k, text, (4, 15), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    let after = fixture::get_text(k);
    let converted = after == "abc привет\r\nмир xyz";
    let unchanged = after == text;
    r.field("outcome", o.code())
        .field("verified", is_verified(o))
        .field("converted_content", converted)
        .field("unchanged", unchanged)
        .field("len_before", units(text))
        .field("len_after", units(&after))
        .field("crlf_count_after", after.matches("\r\n").count())
        .field(
            "lone_cr_or_lf_after",
            after.replace("\r\n", "").contains(['\r', '\n']),
        );
    let mut problems = Vec::new();
    if !converted && !unchanged {
        problems.push("field is neither the original nor the expected text (corruption)");
    }
    if is_converted(o) && !converted {
        problems.push("outcome CONVERTED but the content is not the expected text");
    }
    if !is_converted(o) && !unchanged {
        problems.push("outcome is not CONVERTED but the field changed");
    }
    common::verdict(&r, &problems);
}

/// k04: RichEdit goes through W3, which must not inject a line break: refused, unchanged.
#[test]
fn k04_richedit_multiline_selection_refused() {
    let id = "k04.qa.richedit_multiline_refused";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let Some(k) = c.controls.rich else {
        return common::finish(&r, Status::Skip, "RICHEDIT50W is not available");
    };
    if let Err(why) = prepare(&c, k, "ghbdtn\r\nvbh", (0, -1), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let before = fixture::get_text(k);
    let o = run_pipeline(&r, &Config::default());
    // W3 is asynchronous: give stray input time to arrive before comparing.
    std::thread::sleep(Duration::from_millis(300));
    let unchanged = fixture::get_text(k) == before;
    r.field("outcome", o.code())
        .field("unchanged", unchanged)
        .field("len_before", units(&before))
        .field("len_after", fixture::text_len(k));
    let mut problems = Vec::new();
    if is_converted(o) {
        problems.push("a selection with a line break was written through W3");
    }
    if !unchanged {
        problems.push("field changed");
    }
    common::verdict(&r, &problems);
}

/// k05: a read-only EDIT is refused and left untouched.
#[test]
fn k05_readonly_edit_refused() {
    let id = "k05.qa.readonly_refused";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    if let Err(why) = prepare(&c, k, WRONG, (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let read_only = set_read_only(k);
    r.field("read_only_set", read_only);
    if !read_only {
        return common::finish(&r, Status::Skip, "EM_SETREADONLY was not accepted");
    }
    let sel_before = fixture::get_sel(k);
    let o = run_pipeline(&r, &Config::default());
    let unchanged = fixture::get_text(k) == WRONG;
    r.field("outcome", o.code())
        .field("unchanged", unchanged)
        .field("selection_unchanged", fixture::get_sel(k) == sel_before);
    let mut problems = Vec::new();
    if o != Outcome::Refused(Refusal::ReadOnly) {
        problems.push("outcome is not READ_ONLY");
    }
    if !unchanged {
        problems.push("read-only field changed");
    }
    if fixture::get_sel(k) != sel_before {
        problems.push("selection of the read-only field changed");
    }
    common::verdict(&r, &problems);
}

/// k06: 280 selected units are refused untouched; exactly 200 units are converted.
#[test]
fn k06_selection_limit() {
    let id = "k06.qa.selection_limit";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    let mut problems = Vec::new();

    let over = "ghbdtn ".repeat(40);
    if let Err(why) = prepare(&c, k, &over, (0, -1), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let sel_before = fixture::get_sel(k);
    let o = run_pipeline(&r, &Config::default());
    let unchanged = fixture::get_text(k) == over;
    r.field("over_units", units(&over))
        .field("over_outcome", o.code())
        .field("over_unchanged", unchanged)
        .field(
            "over_selection_unchanged",
            fixture::get_sel(k) == sel_before,
        );
    if o != Outcome::Refused(Refusal::TooLong) {
        problems.push("280 units: outcome is not TOO_LONG");
    }
    if !unchanged {
        problems.push("280 units: field changed");
    }

    let at_limit = format!("{}vbh ", "ghbdtn ".repeat(28));
    let expected = format!("{}мир ", "привет ".repeat(28));
    if let Err(why) = prepare(&c, k, &at_limit, (0, -1), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    let after = fixture::get_text(k);
    r.field("limit_units", units(&at_limit))
        .field("limit_outcome", o.code())
        .field("limit_verified", is_verified(o))
        .field("limit_expected_content", after == expected)
        .field("limit_len_after", units(&after));
    if !is_converted(o) {
        problems.push("200 units: outcome is not CONVERTED");
    }
    if after != expected && after != at_limit {
        problems.push("200 units: field is neither the original nor the expected text");
    } else if after != expected {
        problems.push("200 units: field was not converted");
    }
    common::verdict(&r, &problems);
}

/// k07: after a conversion in a classic EDIT one undo step brings the original text back.
#[test]
fn k07_one_step_undo() {
    let id = "k07.qa.one_step_undo";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    let mut problems = Vec::new();

    // Caret mode.
    if let Err(why) = prepare(&c, k, "abc ghbdtn", (10, 10), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    let converted = fixture::get_text(k) == "abc привет";
    let can_undo = fixture::send_simple(k, EM_CANUNDO) == Some(1);
    fixture::send_simple(k, EM_UNDO);
    let restored = fixture::get_text(k) == "abc ghbdtn";
    r.field("caret_outcome", o.code())
        .field("caret_converted", converted)
        .field("caret_can_undo", can_undo)
        .field("caret_undo_restores", restored)
        .field("caret_len_after_undo", fixture::text_len(k));
    if !converted {
        problems.push("caret mode: not converted");
    }
    if !restored {
        problems.push("caret mode: one undo step did not restore the original");
    }

    // Selection mode.
    let text = "abc ghbdtn xyz";
    if let Err(why) = prepare(&c, k, text, (4, 10), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let o = run_pipeline(&r, &Config::default());
    let converted = fixture::get_text(k) == "abc привет xyz";
    let can_undo = fixture::send_simple(k, EM_CANUNDO) == Some(1);
    fixture::send_simple(k, EM_UNDO);
    let restored = fixture::get_text(k) == text;
    r.field("selection_outcome", o.code())
        .field("selection_converted", converted)
        .field("selection_can_undo", can_undo)
        .field("selection_undo_restores", restored)
        .field("selection_len_after_undo", fixture::text_len(k));
    if !converted {
        problems.push("selection mode: not converted");
    }
    if !restored {
        problems.push("selection mode: one undo step did not restore the original");
    }
    common::verdict(&r, &problems);
}

/// k08: two operations in a row. A repeated press on the already converted word must not
/// garble it; a new wrong-layout word typed after it is converted on its own.
#[test]
fn k08_two_conversions_in_a_row() {
    let id = "k08.qa.two_in_a_row";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls.single;
    if let Err(why) = prepare(&c, k, WRONG, (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let mut problems = Vec::new();

    let first = run_pipeline(&r, &Config::default());
    let first_ok = fixture::get_text(k) == RIGHT;
    r.field("first_outcome", first.code())
        .field("first_converted", first_ok);
    if !first_ok {
        problems.push("first operation did not convert");
    }

    // Same word again: the text is correct now.
    let repeat = run_pipeline(&r, &Config::default());
    let after_repeat = fixture::get_text(k);
    r.field("repeat_outcome", repeat.code())
        .field("repeat_unchanged", after_repeat == RIGHT)
        .field("repeat_reverted", after_repeat == WRONG)
        .field("repeat_len", units(&after_repeat));
    if after_repeat != RIGHT && after_repeat != WRONG {
        problems.push("repeated press garbled the word");
    }
    if after_repeat != RIGHT {
        // Continue from a known state.
        fixture::set_text(k, RIGHT);
        fixture::set_sel(k, 6, 6);
    }

    // The user types the next word, again in the wrong layout.
    fixture::set_sel(k, 6, 6);
    let typed = fixture::replace_sel(k, " vbh", true);
    let second = run_pipeline(&r, &Config::default());
    let second_ok = fixture::get_text(k) == "привет мир";
    r.field("typed_next_word", typed)
        .field("second_outcome", second.code())
        .field("second_verified", is_verified(second))
        .field("second_converted", second_ok)
        .field("second_len", fixture::text_len(k));
    if !second_ok {
        problems.push("second operation did not convert the new word only");
    }
    common::verdict(&r, &problems);
}

/// Shared body of k09/k10: the focus moves to a second fixture process right before the
/// write. Nothing may be written to either window.
fn focus_moves_before_write(id: &'static str, rich: bool) {
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let other = common::child(&r, "edit");
    let c = common::child(&r, "edit");
    let (k, other_k) = if rich {
        match (c.controls.rich, other.controls.rich) {
            (Some(a), Some(b)) => (a, b),
            _ => return common::finish(&r, Status::Skip, "RICHEDIT50W is not available"),
        }
    } else {
        (c.controls.single, other.controls.single)
    };
    let other_text = "zxcv";
    if !fixture::set_text(other_k, other_text) {
        return common::finish(&r, Status::Skip, "WM_SETTEXT failed in the second fixture");
    }
    fixture::set_sel(other_k, 4, 4);
    if let Err(why) = prepare(&c, k, WRONG, (6, 6), &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let source_before = fixture::get_text(k);
    let other_before = fixture::get_text(other_k);

    let fired = Cell::new(false);
    let stolen = Cell::new(false);
    let hook = || {
        fired.set(true);
        let fg = other.bring_to_foreground();
        stolen.set(fg.ok() && other.focus(other_k));
    };
    let o = run_hooked(
        &r,
        &Config::default(),
        At::BeforeWrite,
        Some(Box::new(hook)),
    );
    // W3 is asynchronous: give stray input time to arrive before comparing.
    std::thread::sleep(Duration::from_millis(400));
    let source_unchanged = fixture::get_text(k) == source_before;
    let other_unchanged = fixture::get_text(other_k) == other_before;
    let (sel_a, sel_b) = fixture::get_sel(k);
    r.field("outcome", o.code())
        .field("hook_fired", fired.get())
        .field("focus_stolen", stolen.get())
        .field("source_unchanged", source_unchanged)
        .field("other_unchanged", other_unchanged)
        .field("source_len_after", fixture::text_len(k))
        .field("other_len_after", fixture::text_len(other_k))
        // QA-B1-04: the word must not stay selected in the abandoned field.
        .field("source_selection_units_left", sel_b.saturating_sub(sel_a));
    if !fired.get() {
        return common::finish(
            &r,
            Status::Fail,
            "the pipeline never reached the write step",
        );
    }
    if !stolen.get() {
        return common::finish(
            &r,
            Status::Skip,
            "the second fixture could not take the foreground",
        );
    }
    let mut problems = Vec::new();
    if !other_unchanged {
        problems.push("text was written to the window that took the focus");
    }
    if !source_unchanged {
        problems.push("the original field changed after the focus moved away");
    }
    if o != Outcome::Failed(Failure::TargetChanged) {
        problems.push("outcome is not TARGET_CHANGED");
    }
    if sel_a != sel_b {
        problems.push("the word stayed selected in the original field (QA-B1-04)");
    }
    common::verdict(&r, &problems);
}

/// k09: focus moves away right before a W1 write.
#[test]
fn k09_focus_moves_before_write_w1() {
    focus_moves_before_write("k09.qa.focus_moved_before_write_w1", false);
}

/// k10: focus moves away right before a W3 write.
#[test]
fn k10_focus_moves_before_write_w3() {
    focus_moves_before_write("k10.qa.focus_moved_before_write_w3", true);
}

/// Runs one operation whose target process exits at `at`; returns the outcome.
fn exit_at(r: &mut Report, env: &common::Env, at: At) -> Result<Outcome, &'static str> {
    let c = common::child(r, "edit");
    let k = c.controls.single;
    prepare(&c, k, WRONG, (6, 6), r, env)?;
    // Dropping the handle closes the child's stdin and waits for the process to end.
    let hook = move || drop(c);
    Ok(run_hooked(r, &Config::default(), at, Some(Box::new(hook))))
}

/// k11: the target process exits in the middle of the operation: a clean stop, no panic,
/// nothing reported as converted.
#[test]
fn k11_target_process_exits_mid_operation() {
    let id = "k11.qa.target_exit_mid_operation";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let mut problems = Vec::new();
    for (at, key) in [
        (At::BeforeRead, "exit_before_read_outcome"),
        (At::BeforeWrite, "exit_before_write_outcome"),
    ] {
        let o = match exit_at(&mut r, &env, at) {
            Ok(o) => o,
            Err(why) => return common::finish(&r, Status::Skip, why),
        };
        r.field(key, o.code());
        if is_converted(o) {
            problems.push("operation reported CONVERTED although the target had exited");
        }
        if !matches!(o, Outcome::Failed(_) | Outcome::Refused(_)) {
            problems.push("operation on an exited target is not a failure or refusal");
        }
    }
    common::verdict(&r, &problems);
}

/// k12: the excluded list matches the executable name regardless of the case of the file
/// name on disk. The fixture is copied under a mixed-case name for this.
#[test]
fn k12_excluded_app_case_insensitive() {
    let id = "k12.qa.excluded_case_insensitive";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let dir = std::env::temp_dir().join(format!("ks-qa-k12-{}", std::process::id()));
    let exe = dir.join("KS-QA-MixedCase.EXE");
    let copied = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::copy(common::FIXTURE_EXE, &exe))
        .is_ok();
    r.field("fixture_copied", copied);
    if !copied {
        return common::finish(&r, Status::Skip, "could not copy the fixture executable");
    }
    let mut problems = Vec::new();
    {
        let c = match ChildFixture::spawn(&exe, "edit") {
            Ok(c) => c,
            Err(_) => {
                let _ = std::fs::remove_dir_all(&dir);
                return common::finish(&r, Status::Skip, "could not start the copied fixture");
            }
        };
        let k = c.controls.single;
        if let Err(why) = prepare(&c, k, WRONG, (6, 6), &mut r, &env) {
            drop(c);
            let _ = std::fs::remove_dir_all(&dir);
            return common::finish(&r, Status::Skip, why);
        }
        // The way the settings store an entry typed by the user in yet another case.
        let entry = ks_engine::normalize_exe("ks-qa-MIXEDcase.Exe");
        r.field("entry_normalised", entry.is_some());
        let config = Config {
            excluded: entry.into_iter().collect(),
            ..Config::default()
        };
        let o = run_pipeline(&r, &config);
        let unchanged = fixture::get_text(k) == WRONG;
        r.field("excluded_outcome", o.code())
            .field("excluded_unchanged", unchanged);
        if o != Outcome::Refused(Refusal::ExcludedApp) {
            problems.push("mixed-case executable was not matched by the excluded list");
        }
        if !unchanged {
            problems.push("excluded app content changed");
        }
        // Control: without the entry the same target is converted.
        fixture::set_text(k, WRONG);
        fixture::set_sel(k, 6, 6);
        let o = run_pipeline(&r, &Config::default());
        r.field("control_outcome", o.code())
            .field("control_converted", fixture::get_text(k) == RIGHT);
        if !is_converted(o) {
            problems.push("control run without the entry did not convert");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    common::verdict(&r, &problems);
}
