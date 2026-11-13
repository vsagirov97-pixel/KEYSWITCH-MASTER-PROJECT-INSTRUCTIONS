//! Beta 1 scenarios j13–j15: the language-choice popup (`ks_platform_win::popup`) inside the
//! product pipeline, against the fixture process. Keys are injected with the probe-only
//! `SendInput` helper; the popup receives them through its temporary `RegisterHotKey`
//! registrations. Synthetic strings only; reports carry codes and booleans.
#![cfg(windows)]

mod common;

use std::time::Duration;

use ks_engine::{Config, Outcome};
use ks_platform_win::adapter::WinPlatform;
use ks_platform_win::popup;
use ks_winprobe::child::ChildFixture;
use ks_winprobe::fixture::{self, Hwnd};
use ks_winprobe::gate::Gate;
use ks_winprobe::input;
use ks_winprobe::report::{Report, Status};
use ks_winprobe::uia::{ComGuard, Uia};
use windows::Win32::UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_ESCAPE};

const MIXED: &str = "привет ghbdtn";
const MIXED_UNITS: i32 = 13;
const MIXED_AS_RU: &str = "привет привет";

/// What the helper thread saw while the popup was open.
#[derive(Clone, Copy, Default)]
struct Seen {
    popup: bool,
    foreground_kept: bool,
    popup_not_foreground: bool,
}

/// Runs one pipeline operation; once the popup is open, a helper thread records the
/// foreground state and runs `act`.
fn run_with_popup(
    r: &Report,
    c: &ChildFixture,
    gate: &Gate,
    timeout: Option<Duration>,
    act: impl FnOnce() + Send,
) -> (Outcome, Seen) {
    let fail = |e: &str| -> ! {
        common::finish(r, Status::Fail, e);
        unreachable!("finish panics on FAIL")
    };
    let com = ComGuard::mta().unwrap_or_else(|e| fail(&e));
    let uia = Uia::new().unwrap_or_else(|e| fail(&e));
    let converter = ks_core::builtin().expect("built-in layouts parse");
    let main = c.controls.main;
    let (outcome, seen) = std::thread::scope(|s| {
        let helper = s.spawn(move || {
            let mut seen = Seen::default();
            let opened = fixture::wait_until(Duration::from_secs(3), || popup::current().is_some());
            if let (Some(_), Some(p)) = (opened, popup::current()) {
                seen.popup = true;
                seen.foreground_kept = fixture::is_foreground(main);
                seen.popup_not_foreground = !fixture::is_foreground(p);
                act();
            }
            seen
        });
        let outcome = {
            let mut platform = WinPlatform::new(&uia, gate);
            if let Some(t) = timeout {
                platform.set_popup_timeout(t);
            }
            ks_engine::run(&mut platform, &converter, &Config::default())
        };
        (outcome, helper.join().unwrap_or_default())
    });
    drop(uia);
    drop(com);
    (outcome, seen)
}

fn prepare(c: &ChildFixture, r: &mut Report, env: &common::Env) -> Result<Hwnd, &'static str> {
    let k = c.controls.single;
    if !fixture::set_text(k, MIXED) {
        return Err("WM_SETTEXT failed");
    }
    common::child_target(c, k, r, env)?;
    fixture::set_sel(k, 0, MIXED_UNITS);
    Ok(k)
}

fn record(r: &mut Report, label: &'static str, o: Outcome, seen: Seen) {
    r.field(
        label,
        format!(
            "outcome={} popup_seen={} target_kept_foreground={} popup_not_foreground={} popup_closed={}",
            o.code(),
            seen.popup,
            seen.foreground_kept,
            seen.popup_not_foreground,
            popup::current().is_none()
        ),
    );
}

/// The digit key that picks Russian in the popup (`1` = first offered language).
fn key_for_ru() -> Option<VIRTUAL_KEY> {
    let c = ks_core::builtin().expect("built-in layouts parse");
    let i = c.languages().position(|l| l == "ru")?;
    Some(VIRTUAL_KEY(u16::from(b'1') + u16::try_from(i).ok()?))
}

/// j13: mixed text opens the popup without taking the foreground; the digit key converts
/// the selection to the picked language.
#[test]
fn j13_popup_digit_converts_mixed_selection() {
    let id = "j13.pipeline.popup_pick";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = match prepare(&c, &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let Some(key) = key_for_ru() else {
        return common::finish(&r, Status::Fail, "no Russian among the built-in languages");
    };
    let gate = Gate::new();
    let (o, seen) = run_with_popup(&r, &c, &gate, None, move || {
        input::send_vk_chord(&[key]);
    });
    record(&mut r, "pick", o, seen);
    let mut problems = Vec::new();
    if !seen.popup {
        problems.push("the popup did not open for mixed text");
    }
    if seen.popup && !(seen.foreground_kept && seen.popup_not_foreground) {
        problems.push("the popup took the foreground from the target");
    }
    if !matches!(o, Outcome::Converted { .. }) {
        problems.push("outcome is not CONVERTED");
    }
    if fixture::get_text(k) != MIXED_AS_RU {
        problems.push("selection was not converted to the picked language");
    }
    if popup::current().is_some() {
        problems.push("the popup is still open after the operation");
    }
    common::verdict(&r, &problems);
}

/// j14: Esc and the timeout both cancel — nothing is written and the popup is gone.
#[test]
fn j14_popup_esc_and_timeout_cancel() {
    let id = "j14.pipeline.popup_cancel";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let mut problems = Vec::new();
    let gate = Gate::new();

    let k = match prepare(&c, &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let (o, seen) = run_with_popup(&r, &c, &gate, None, || {
        input::send_vk_chord(&[VK_ESCAPE]);
    });
    record(&mut r, "esc", o, seen);
    if !seen.popup || o != Outcome::Cancelled {
        problems.push("Esc did not cancel the operation");
    }
    if fixture::get_text(k) != MIXED {
        problems.push("Esc: field changed");
    }

    let k = match prepare(&c, &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let short = Duration::from_millis(400);
    let started = std::time::Instant::now();
    let (o, seen) = run_with_popup(&r, &c, &gate, Some(short), || {});
    r.field("timeout_run_ms", started.elapsed().as_millis());
    record(&mut r, "timeout", o, seen);
    if !seen.popup || o != Outcome::Cancelled {
        problems.push("the timeout did not cancel the operation");
    }
    if fixture::get_text(k) != MIXED {
        problems.push("timeout: field changed");
    }
    if popup::current().is_some() {
        problems.push("the popup is still open after the operation");
    }
    common::verdict(&r, &problems);
}

/// j15: a newer operation generation while the popup is open closes it; a digit pressed
/// afterwards writes nothing (generation gate before the write).
#[test]
fn j15_popup_stale_generation_writes_nothing() {
    let id = "j15.pipeline.popup_generation";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = match prepare(&c, &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let Some(key) = key_for_ru() else {
        return common::finish(&r, Status::Fail, "no Russian among the built-in languages");
    };
    let gate = Gate::new();
    let (o, seen) = run_with_popup(&r, &c, &gate, None, || {
        gate.bump();
        // The digit arrives together with the invalidation: whichever the popup sees
        // first, the write must not happen.
        input::send_vk_chord(&[key]);
    });
    record(&mut r, "stale", o, seen);
    // Let a wrongly performed write land before judging the field.
    std::thread::sleep(Duration::from_millis(300));
    let mut problems = Vec::new();
    if !seen.popup {
        problems.push("the popup did not open for mixed text");
    }
    if matches!(o, Outcome::Converted { .. }) {
        problems.push("a stale operation reported CONVERTED");
    }
    if fixture::get_text(k) != MIXED {
        problems.push("a stale operation changed the field");
    }
    if popup::current().is_some() {
        problems.push("the popup is still open after the operation");
    }
    common::verdict(&r, &problems);
}
