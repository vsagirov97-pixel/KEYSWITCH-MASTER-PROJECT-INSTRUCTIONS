//! `ks_engine::Platform` for Windows, and the worker thread that runs operations.
//!
//! One [`WinPlatform`] lives for one operation. `begin` captures the target fingerprint
//! (foreground HWND, focused HWND, pid, process start time) and applies the refusals; every
//! later step first re-checks the operation generation and the fingerprint, and W3 re-checks
//! them (plus held modifiers) before every `SendInput` batch.
//!
//! Write method choice: W1 (`EM_REPLACESEL`) only when the focused window's class is exactly
//! `Edit` (D-30), otherwise W3 (`SendInput` Unicode, at most [`W3_MAX_UNITS`] units, no
//! control/format characters). For an application with a row in [`profiles`] the write goes
//! through W2 (clipboard paste, [`w2`]) instead of W3, and a focused element without
//! TextPattern is read through Ctrl+C — the selection only.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ks_engine::{Choice, Config, Failure, Outcome, Platform, Refusal, Stop, TargetInfo, UserText};
use windows::Win32::UI::Accessibility::IUIAutomationElement;

use crate::gate::{self, CaptureError, Fingerprint, Gate, Token};
use crate::input::{self, SendError};
use crate::policy::{self, W1Refusal};
use crate::profiles::{self, Profile};
use crate::uia::{self, ComGuard, Read, Uia};
use crate::w2::{self, ClipboardHold, Settled, W2Error};
use crate::{desktop, edit, layout, popup, process};

/// W3 never types more than this many UTF-16 units.
pub const W3_MAX_UNITS: usize = 200;
/// How long W3 waits for the user to release the hotkey's modifiers.
pub const MODIFIER_WAIT: Duration = Duration::from_millis(1500);
/// Pause between W3 batches, letting the target consume the previous batch.
pub const BATCH_PAUSE: Duration = Duration::from_millis(20);
/// How often the idle worker checks whether a held clipboard snapshot has expired.
const HOLD_POLL: Duration = Duration::from_secs(5);
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
    W2,
    W3,
}

struct Target {
    fp: Fingerprint,
    element: IUIAutomationElement,
    method: Method,
    /// Read the selection via Ctrl+C (profiled application without usable TextPattern).
    copy_read: bool,
    /// The profile's paste signal was confirmed on the real application.
    signal_confirmed: bool,
}

/// Windows implementation of the engine's system access for one operation.
pub struct WinPlatform<'a> {
    uia: &'a Uia,
    gate: &'a Gate,
    token: Token,
    target: Option<Target>,
    profiles: &'a [Profile],
    clip: w2::Session,
    settled: Settled,
    popup_timeout: Duration,
}

impl<'a> WinPlatform<'a> {
    /// Starts a new operation generation (older operations become stale).
    /// Uses the default application profiles and a private clipboard hold.
    pub fn new(uia: &'a Uia, gate: &'a Gate) -> Self {
        Self::with_clipboard(uia, gate, profiles::DEFAULTS, ClipboardHold::new())
    }

    /// As [`WinPlatform::new`], with an explicit profile table and the shared hold that
    /// receives the user's clipboard snapshot after an unconfirmed paste.
    pub fn with_clipboard(
        uia: &'a Uia,
        gate: &'a Gate,
        profiles: &'a [Profile],
        hold: ClipboardHold,
    ) -> Self {
        Self {
            uia,
            gate,
            token: gate.begin(),
            target: None,
            profiles,
            clip: w2::Session::new(hold),
            settled: Settled::Nothing,
            popup_timeout: popup::TIMEOUT,
        }
    }

    /// Overrides how long the language-choice popup waits (default [`popup::TIMEOUT`]).
    pub fn set_popup_timeout(&mut self, timeout: Duration) {
        self.popup_timeout = timeout;
    }

    /// What the last W2 paste of this operation did with the user's clipboard.
    pub fn clipboard_settled(&self) -> Settled {
        self.settled
    }

    /// Post-check: `true` if the text right before the caret now equals `expected`.
    fn verify_by_uia(&self, expected: &UserText) -> bool {
        let units = units_i32(expected.utf16_len());
        let start = Instant::now();
        loop {
            let Ok(t) = self.checked() else {
                return false;
            };
            // A fresh element for every attempt: the element captured in `begin` may be
            // served by a client-side proxy (classic `Edit`) that keeps the text it saw
            // before the write.
            if let Some(element) = self.fresh_element(t.fp.pid)
                && let Ok(Some(now)) = text_of(self.uia.read_before_caret(&element, units))
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

    /// W3: `SendInput` Unicode with the fingerprint re-checked before every batch.
    fn write_w3(&self, fp: Fingerprint, text: &UserText) -> Result<(), Stop> {
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
        let (gate, token) = (self.gate, self.token);
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

    /// The currently focused element, if it still belongs to process `pid`.
    fn fresh_element(&self, pid: u32) -> Option<IUIAutomationElement> {
        let element = self.uia.focused().ok()?;
        // SAFETY: property getter on a live interface.
        let element_pid = unsafe { element.CurrentProcessId() }.ok()?;
        (u32::try_from(element_pid).ok() == Some(pid)).then_some(element)
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

fn clipboard_stop(e: W2Error, failure: Failure) -> Stop {
    match e {
        W2Error::Unavailable => Stop::Refused(Refusal::Clipboard),
        W2Error::Interference => Stop::Refused(Refusal::ClipboardListener),
        W2Error::TargetChanged => Stop::Failed(Failure::TargetChanged),
        W2Error::ModifiersHeld => Stop::Failed(Failure::ModifiersHeld),
        W2Error::Failed => Stop::Failed(failure),
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
        // Unknown executable: the excluded list cannot be honoured, so fail closed.
        let Some(exe) = process::exe_name(fp.pid) else {
            return refused(Refusal::Elevated);
        };
        let (class, real) = gate::class_names(fp.focus);
        let profile = profiles::find(self.profiles, &exe).copied();
        if profile.is_some_and(|p| !p.allows_class(&class)) {
            return refused(Refusal::Unsupported);
        }
        let style = gate::window_style(fp.focus);
        let method = match policy::w1_permitted(&class, &real, style) {
            Ok(()) => Method::W1,
            Err(W1Refusal::Password) => return refused(Refusal::Password),
            Err(W1Refusal::ReadOnly) => return refused(Refusal::ReadOnly),
            Err(W1Refusal::NotEditClass) => {
                if policy::is_edit_family(&class) && style & policy::ES_READONLY != 0 {
                    return refused(Refusal::ReadOnly);
                }
                // The clipboard only for a profiled application, never by default.
                if profile.is_some_and(|p| p.paste) {
                    Method::W2
                } else {
                    Method::W3
                }
            }
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
        let has_text_pattern = uia::text_pattern(&element).is_some();
        let copy_read = profile.is_some_and(|p| p.reads_by_copy(has_text_pattern));
        if !has_text_pattern && !copy_read {
            return refused(Refusal::Unsupported);
        }
        self.target = Some(Target {
            fp,
            element,
            method,
            copy_read,
            signal_confirmed: profile.is_some_and(|p| p.signal_confirmed),
        });
        Ok(TargetInfo { exe })
    }

    fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        let t = self.checked()?;
        if !t.copy_read {
            return text_of(self.uia.read_selection(&t.element, units_i32(max_units)));
        }
        let (fp, gate, token) = (t.fp, self.gate, self.token);
        let check = move || gate::pre_batch(&fp, gate, token);
        self.clip
            .copy_selection(fp.pid, max_units, &check)
            .map_err(|e| clipboard_stop(e, Failure::Read))
    }

    fn read_before_caret(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        let t = self.checked()?;
        if t.copy_read {
            // Ctrl+C reads a selection only: no "last word" mode without TextPattern.
            return Ok(None);
        }
        text_of(self.uia.read_before_caret(&t.element, units_i32(max_units)))
    }

    fn select_before_caret(&mut self, units: usize) -> Result<(), Stop> {
        let t = self.checked()?;
        if t.copy_read {
            return Err(Stop::Failed(Failure::Write));
        }
        match self.uia.select_before_caret(&t.element, units_i32(units)) {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(Stop::Failed(Failure::Write)),
        }
    }

    fn restore_caret(&mut self, selected: &UserText) {
        // Bound to the captured element, not to whatever has the focus now: the focus may
        // have moved away (that is often why the write stopped). Only the same process
        // instance, only if the selection is still exactly the fragment we selected; a
        // UIA selection change, no input is sent.
        let Some(t) = self.target.as_ref() else {
            return;
        };
        if t.copy_read || gate::process_start_time(t.fp.pid) != Some(t.fp.start_time) {
            return;
        }
        let _ = self
            .uia
            .collapse_selection_if(&t.element, selected.expose());
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
            Method::W3 => self.write_w3(t.fp, text),
            Method::W2 => {
                let (fp, gate, token) = (t.fp, self.gate, self.token);
                let check = move || gate::pre_batch(&fp, gate, token);
                match self.clip.paste(fp.pid, text, &check) {
                    Ok(()) => Ok(()),
                    // A listener rendered before Ctrl+V: nothing was pasted and the
                    // clipboard is back. §4.1 p.3: W3 if the text allows it, else refuse.
                    Err(W2Error::Interference) => match self.write_w3(fp, text) {
                        Err(Stop::Refused(_)) => Err(Stop::Refused(Refusal::ClipboardListener)),
                        other => other,
                    },
                    Err(e) => Err(clipboard_stop(e, Failure::Write)),
                }
            }
        }
    }

    fn verify(&mut self, expected: &UserText) -> bool {
        let readable = self.target.as_ref().is_some_and(|t| !t.copy_read);
        let verified = readable.then(|| self.verify_by_uia(expected));
        if self.clip.paste_pending() {
            let confirmed = self.target.as_ref().is_some_and(|t| t.signal_confirmed);
            // §4.1 p.4-5: restore only on the target's signal and a positive post-check.
            self.settled = self.clip.settle(verified, confirmed);
        }
        verified.unwrap_or(false)
    }

    fn choose_language(&mut self, langs: &[&str]) -> Choice {
        let Ok(t) = self.checked() else {
            return Choice::Cancelled;
        };
        let (fp, gate, token) = (t.fp, self.gate, self.token);
        // The popup never takes activation, so the fingerprint must stay the same while it
        // is open; a newer operation or a focus change closes it.
        let still_valid = move || gate.is_current(token) && gate::capture().ok() == Some(fp);
        match popup::ask(langs, fp.focus, self.popup_timeout, &still_valid) {
            popup::Answer::Picked(i) => Choice::Picked(i),
            popup::Answer::Cancelled => Choice::Cancelled,
            popup::Answer::Unavailable => Choice::Unavailable,
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
    hold: ClipboardHold,
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

    /// `true` while the user's clipboard from before an unconfirmed paste can be restored.
    pub fn clipboard_held(&self) -> bool {
        self.hold.is_held()
    }

    /// The user's explicit "restore clipboard" action.
    pub fn restore_clipboard(&self) -> w2::HoldRestore {
        self.hold.restore()
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
    let hold = ClipboardHold::new();
    let thread_hold = hold.clone();
    std::thread::Builder::new()
        .name("ks-engine".into())
        .spawn(move || {
            let com = ComGuard::mta();
            let uia = com.as_ref().ok().and_then(|_| Uia::new().ok());
            loop {
                match rx.recv_timeout(HOLD_POLL) {
                    Ok(()) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // A held clipboard snapshot lives at most `w2::HOLD_MAX`.
                        thread_hold.expire();
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                if let Some(cfg) = config() {
                    let outcome = match &uia {
                        Some(uia) => {
                            let mut platform = WinPlatform::with_clipboard(
                                uia,
                                &thread_gate,
                                profiles::DEFAULTS,
                                thread_hold.clone(),
                            );
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
    Some(Worker { tx, gate, hold })
}
