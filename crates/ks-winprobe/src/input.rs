//! Probe-only input helpers plus a re-export of the product W3 writer.

pub use ks_platform_win::input::*;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY,
};

/// Sends raw virtual-key down/up events. PROBE-ONLY (WS-07): a generic VK sender must never
/// reach product code (CI guard step forbids the name outside `crates/ks-winprobe`).
/// Returns the number of injected events.
pub fn send_vk_chord(vks: &[VIRTUAL_KEY]) -> u32 {
    let mut v: Vec<INPUT> = Vec::new();
    let mk = |vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    for &vk in vks {
        v.push(mk(vk, KEYBD_EVENT_FLAGS(0)));
    }
    for &vk in vks.iter().rev() {
        v.push(mk(vk, KEYEVENTF_KEYUP));
    }
    // SAFETY: `v` holds fully initialised keyboard INPUTs; cbSize is one INPUT.
    unsafe { SendInput(&v, std::mem::size_of::<INPUT>() as i32) }
}

/// Injects a single key-down (`down = true`) or key-up for `vk`. PROBE-ONLY (WS-07), used to
/// simulate a held modifier (WS-05). Returns the number of injected events.
pub fn send_vk_state(vk: VIRTUAL_KEY, down: bool) -> u32 {
    let flags = if down {
        KEYBD_EVENT_FLAGS(0)
    } else {
        KEYEVENTF_KEYUP
    };
    let ev = [INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }];
    // SAFETY: one fully initialised keyboard INPUT; cbSize is one INPUT.
    unsafe { SendInput(&ev, std::mem::size_of::<INPUT>() as i32) }
}
