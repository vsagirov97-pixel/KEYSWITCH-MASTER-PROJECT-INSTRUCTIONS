//! W-MAN-1: manual Windows test kit. The owner runs it on a real PC.
//!
//! Not product code. Ground rules (CLAUDE.md, S-2a security review WS-01..WS-08):
//! - refuses to start without `--i-understand` and refuses to run elevated;
//! - no network, no keyboard hooks, no injection into other processes;
//! - `fixture` mode writes/types only into the tool's own window; `survey` mode is read-only
//!   (UI Automation metadata of the focused element, no writes, no input, no clipboard);
//! - user text is never printed, logged or saved: the report holds lengths, flags, class
//!   names, executable file names and error codes only.

// On non-Windows hosts only the unit tests use most of the policy module.
#[cfg_attr(not(windows), allow(dead_code))]
mod policy;
#[cfg(windows)]
mod win;

use std::io::Write as _;
use std::process::ExitCode;

use policy::ArgError;

/// Writes a line to the console (stdout). Never pass user text.
pub fn say(s: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{s}");
    let _ = out.flush();
}

const USAGE: &str = "\
KeySwitch manual Windows probe (W-MAN-1)

Usage:
  .\\ks-win-manual-probe.exe --i-understand fixture
  .\\ks-win-manual-probe.exe --i-understand survey

fixture  - tests on the tool's own window (clipboard and keyboard layout are restored).
survey   - read-only: press Ctrl+Alt+Shift+F12 in another app's input field; q = finish.
The report (metadata only, no text) is saved to the Desktop as KeySwitch-probe-report.txt.
Run WITHOUT administrator rights.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match policy::parse_args(&args) {
        Ok(mode) => run(mode),
        Err(ArgError::Help) => {
            say(USAGE);
            ExitCode::SUCCESS
        }
        Err(e) => {
            say(match e {
                ArgError::NotAcknowledged => {
                    "Refused: add --i-understand (see the instructions W-MAN-1)."
                }
                ArgError::BadMode => "Refused: choose exactly one mode: fixture or survey.",
                _ => "Refused: unknown argument.",
            });
            say("");
            say(USAGE);
            ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
fn run(mode: policy::Mode) -> ExitCode {
    win::run(mode)
}

#[cfg(not(windows))]
fn run(_mode: policy::Mode) -> ExitCode {
    say("This tool runs on Windows only.");
    ExitCode::from(2)
}
