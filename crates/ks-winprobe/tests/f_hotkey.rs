//! S-2a scenario f: `RegisterHotKey` on a dedicated message-loop thread (no hooks, D-14).
#![cfg(windows)]

mod common;

use std::time::Duration;

use ks_winprobe::hotkey;
use ks_winprobe::input;
use ks_winprobe::report::Status;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_SHIFT, VK_CONTROL, VK_F13, VK_MENU, VK_SHIFT,
};

const ERROR_HOTKEY_ALREADY_REGISTERED: u32 = 1409;

#[test]
fn f1_register_hotkey_and_conflict() {
    let id = "f1.hotkey.register_conflict";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let mods = MOD_CONTROL | MOD_ALT | MOD_SHIFT;
    let first = match hotkey::spawn(1, mods, VK_F13) {
        Ok(h) => h,
        Err(code) => {
            r.field("first_error", code);
            let status = if env.desktop.unusable_reason().is_some() {
                Status::Skip
            } else {
                Status::Fail
            };
            return common::finish(&r, status, "RegisterHotKey(Ctrl+Alt+Shift+F13) failed");
        }
    };
    r.field("first_registered", true);
    let mut problems: Vec<&'static str> = Vec::new();
    match hotkey::spawn(2, mods, VK_F13) {
        Ok(_) => {
            r.field("duplicate", "registered");
            problems.push("duplicate registration was not reported as a conflict");
        }
        Err(code) => {
            r.field("duplicate_error", code);
            if code != ERROR_HOTKEY_ALREADY_REGISTERED {
                problems.push("duplicate failed with an unexpected error code");
            }
        }
    }
    drop(first);
    let again = hotkey::spawn(3, mods, VK_F13);
    r.field("re_register_after_unregister", again.is_ok());
    if again.is_err() {
        problems.push("hotkey not released after the owning thread unregistered it");
    }
    common::verdict(&r, &problems);

    // f2: delivery of WM_HOTKEY for an injected chord (fact only; needs an input desktop).
    let id2 = "f2.hotkey.delivery_injected";
    let mut r2 = common::report(id2, &env);
    let Ok(hk) = again else {
        return common::finish(&r2, Status::Skip, "no registered hotkey");
    };
    if let Some(why) = env.desktop.unusable_reason() {
        return common::finish(&r2, Status::Skip, why);
    }
    let injected = input::send_vk_chord(&[VK_CONTROL, VK_MENU, VK_SHIFT, VK_F13]);
    r2.field("injected_events", injected);
    if injected == 0 {
        return common::finish(&r2, Status::Skip, "SendInput injected 0 events");
    }
    let got = hk.events.recv_timeout(Duration::from_secs(2));
    r2.field("wm_hotkey_received", got.is_ok())
        .field("wm_hotkey_id", got.map_or(-1, i64::from));
    common::finish(
        &r2,
        Status::Observed,
        "delivery fact; real key presses not verifiable on CI",
    );
}
