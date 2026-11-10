//! S-2a scenario c: write method W3 (`SendInput` + `KEYEVENTF_UNICODE`).
#![cfg(windows)]

mod common;

use std::cell::Cell;
use std::time::{Duration, Instant};

use ks_winprobe::fixture::{self, Fixture, Hwnd};
use ks_winprobe::input::{self, Refusal, SendError};
use ks_winprobe::report::{Report, Status};
use windows::Win32::UI::Controls::EM_UNDO;

/// Prepares a focused, empty target; `Err(reason)` = SKIP.
fn target(fx: &Fixture, h: Hwnd, r: &mut Report, env: &common::Env) -> Result<(), &'static str> {
    if let Some(why) = env.desktop.unusable_reason() {
        return Err(why);
    }
    if !common::foreground(fx, r).ok() {
        return Err(
            "fixture could not become the foreground window; injected input would go elsewhere",
        );
    }
    fixture::set_text(h, "");
    if !fx.focus(h) {
        return Err("SetFocus to the target edit failed");
    }
    Ok(())
}

fn send_error_label(e: SendError) -> (Status, String) {
    match e {
        SendError::Blocked { injected: 0, .. } => (
            Status::Skip,
            "SendInput injected 0 events (no input desktop or UIPI)".into(),
        ),
        SendError::Blocked {
            injected,
            requested,
        } => (
            Status::Fail,
            format!("SendInput short write: {injected}/{requested} events"),
        ),
        SendError::TargetLost { batches_sent } => (
            Status::Skip,
            format!("fixture lost the foreground after {batches_sent} batches"),
        ),
        SendError::Refused(_) => (Status::Fail, "valid text was refused".into()),
    }
}

/// c1: short ASCII + Cyrillic text (and a surrogate pair) arrives intact.
#[test]
fn c1_unicode_short_text() {
    let id = "c1.w3.sendinput_unicode_short";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let h = fx.controls.single;
    if let Err(why) = target(&fx, h, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let mut problems: Vec<&'static str> = Vec::new();
    for (label, text) in [
        ("ascii_cyrillic", "Hello Привет"),
        ("surrogate_pair", "a\u{1F600}b"),
    ] {
        fixture::set_text(h, "");
        match input::send_unicode(text, || fx.is_foreground()) {
            Ok(sent) => {
                let arrived =
                    fixture::wait_until(Duration::from_secs(3), || fixture::get_text(h) == text);
                r.field(
                    label,
                    format!(
                        "units={} events={} batches={} arrived_ms={}",
                        sent.units,
                        sent.events,
                        sent.batches,
                        arrived.map_or(-1, |d| d.as_millis() as i64)
                    ),
                );
                if arrived.is_none() {
                    problems.push("injected text did not arrive intact within 3 s");
                }
            }
            Err(e) => {
                let (status, why) = send_error_label(e);
                return common::finish(&r, status, &why);
            }
        }
    }
    common::verdict(&r, &problems);
}

/// c2: throughput for 200 characters (ASCII + Cyrillic) and Undo granularity (recorded).
#[test]
fn c2_throughput_200_chars() {
    let id = "c2.w3.sendinput_throughput_200";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let h = fx.controls.multi;
    if let Err(why) = target(&fx, h, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let text: String = "Ghbdtn Привет ".chars().cycle().take(200).collect();
    let checks = Cell::new(0u32);
    let t0 = Instant::now();
    let sent = match input::send_unicode(&text, || {
        checks.set(checks.get() + 1);
        fx.is_foreground()
    }) {
        Ok(s) => s,
        Err(e) => {
            let (status, why) = send_error_label(e);
            return common::finish(&r, status, &why);
        }
    };
    let send_time = t0.elapsed();
    let arrived = fixture::wait_until(Duration::from_secs(20), || fixture::text_len(h) >= 200);
    let total = t0.elapsed();
    let intact = fixture::get_text(h) == text;
    let cps = if arrived.is_some() {
        (200.0 / total.as_secs_f64()) as u64
    } else {
        0
    };
    fixture::send_simple(h, EM_UNDO);
    r.field("chars", 200)
        .field("batches", sent.batches)
        .field("events", sent.events)
        .field("pre_batch_checks", checks.get())
        .field("send_call_ms", send_time.as_millis())
        .field("arrival_ms", total.as_millis())
        .field("chars_per_sec", cps)
        .field("intact", intact)
        .field("len_after_one_em_undo", fixture::text_len(h));
    if arrived.is_none() || !intact {
        return common::finish(
            &r,
            Status::Fail,
            "200 injected chars did not arrive intact in 20 s",
        );
    }
    common::finish(&r, Status::Pass, "");
}

/// c3: the writer refuses "\n" / "\t" / "\r\n" before sending anything; no Enter/Tab events.
#[test]
fn c3_refuses_control_chars() {
    let id = "c3.w3.refuses_control_chars";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let mut problems: Vec<&'static str> = Vec::new();
    // If we can own the foreground, also prove the focused multi-line edit stays unchanged.
    let h = fx.controls.multi;
    let live = target(&fx, h, &mut r, &env).is_ok();
    if live {
        fixture::set_text(h, "base");
    }
    for (label, text) in [
        ("lf", "ab\ncd"),
        ("tab", "ab\tcd"),
        ("crlf", "ab\r\ncd"),
        ("cr", "ab\rcd"),
    ] {
        let checks = Cell::new(0u32);
        let res = input::send_unicode(text, || {
            checks.set(checks.get() + 1);
            true
        });
        let refused = res == Err(SendError::Refused(Refusal::ControlChar));
        r.field(
            label,
            format!("refused={refused} pre_batch_checks={}", checks.get()),
        );
        if !refused || checks.get() != 0 {
            problems.push("control-character text was not refused up front");
        }
    }
    // Valid text never maps to VK_RETURN or a CR/LF unit.
    let ok_batches = input::build_batches("Ghbdtn Привет \u{1F600}").unwrap_or_default();
    let no_return = !ok_batches.is_empty() && !input::contains_return(&ok_batches);
    r.field("valid_text_contains_return", !no_return);
    if !no_return {
        problems.push("valid text produced a Return event");
    }
    if live {
        std::thread::sleep(Duration::from_millis(300));
        let unchanged = fixture::get_text(h) == "base";
        r.field("focused_field_unchanged", unchanged);
        if !unchanged {
            problems.push("focused field changed although every send was refused");
        }
    } else {
        r.field("focused_field_unchanged", "not_evaluated_no_foreground");
    }
    common::verdict(&r, &problems);
}
