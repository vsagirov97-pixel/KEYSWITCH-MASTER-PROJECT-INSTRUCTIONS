//! S-2b scenario i: target fingerprint + generation gate + modifier check before every W3
//! batch (WS-05, C-6′), refusals (SelfTarget) and WM_HOTKEY injected from another process at
//! the same and at Low integrity level.
#![cfg(windows)]

mod common;

use std::cell::Cell;
use std::process::Command;
use std::time::Duration;

use ks_winprobe::child::ChildFixture;
use ks_winprobe::fixture;
use ks_winprobe::gate::{self, Abort, Fingerprint, Gate, TargetRefusal};
use ks_winprobe::input::{self, BATCH_UNITS, SendError};
use ks_winprobe::report::{Report, Status};
use ks_winprobe::{hotkey, integrity};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_SHIFT, VK_CONTROL, VK_F14,
};

const TEXT_200: &str = "Ghbdtn Привет ";

fn text200() -> String {
    TEXT_200.chars().cycle().take(200).collect()
}

fn capture_or_skip(r: &Report) -> Option<Fingerprint> {
    match gate::capture() {
        Ok(fp) => Some(fp),
        Err(_) => {
            common::finish(r, Status::Skip, "fingerprint capture failed");
            None
        }
    }
}

/// Gated send: `hook(batch_index)` runs before the check (to simulate events).
fn gated_send(
    text: &str,
    fp: &Fingerprint,
    g: &Gate,
    mut hook: impl FnMut(usize),
    pause: Duration,
) -> (Result<input::Sent, SendError>, Option<Abort>) {
    let tok = g.begin();
    let n = Cell::new(0usize);
    let mut abort = None;
    let res = input::send_unicode(text, || {
        let i = n.get();
        n.set(i + 1);
        if !pause.is_zero() && i > 0 {
            std::thread::sleep(pause);
        }
        hook(i);
        match gate::pre_batch(fp, g, tok) {
            Ok(()) => true,
            Err(a) => {
                abort = Some(a);
                false
            }
        }
    });
    (res, abort)
}

fn sent_batches(res: &Result<input::Sent, SendError>) -> i64 {
    match res {
        Ok(s) => s.batches as i64,
        Err(SendError::TargetLost { batches_sent }) => *batches_sent as i64,
        Err(_) => -1,
    }
}

/// i1: fingerprint = (foreground, focus, pid, start time) of the child; stable; focus moved to
/// a sibling control (P-10) or another process → TargetChanged.
#[test]
fn i1_fingerprint_and_target_changed() {
    let id = "i1.gate.fingerprint_p10";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let a = common::child(&r, "edit");
    let b = common::child(&r, "edit");
    if let Err(why) = common::child_target(&a, a.controls.single, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let Some(fp) = capture_or_skip(&r) else {
        return;
    };
    let mut problems: Vec<&'static str> = Vec::new();
    let st = gate::process_start_time(a.pid);
    let again = gate::capture().ok();
    let g = Gate::new();
    let tok = g.begin();
    let ok_now = gate::pre_batch(&fp, &g, tok).is_ok();
    r.field("fg_is_child_main", fp.foreground == a.controls.main)
        .field("focus_is_child_single", fp.focus == a.controls.single)
        .field("pid_is_child", fp.pid == a.pid)
        .field("start_time_matches", Some(fp.start_time) == st)
        .field("stable", again == Some(fp))
        .field("pre_batch_ok_unchanged", ok_now);
    if !(fp.foreground == a.controls.main
        && fp.focus == a.controls.single
        && fp.pid == a.pid
        && Some(fp.start_time) == st
        && again == Some(fp)
        && ok_now)
    {
        problems.push("fingerprint does not describe the focused child control");
    }
    // P-10: focus moves to a sibling in the same window.
    let _ = a.focus(a.controls.multi);
    std::thread::sleep(Duration::from_millis(50));
    let sibling = gate::pre_batch(&fp, &g, tok);
    r.field("sibling_focus", sibling.err().map_or("none", |x| x.label()));
    if sibling != Err(Abort::TargetChanged) {
        problems.push("focus move to a sibling control not detected");
    }
    // Another process.
    let _ = common::child_target(&b, b.controls.single, &mut r, &env);
    let other = gate::capture().ok();
    let other_check = gate::pre_batch(&fp, &g, tok);
    r.field(
        "other_process",
        format!(
            "pid_differs={} start_differs={} check={}",
            other.is_some_and(|o| o.pid != fp.pid),
            other.is_some_and(|o| o.start_time != fp.start_time),
            other_check.err().map_or("none", |x| x.label())
        ),
    );
    if other_check != Err(Abort::TargetChanged) {
        problems.push("another process as foreground not detected");
    }
    common::verdict(&r, &problems);
}

/// i2 WS-05: a held Ctrl (injected key-down) refuses W3 before the first batch; the field
/// stays unchanged.
#[test]
fn i2_modifier_held_refuses() {
    let id = "i2.gate.modifier_held";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let a = common::child(&r, "edit");
    let h = a.controls.single;
    fixture::set_text(h, "");
    if let Err(why) = common::child_target(&a, h, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let Some(fp) = capture_or_skip(&r) else {
        return;
    };
    let before = gate::modifiers_down();
    let down = input::send_vk_state(VK_CONTROL, true);
    std::thread::sleep(Duration::from_millis(30));
    let held = gate::modifiers_down();
    let g = Gate::new();
    let (res, abort) = gated_send("Ghbdtn", &fp, &g, |_| {}, Duration::ZERO);
    let up = input::send_vk_state(VK_CONTROL, false);
    std::thread::sleep(Duration::from_millis(30));
    let after = gate::modifiers_down();
    std::thread::sleep(Duration::from_millis(200));
    let len = fixture::text_len(h);
    r.field("modifiers_before", before)
        .field("ctrl_down_injected", down)
        .field("modifiers_seen_held", held)
        .field("abort", abort.map_or("none", |x| x.label()))
        .field("batches_sent", sent_batches(&res))
        .field("ctrl_up_injected", up)
        .field("modifiers_after_release", after)
        .field("field_len_after", len);
    if down == 0 {
        return common::finish(&r, Status::Skip, "SendInput injected 0 events");
    }
    let mut problems: Vec<&'static str> = Vec::new();
    if !held || abort != Some(Abort::Modifiers) || sent_batches(&res) != 0 {
        problems.push("held Ctrl did not refuse W3 before the first batch");
    }
    if len != 0 {
        problems.push("field changed although W3 was refused");
    }
    if after {
        problems.push("modifier still reported down after release");
    }
    common::verdict(&r, &problems);
}

/// i3: a newer operation (generation bump, e.g. second hotkey press) aborts the running one
/// at the next batch.
#[test]
fn i3_generation_bump_aborts() {
    let id = "i3.gate.generation_bump";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let a = common::child(&r, "edit");
    let h = a.controls.multi;
    fixture::set_text(h, "");
    if let Err(why) = common::child_target(&a, h, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let Some(fp) = capture_or_skip(&r) else {
        return;
    };
    let g = Gate::new();
    let (res, abort) = gated_send(
        &text200(),
        &fp,
        &g,
        |i| {
            if i == 2 {
                g.bump();
            }
        },
        Duration::ZERO,
    );
    let _ = fixture::wait_until(Duration::from_secs(2), || {
        fixture::text_len(h) >= 2 * BATCH_UNITS
    });
    std::thread::sleep(Duration::from_millis(200));
    let len = fixture::text_len(h);
    r.field("abort", abort.map_or("none", |x| x.label()))
        .field("batches_sent", sent_batches(&res))
        .field("units_in_target", len);
    let problems: Vec<&'static str> =
        if abort == Some(Abort::Generation) && sent_batches(&res) == 2 && len == 2 * BATCH_UNITS {
            Vec::new()
        } else {
            vec!["generation bump did not stop the operation at the next batch"]
        };
    common::verdict(&r, &problems);
}

fn steal(thief: &ChildFixture) -> bool {
    thief.bring_to_foreground().ok() && thief.focus(thief.controls.single)
}

/// i4 WS-05: foreground stolen between batches (deterministic: right before batch 2's check)
/// → abort, 0 units in the thief, exactly 2 batches in the target. i5: asynchronous thief
/// while batches are spaced 15 ms → at most one batch lands in the wrong window.
#[test]
fn i4_foreground_stolen_between_batches() {
    let id = "i4.gate.foreground_stolen";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let a = common::child(&r, "edit");
    let b = common::child(&r, "edit");
    let mut problems: Vec<&'static str> = Vec::new();

    // Deterministic.
    fixture::set_text(a.controls.multi, "");
    fixture::set_text(b.controls.single, "");
    if let Err(why) = common::child_target(&a, a.controls.multi, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let Some(fp) = capture_or_skip(&r) else {
        return;
    };
    let g = Gate::new();
    let stolen = Cell::new(false);
    let (res, abort) = gated_send(
        &text200(),
        &fp,
        &g,
        |i| {
            if i == 2 {
                stolen.set(steal(&b));
            }
        },
        Duration::ZERO,
    );
    std::thread::sleep(Duration::from_millis(300));
    let in_target = fixture::text_len(a.controls.multi);
    let in_thief = fixture::text_len(b.controls.single);
    r.field(
        "deterministic",
        format!(
            "stolen={} abort={} batches_sent={} units_target={in_target} units_thief={in_thief}",
            stolen.get(),
            abort.map_or("none", |x| x.label()),
            sent_batches(&res)
        ),
    );
    if !stolen.get() {
        return common::finish(&r, Status::Skip, "thief could not take the foreground");
    }
    if abort != Some(Abort::TargetChanged) || in_thief != 0 || in_target != 2 * BATCH_UNITS {
        problems.push("deterministic steal: wrong abort or input reached the thief");
    }

    // Asynchronous thief.
    fixture::set_text(a.controls.multi, "");
    fixture::set_text(b.controls.single, "");
    if common::child_target(&a, a.controls.multi, &mut r, &env).is_err() {
        return common::finish(&r, Status::Skip, "target could not retake the foreground");
    }
    let Some(fp) = capture_or_skip(&r) else {
        return;
    };
    let bm = b.controls.main;
    let bs = b.controls.single;
    let thief = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(40));
        fixture::request_foreground(bm).ok() && fixture::request_focus(bm, bs)
    });
    let (res, abort) = gated_send(&text200(), &fp, &g, |_| {}, Duration::from_millis(15));
    let async_stolen = thief.join().unwrap_or(false);
    std::thread::sleep(Duration::from_millis(300));
    let in_target = fixture::text_len(a.controls.multi);
    let in_thief = fixture::text_len(b.controls.single);
    r.field(
        "async",
        format!(
            "stolen={async_stolen} abort={} batches_sent={} units_target={in_target} units_thief={in_thief}",
            abort.map_or("none", |x| x.label()),
            sent_batches(&res)
        ),
    );
    if async_stolen {
        if abort != Some(Abort::TargetChanged) {
            problems.push("async steal: operation not aborted");
        }
        if in_thief > BATCH_UNITS {
            problems.push("async steal: more than one batch reached the wrong window");
        }
    } else {
        r.field(
            "async_note",
            "thief did not get the foreground; async part not evaluated",
        );
    }
    common::verdict(&r, &problems);
}

/// i6: KeySwitch's own window as target → SelfTarget before any write.
#[test]
fn i6_self_target_refused() {
    let id = "i6.gate.self_target";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    if env.desktop.unusable_reason().is_some() || !common::foreground(&fx, &mut r).ok() {
        return common::finish(&r, Status::Skip, "in-process fixture not foreground");
    }
    if !fx.focus(fx.controls.single) {
        return common::finish(&r, Status::Skip, "SetFocus failed");
    }
    let Some(fp) = capture_or_skip(&r) else {
        return;
    };
    let cls = gate::classify(&fp);
    r.field("pid_is_self", fp.pid == std::process::id()).field(
        "classify",
        match cls {
            Ok(()) => "allowed",
            Err(TargetRefusal::SelfTarget) => "refused_self_target",
        },
    );
    let problems: Vec<&'static str> = if cls == Err(TargetRefusal::SelfTarget) {
        Vec::new()
    } else {
        vec!["own window not refused as target"]
    };
    common::verdict(&r, &problems);
}

/// i7 (S-2a f2 follow-up): does an injected hotkey chord from **another process** produce
/// WM_HOTKEY — at the same integrity level and at Low IL? Fact only.
#[test]
fn i7_hotkey_injected_from_other_process_and_low_il() {
    let id = "i7.hotkey.injected_other_process_low_il";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    if let Some(why) = env.desktop.unusable_reason() {
        return common::finish(&r, Status::Skip, why);
    }
    let fx = common::fixture(&r);
    let fg = common::foreground(&fx, &mut r).ok();
    let hk = match hotkey::spawn(7, MOD_CONTROL | MOD_ALT | MOD_SHIFT, VK_F14) {
        Ok(h) => h,
        Err(code) => {
            r.field("register_error", code);
            return common::finish(
                &r,
                Status::Skip,
                "RegisterHotKey(Ctrl+Alt+Shift+F14) failed",
            );
        }
    };
    r.field("own_rid", integrity::current_rid().map_or(-1, i64::from))
        .field("fixture_foreground", fg);
    // Same IL, another process.
    let same = Command::new(common::FIXTURE_EXE)
        .arg("inject-hotkey")
        .status()
        .ok()
        .and_then(|s| s.code());
    let same_got = hk.events.recv_timeout(Duration::from_secs(2)).is_ok();
    r.field("same_il_injected", same.unwrap_or(-1))
        .field("same_il_wm_hotkey", same_got);
    while hk.events.try_recv().is_ok() {}
    // Low IL.
    let quoted = format!("\"{}\"", common::FIXTURE_EXE);
    let low_rid = integrity::run_low_il(&format!("{quoted} report-il"), 10_000);
    let low_rid = match low_rid {
        Ok(c) => c,
        Err(e) => {
            r.field("low_il_error", e);
            return common::finish(
                &r,
                Status::Skip,
                "could not start a Low-IL helper on this runner",
            );
        }
    };
    r.field("low_helper_rid", low_rid);
    let low = integrity::run_low_il(&format!("{quoted} inject-hotkey"), 10_000);
    let low_got = hk.events.recv_timeout(Duration::from_secs(2)).is_ok();
    r.field(
        "low_il_injected",
        low.as_ref().map_or(-1, |&c| i64::from(c as i32)),
    )
    .field("low_il_wm_hotkey", low_got);
    if low_rid != integrity::LOW_RID {
        return common::finish(&r, Status::Skip, "helper did not run at Low IL");
    }
    common::finish(
        &r,
        Status::Observed,
        "delivery facts for injected input from other processes",
    );
}
