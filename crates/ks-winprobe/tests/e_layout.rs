//! S-2a scenario e: keyboard layout read and switch via `WM_INPUTLANGCHANGEREQUEST`.
#![cfg(windows)]

mod common;

use std::time::Duration;

use ks_winprobe::fixture;
use ks_winprobe::layout::{self, Hkl};
use ks_winprobe::report::Status;

const EN_US: u16 = 0x0409;
const RU_RU: u16 = 0x0419;

fn ids(v: &[Hkl]) -> String {
    v.iter()
        .map(|h| format!("{:04x}", h.lang_id()))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn e1_layout_read_and_switch() {
    let id = "e1.layout.read_switch";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let before = layout::loaded_layouts();
    let initial = layout::thread_layout(fx.thread_id);
    r.field("loaded_before", ids(&before)).field(
        "fixture_thread_layout",
        format!("{:04x}", initial.lang_id()),
    );

    let en = layout::load("00000409");
    let ru = layout::load("00000419");
    r.field("load_0409", en.is_some())
        .field("load_0419", ru.is_some());
    let cleanup = |r: &mut ks_winprobe::report::Report| {
        // Unload only what this test added to the session's list.
        for h in [en, ru].into_iter().flatten() {
            if !before.contains(&h) {
                r.field("unloaded_added_layout", layout::unload(h));
            }
        }
    };
    let (Some(en), Some(ru)) = (en, ru) else {
        cleanup(&mut r);
        return common::finish(
            &r,
            Status::Skip,
            "LoadKeyboardLayoutW failed for 0409 or 0419",
        );
    };
    // Post to the focused control as KeySwitch would to the target's focus window.
    let fg_ok = env.desktop.unusable_reason().is_none() && common::foreground(&fx, &mut r).ok();
    let target = if fg_ok && fx.focus(fx.controls.single) {
        fx.controls.single
    } else {
        fx.controls.main
    };
    let switch = |to: Hkl, want: u16| -> Option<Duration> {
        if !layout::request_switch(target, to) {
            return None;
        }
        fixture::wait_until(Duration::from_secs(3), || {
            layout::thread_layout(fx.thread_id).lang_id() == want
        })
    };
    let first = if initial.lang_id() == RU_RU {
        (en, EN_US)
    } else {
        (ru, RU_RU)
    };
    let t1 = switch(first.0, first.1);
    let t2 = switch(initial, initial.lang_id());
    r.field("switch_to", format!("{:04x}", first.1))
        .field("switched_ms", t1.map_or(-1, |d| d.as_millis() as i64))
        .field("switched_back_ms", t2.map_or(-1, |d| d.as_millis() as i64))
        .field("loaded_after", ids(&layout::loaded_layouts()));
    cleanup(&mut r);
    match (t1, t2) {
        (Some(_), Some(_)) => common::finish(&r, Status::Pass, ""),
        _ if !fg_ok => common::finish(
            &r,
            Status::Skip,
            "switch not observed and fixture was not foreground (cannot tell API from environment)",
        ),
        _ => common::finish(
            &r,
            Status::Fail,
            "WM_INPUTLANGCHANGEREQUEST did not change the thread layout",
        ),
    }
}
