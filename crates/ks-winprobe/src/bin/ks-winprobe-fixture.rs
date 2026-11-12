//! S-2b separate fixture process. NOT product code.
//!
//! Modes (argv[1]):
//! - `edit`        — the S-2a Win32 fixture (EDIT single/multi/password, RICHEDIT50W);
//! - `late`        — same, plus XR-02 "reads the clipboard 1500 ms after Ctrl+V";
//! - `listener`    — same, plus XR-01(a) hostile clipboard listener that forces rendering;
//! - `hung-owner`  — same, plus XR-04 clipboard owner that hangs inside WM_RENDERFORMAT;
//! - `inject-hotkey` — injects Ctrl+Alt+Shift+F14 down/up once; exit code = events injected;
//! - `report-il`   — exit code = integrity RID of this process.
//!
//! Output is metadata only (`READY` handles, `EVT` counters); never text content.
#![cfg_attr(not(windows), allow(unused))]

#[cfg(windows)]
fn main() {
    std::process::exit(win::run());
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
mod win {
    use std::io::{Read, Write};

    use ks_winprobe::fixture::{self, ChildMode, Fixture};
    use ks_winprobe::{input, integrity};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_F14, VK_MENU, VK_SHIFT};

    fn sink(line: &str) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "EVT {line}");
        let _ = out.flush();
    }

    pub fn run() -> i32 {
        let mode = std::env::args().nth(1).unwrap_or_default();
        let child_mode = match mode.as_str() {
            "edit" => ChildMode::Plain,
            "late" => ChildMode::LatePaste,
            "listener" => ChildMode::Listener,
            "hung-owner" => ChildMode::HungOwner,
            "inject-hotkey" => {
                return input::send_vk_chord(&[VK_CONTROL, VK_MENU, VK_SHIFT, VK_F14]) as i32;
            }
            "report-il" => return integrity::current_rid().map_or(-1, |r| r as i32),
            _ => return 2,
        };
        fixture::set_child_mode(child_mode);
        fixture::set_event_sink(sink);
        let fx = match Fixture::spawn() {
            Ok(f) => f,
            Err(_) => return 3,
        };
        let c = fx.controls;
        {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(
                out,
                "READY {} {} {} {} {}",
                c.main.as_isize(),
                c.single.as_isize(),
                c.multi.as_isize(),
                c.password.as_isize(),
                c.rich.map_or(0, |h| h.as_isize())
            );
            let _ = out.flush();
        }
        // Live until the test closes our stdin (or kills us).
        let mut sinkhole = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut sinkhole);
        drop(fx);
        0
    }
}
