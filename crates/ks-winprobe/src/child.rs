//! Test-side handle of the separate fixture process (`ks-winprobe-fixture.exe`, S-2b).
//!
//! Protocol (stdout of the child, metadata only): one line
//! `READY <main> <single> <multi> <password> <rich|0>` with window handle values, then
//! zero or more `EVT <label> key=value...` lines. Closing the child's stdin ends it.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::fixture::{Controls, Foreground, Hwnd, request_focus, request_foreground};

/// A running fixture process.
#[derive(Debug)]
pub struct ChildFixture {
    child: Child,
    stdin: Option<ChildStdin>,
    /// Child process id.
    pub pid: u32,
    /// Window handles in the child.
    pub controls: Controls,
    /// `EVT` lines (without the prefix).
    pub events: mpsc::Receiver<String>,
}

impl ChildFixture {
    /// Starts `exe <mode>` and waits (≤ 10 s) for its READY line.
    pub fn spawn(exe: &Path, mode: &str) -> Result<Self, String> {
        let mut child = Command::new(exe)
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn fixture exe: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("no child stdout")?;
        let (ready_tx, ready_rx) = mpsc::channel::<String>();
        let (ev_tx, ev_rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("READY ") {
                    let _ = ready_tx.send(rest.to_owned());
                } else if let Some(rest) = line.strip_prefix("EVT ") {
                    let _ = ev_tx.send(rest.to_owned());
                }
            }
        });
        let pid = child.id();
        let ready = match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(r) => r,
            Err(_) => {
                let _ = child.kill();
                return Err("fixture exe did not report READY in 10 s".into());
            }
        };
        let v: Vec<isize> = ready
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if v.len() != 5 {
            let _ = child.kill();
            return Err("malformed READY line".into());
        }
        let h = Hwnd::from_isize;
        Ok(Self {
            child,
            stdin,
            pid,
            controls: Controls {
                main: h(v[0]),
                single: h(v[1]),
                multi: h(v[2]),
                password: h(v[3]),
                rich: (v[4] != 0).then(|| h(v[4])),
            },
            events: ev_rx,
        })
    }

    /// Asks the child to take the foreground.
    pub fn bring_to_foreground(&self) -> Foreground {
        request_foreground(self.controls.main)
    }

    /// Asks the child to focus `control`.
    pub fn focus(&self, control: Hwnd) -> bool {
        request_focus(self.controls.main, control)
    }

    /// Waits for an event line starting with `prefix`.
    pub fn wait_event(&self, prefix: &str, timeout: Duration) -> Option<(String, Duration)> {
        let start = Instant::now();
        while let Some(left) = timeout.checked_sub(start.elapsed()) {
            match self.events.recv_timeout(left) {
                Ok(l) if l.starts_with(prefix) => return Some((l, start.elapsed())),
                Ok(_) => {}
                Err(_) => return None,
            }
        }
        None
    }
}

impl Drop for ChildFixture {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Parses `key=<int>` from an event line.
pub fn event_field(line: &str, key: &str) -> Option<i64> {
    line.split_whitespace()
        .find_map(|t| t.strip_prefix(key)?.strip_prefix('=')?.parse().ok())
}
