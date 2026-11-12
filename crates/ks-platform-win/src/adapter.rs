//! `ks_engine::Platform` for Windows, and the worker thread that runs operations.
//!
//! One [`WinPlatform`] lives for one operation. `begin` captures the target fingerprint
//! (foreground HWND, focused HWND, pid, process start time) and applies the refusals; every
//! later step first re-checks the operation generation and the fingerprint, and W3 re-checks
//! them (plus held modifiers) before every `SendInput` batch.
//!
//! Write method choice: W1 (`EM_REPLACESEL`) only when the focused window's class is exactly
//! `Edit` (D-30), otherwise W3 (`SendInput` Unicode, at most [`W3_MAX_UNITS`] units, no
//! control/format characters). W2 (clipboard) is not used in Beta 1.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ks_engine::{Config, Failure, Outcome, Platform, Refusal, Stop, TargetInfo, UserText};
use windows::Win32::UI::Accessibility::IUIAutomationElement;

use crate::gate::{self, CaptureError, Fingerprint, Gate, Token};
use crate::input::{self, SendError};
use crate::policy::{self, W1Refusal};
use crate::uia::{self, ComGuard, Read, Uia};
use crate::{desktop, edit, layout, process};

/// W3 never types more than this many UTF-16 units.
pub const W3_MAX_UNITS: usize = 200;
/// How long W3 waits for the user to release the hotkey's modifiers.
pub const MODIFIER_WAIT: Duration = Duration::from_millis(1500);
/// Pause between W3 batches, letting the target consume the previous batch.
pub const BATCH_PAUSE: Duration = Duration::from_millis(20);
/// How long the post-check waits for the written text to appear.
pub const VERIFY_WAIT: Duration = Duration::from_millis(800);

/// Top-level window classes of consoles and terminals (refused: no text field semantics).
const CONSOLE_CLASSES: [&str; 3] = [
    "ConsoleWindowClass",
    "CASCADIA_HOSTING_WINDOW_CLASS",
    "PseudoConsoleWindow",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    W1,
    W3,
}

struct Target {
    fp: Fingerprint,
    element: IUIAutomationElement,
    method: Method,
}

/// Windows implementation of the engine's system access for one operation.
pub struct WinPlatform<'a> {
    uia: &'a Uia,
    gate: &'a Gate,
    token: Token,
    target: Option<Target>,
}

impl<'a> WinPlatform<'a> {
    /// Starts a new operation generation (older operations become stale).
    pub fn new(uia: &'a Uia, gate: &'a Gate) -> Self {
        Self {
            uia,
            gate,
            token: gate.begin(),
            target: None,
        }
    }

    /// The captured target, after re-checking generation and fingerprint.
    fn checked(&self) -> Result<&Target, Stop> {
        let t = self
            .target
            .as_ref()
            .ok_or(Stop::Failed(Failure::TargetChanged))?;
        if !self.gate.is_current(self.token) || gate::capture().ok() != Some(t.fp) {
            return Err(Stop::Failed(Failure::TargetChanged));
        }
        Ok(t)
    }
}

fn text_of(read: Result<Option<Read>, String>) -> Result<Option<UserText>, Stop> {
    match read {
        Ok(Some(Read::Text(s))) => Ok(Some(UserText::new(s))),
        Ok(Some(Read::RefusedPassword)) => Err(Stop::Refused(Refusal::Password)),
        Ok(Some(Read::NoTextPattern)) => Err(Stop::Refused(Refusal::Unsupported)),
        Ok(None) => Ok(None),
        // The error string names an API and an HRESULT only; it is dropped here.
        Err(_) => Err(Stop::Failed(Failure::Read)),
    }
}

fn units_i32(units: usize) -> i32 {
    i32::try_from(units).unwrap_or(i32::MAX)
}

impl Platform for WinPlatform<'_> {
    fn begin(&mut self) -> Result<TargetInfo, Stop> {
        let refused = |r| Err(Stop::Refused(r));
        if !desktop::probe().input_desktop {
            return refused(Refusal::SecureDesktop);
        }
        let fp = match gate::capture() {
            Ok(fp) => fp,
            // A process whose start time cannot be read is protected or elevated.
            Err(CaptureError::NoStartTime) => return refused(Refusal::Elevated),
            Err(CaptureError::NoForeground | CaptureError::NoFocus) => {
                return refused(Refusal::Unsupported);
            }
        };
        if gate::classify(&fp).is_err() {
            return refused(Refusal::SelfTarget);
        }
        if process::is_elevated_target(fp.pid) {
            return refused(Refusal::Elevated);
        }
        let (fg_class, _) = gate::class_names(fp.foreground);
        if CONSOLE_CLASSES
            .iter()
            .any(|c| fg_class.eq_ignore_ascii_case(c))
        {
            return refused(Refusal::Console);
        }
        if gate::hwnd_is_password(fp.focus) {
            return refused(Refusal::Password);
        }
        let (class, real) = gate::class_names(fp.focus);
        let style = gate::window_style(fp.focus);
        let method = match policy::w1_permitted(&class, &real, style) {
            Ok(()) => Method::W1,
            Err(W1Refusal::Password) => return refused(Refusal::Password),
            Err(W1Refusal::ReadOnly) => return refused(Refusal::ReadOnly),
            Err(W1Refusal::NotEditClass) => {
                if policy::is_edit_family(&class) && style & policy::ES_READONLY != 0 {
                    return refused(Refusal::ReadOnly);
                }
                Method::W3
            }
        };
        // Unknown executable: the excluded list cannot be honoured, so fail closed.
        let Some(exe) = process::exe_name(fp.pid) else {
            return refused(Refusal::Elevated);
        };
        let Ok(element) = self.uia.focused() else {
            return refused(Refusal::Unsupported);
        };
        // SAFETY: property getter on a live interface.
        let element_pid = unsafe { element.CurrentProcessId() }.unwrap_or(0);
        if u32::try_from(element_pid).ok() != Some(fp.pid) {
            // The focused element belongs to another process than the focused window
            // (hosted/UWP content): not supported in Beta 1.
            return refused(Refusal::Unsupported);
        }
        if uia::password_guard(&element).is_some() {
            return refused(Refusal::Password);
        }
        if uia::text_pattern(&element).is_none() {
            return refused(Refusal::Unsupported);
        }
        self.target = Some(Target {
            fp,
            element,
            method,
        });
        Ok(TargetInfo { exe })
    }

    fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        let t = self.checked()?;
        text_of(self.uia.read_selection(&t.element, units_i32(max_units)))
    }

    fn read_before_caret(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        let t = self.checked()?;
        text_of(self.uia.read_before_caret(&t.element, units_i32(max_units)))
    }

    fn select_before_caret(&mut self, units: usize) -> Result<(), Stop> {
        let t = self.checked()?;
        match self.uia.select_before_caret(&t.element, units_i32(units)) {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(Stop::Failed(Failure::Write)),
        }
    }

    fn write(&mut self, text: &UserText) -> Result<(), Stop> {
        let t = self.checked()?;
        match t.method {
            Method::W1 => {
                if edit::replace_selection(t.fp.focus, text.expose()) {
                    Ok(())
                } else {
                    Err(Stop::Failed(Failure::Write))
                }
            }
            Method::W3 => {
                if text.utf16_len() > W3_MAX_UNITS {
                    return Err(Stop::Refused(Refusal::TooLong));
                }
                if policy::first_forbidden(text.expose()).is_some() {
                    return Err(Stop::Refused(Refusal::ForbiddenChars));
                }
                // The hotkey's own modifiers are usually still down: typed characters would
                // become shortcuts. Wait for the release, bounded.
                let start = Instant::now();
                while gate::modifiers_down() {
                    if start.elapsed() > MODIFIER_WAIT {
                        return Err(Stop::Failed(Failure::ModifiersHeld));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                let (fp, gate, token) = (t.fp, self.gate, self.token);
                let mut first = true;
                let sent = input::send_unicode(text.expose(), || {
                    if !std::mem::take(&mut first) {
                        std::thread::sleep(BATCH_PAUSE);
                    }
                    gate::pre_batch(&fp, gate, token).is_ok()
                });
                match sent {
                    Ok(_) => Ok(()),
                    Err(SendError::Refused(_)) => Err(Stop::Refused(Refusal::ForbiddenChars)),
                    Err(SendError::TargetLost { .. }) => Err(Stop::Failed(Failure::TargetChanged)),
                    Err(SendError::Blocked { .. }) => Err(Stop::Failed(Failure::Write)),
                }
            }
        }
    }

    fn verify(&mut self, expected: &UserText) -> bool {
        let units = units_i32(expected.utf16_len());
        let start = Instant::now();
        loop {
            let Ok(t) = self.checked() else {
                return false;
            };
            if let Ok(Some(now)) = text_of(self.uia.read_before_caret(&t.element, units))
                && now.same_as(expected)
            {
                return true;
            }
            if start.elapsed() > VERIFY_WAIT {
                return false;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    fn switch_layout(&mut self, lang: &str) -> bool {
        // Primary language ids (low 10 bits of the LANGID).
        let want: u16 = match lang {
            "ru" => 0x19,
            "en" => 0x09,
            _ => return false,
        };
        let Ok(t) = self.checked() else {
            return false;
        };
        let primary = |h: layout::Hkl| h.lang_id() & 0x3FF;
        if primary(layout::thread_layout(gate::window_thread(t.fp.focus))) == want {
            return true;
        }
        layout::loaded_layouts()
            .into_iter()
            .find(|h| primary(*h) == want)
            .is_some_and(|h| layout::request_switch(t.fp.focus, h))
    }
}

/// Handle of the operation worker thread.
#[derive(Debug)]
pub struct Worker {
    tx: mpsc::Sender<()>,
    gate: Arc<Gate>,
}

impl Worker {
    /// Requests one operation (called from the hotkey thread). Presses that arrive while an
    /// operation is running are dropped.
    pub fn trigger(&self) {
        let _ = self.tx.send(());
    }

    /// Invalidates a running operation (pause, quit).
    pub fn cancel(&self) {
        self.gate.bump();
    }
}

/// Starts the worker thread (MTA, one UI Automation client). For every trigger it asks
/// `config` for the current policy (`None` = paused: do nothing), runs the pipeline and hands
/// the outcome code to `on_outcome`. `None` if the thread could not be started.
pub fn spawn_worker(
    converter: ks_core::Converter,
    config: impl Fn() -> Option<Config> + Send + 'static,
    on_outcome: impl Fn(Outcome) + Send + 'static,
) -> Option<Worker> {
    let (tx, rx) = mpsc::channel::<()>();
    let gate = Arc::new(Gate::new());
    let thread_gate = Arc::clone(&gate);
    std::thread::Builder::new()
        .name("ks-engine".into())
        .spawn(move || {
            let com = ComGuard::mta();
            let uia = com.as_ref().ok().and_then(|_| Uia::new().ok());
            while rx.recv().is_ok() {
                if let Some(cfg) = config() {
                    let outcome = match &uia {
                        Some(uia) => {
                            let mut platform = WinPlatform::new(uia, &thread_gate);
                            ks_engine::run(&mut platform, &converter, &cfg)
                        }
                        None => Outcome::Refused(Refusal::Unsupported),
                    };
                    on_outcome(outcome);
                }
                // Presses during the operation are ignored, not queued.
                while rx.try_recv().is_ok() {}
            }
            drop(uia);
            drop(com);
        })
        .ok()?;
    Some(Worker { tx, gate })
}
