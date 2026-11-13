//! W2: clipboard paste, and reading the selection via Ctrl+C, for profiled applications.
//!
//! Contract (SYSTEM_ARCHITECTURE §4.1, D-26, CISO Q-1; mechanisms proven by probe scenarios
//! h1–h6):
//! 1. The user's clipboard is snapshotted by whitelist under a deadline; foreign formats,
//!    more than 16 MB, a hung owner or a remote session refuse the method before anything
//!    is touched.
//! 2. The temporary text is offered by delayed rendering together with the privacy markers.
//! 3. A render **before** our Ctrl+V is a listener: abort, restore, never paste.
//! 4. After Ctrl+V only a render requested by the target's process is the paste signal.
//! 5. The snapshot is restored only on that signal **and** a positive post-check (or, when
//!    the result cannot be read back, only for a profile with a confirmed signal), always
//!    under the owner + sequence-number guard.
//! 6. Otherwise nothing is restored blindly: the temporary text stays (as real data, with
//!    markers) and the snapshot is held for an explicit "restore clipboard" action, at most
//!    [`HOLD_MAX`] or until the next clipboard operation.
//!
//! Nothing here returns or logs text; errors are codes.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ks_engine::UserText;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};

use crate::clipboard::{self, DelayedOwner, Render, Restore, Snapshot};
use crate::gate::{self, Abort};
use crate::input::{self, CHORD_EVENTS, Chord};

/// Deadline for the snapshot and for reading the target's copy (XR-04).
pub const SNAPSHOT_DEADLINE: Duration = Duration::from_millis(250);
/// Quiet window after the delayed put: any render in it is a listener (value used in h3/h4).
pub const QUIET: Duration = Duration::from_millis(400);
/// How long the target has to request the text after Ctrl+V (value used in h4/h5).
pub const SIGNAL: Duration = Duration::from_millis(500);
/// How long the target has to put its selection on the clipboard after Ctrl+C.
pub const COPY_WAIT: Duration = Duration::from_millis(500);
/// How long W2 waits for the hotkey's modifiers to be released.
pub const MODIFIER_WAIT: Duration = Duration::from_millis(1500);
/// A held snapshot is destroyed after this long (CISO Q-1).
pub const HOLD_MAX: Duration = Duration::from_secs(120);

/// Why a clipboard step stopped. Codes only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum W2Error {
    /// The clipboard cannot be used safely now (foreign formats, size, hung owner, remote
    /// session). Nothing was touched.
    Unavailable,
    /// Someone other than the target touched the clipboard during the operation.
    Interference,
    /// Focus, foreground, process or operation generation changed.
    TargetChanged,
    /// A modifier key stayed down.
    ModifiersHeld,
    /// A system call failed.
    Failed,
}

impl From<Abort> for W2Error {
    fn from(a: Abort) -> Self {
        match a {
            Abort::Modifiers => W2Error::ModifiersHeld,
            Abort::Generation | Abort::TargetChanged | Abort::NotDelivered => {
                W2Error::TargetChanged
            }
        }
    }
}

/// What happened to the user's clipboard at the end of a paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settled {
    /// No paste was pending.
    Nothing,
    /// The user's clipboard is back.
    Restored,
    /// Consumption was not confirmed: the temporary text stays, the snapshot is held.
    Held,
    /// Someone else owns the clipboard now; their data is kept, the snapshot is dropped.
    Kept,
}

/// Result of the explicit "restore clipboard" action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldRestore {
    /// The held snapshot was put back.
    Restored,
    /// The clipboard changed since the operation; the newer data is kept.
    Skipped,
    /// Nothing was held (or it had expired).
    Nothing,
}

struct Held {
    snap: Snapshot,
    owner: DelayedOwner,
    seq: u32,
    at: Instant,
}

/// The user's clipboard snapshot kept after an unconfirmed paste. Shared between the
/// operation worker and the UI thread; holds no accessor to the data.
#[derive(Clone, Default)]
pub struct ClipboardHold(Arc<Mutex<Option<Held>>>);

impl std::fmt::Debug for ClipboardHold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Metadata only: the snapshot itself has no `Debug`.
        f.debug_struct("ClipboardHold")
            .field("held", &self.is_held())
            .finish()
    }
}

impl ClipboardHold {
    /// An empty hold.
    pub fn new() -> Self {
        Self::default()
    }

    fn slot(&self) -> MutexGuard<'_, Option<Held>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `true` while a snapshot can still be restored.
    pub fn is_held(&self) -> bool {
        self.slot()
            .as_ref()
            .is_some_and(|h| h.at.elapsed() <= HOLD_MAX)
    }

    /// Destroys the held snapshot (zeroed on drop).
    pub fn discard(&self) {
        let held = self.slot().take();
        drop(held);
    }

    /// Destroys the held snapshot if it is older than [`HOLD_MAX`].
    pub fn expire(&self) {
        let mut slot = self.slot();
        if slot.as_ref().is_some_and(|h| h.at.elapsed() > HOLD_MAX) {
            let held = slot.take();
            drop(slot);
            drop(held);
        }
    }

    /// The user's explicit action: puts the held snapshot back if the clipboard still holds
    /// what the operation left there.
    pub fn restore(&self) -> HoldRestore {
        let Some(h) = self.slot().take() else {
            return HoldRestore::Nothing;
        };
        if h.at.elapsed() > HOLD_MAX {
            return HoldRestore::Nothing;
        }
        match clipboard::restore_snapshot(h.owner.hwnd, h.owner.hwnd, h.seq, &h.snap) {
            Ok(Restore::Restored) => HoldRestore::Restored,
            _ => HoldRestore::Skipped,
        }
    }
}

/// `true` in a Remote Desktop session (clipboard redirection: W2 is forbidden, §4.1 p.2).
pub fn remote_session() -> bool {
    // SAFETY: trivial metric query.
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}

fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if cond() {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_modifiers() -> Result<(), W2Error> {
    // The hotkey's own modifiers are usually still down: Ctrl+C would become another chord.
    if wait_until(MODIFIER_WAIT, || !gate::modifiers_down()) {
        Ok(())
    } else {
        Err(W2Error::ModifiersHeld)
    }
}

fn user_snapshot() -> Result<Snapshot, W2Error> {
    if remote_session() {
        return Err(W2Error::Unavailable);
    }
    // The worker handle is dropped: if the owner hangs, the worker ends on its own.
    let (snap, _worker) = clipboard::snapshot_guarded(SNAPSHOT_DEADLINE);
    snap.map_err(|_| W2Error::Unavailable)
}

/// Sequence number the restore guard expects: after our put or, if our owner rendered on
/// request, right after the last render.
fn expected_seq(put_seq: u32, renders: &[Render]) -> u32 {
    renders.last().map_or(put_seq, |r| r.seq_after)
}

struct Pending {
    snap: Snapshot,
    put_seq: u32,
    signal: bool,
}

/// Clipboard state of one operation.
pub struct Session {
    hold: ClipboardHold,
    owner: Option<DelayedOwner>,
    pending: Option<Pending>,
}

impl Session {
    /// A session that parks unconfirmed snapshots in `hold`.
    pub fn new(hold: ClipboardHold) -> Self {
        Self {
            hold,
            owner: None,
            pending: None,
        }
    }

    /// `true` if a paste awaits [`Session::settle`].
    pub fn paste_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn take_owner(&mut self) -> Result<DelayedOwner, W2Error> {
        if let Some(owner) = self.owner.take() {
            return Ok(owner);
        }
        // "Until the next operation": the held snapshot (and its owner window — there is
        // one owner at a time) ends here.
        self.hold.discard();
        DelayedOwner::spawn().map_err(|_| W2Error::Failed)
    }

    /// Reads the target's **selection** via Ctrl+C. `Ok(None)` = the target copied nothing
    /// (no selection); the clipboard was not changed then. The user's clipboard is put back
    /// under the guard before returning.
    pub fn copy_selection(
        &mut self,
        target_pid: u32,
        max_units: usize,
        check: &dyn Fn() -> Result<(), Abort>,
    ) -> Result<Option<UserText>, W2Error> {
        let snap = user_snapshot()?;
        let owner = self.take_owner()?;
        let result = copy_inner(&owner, &snap, target_pid, max_units, check);
        self.owner = Some(owner);
        result
    }

    /// Offers `text` by delayed rendering and sends Ctrl+V to the target. `Ok` = the chord
    /// was sent; the clipboard is settled later by [`Session::settle`].
    pub fn paste(
        &mut self,
        target_pid: u32,
        text: &UserText,
        check: &dyn Fn() -> Result<(), Abort>,
    ) -> Result<(), W2Error> {
        let snap = user_snapshot()?;
        let owner = self.take_owner()?;
        let result = paste_inner(&owner, snap, target_pid, text, check);
        self.owner = Some(owner);
        self.pending = Some(result?);
        Ok(())
    }

    /// Ends a pending paste. `verified`: the post-check result, `None` if the target cannot
    /// be read back. `signal_confirmed`: the profile's signal was confirmed on the real app.
    pub fn settle(&mut self, verified: Option<bool>, signal_confirmed: bool) -> Settled {
        let (Some(p), Some(owner)) = (self.pending.take(), self.owner.take()) else {
            return Settled::Nothing;
        };
        let confirmed = p.signal && verified.unwrap_or(signal_confirmed);
        if confirmed {
            let expected = expected_seq(p.put_seq, &owner.renders());
            match clipboard::restore_snapshot(owner.hwnd, owner.hwnd, expected, &p.snap) {
                Ok(Restore::Restored) => {
                    self.owner = Some(owner);
                    return Settled::Restored;
                }
                Ok(_) => {
                    self.owner = Some(owner);
                    return Settled::Kept;
                }
                // Could not open the clipboard: fall through and keep the snapshot.
                Err(_) => {}
            }
        }
        // No confirmed consumption: a restore now could make a slow target paste the user's
        // previous clipboard (XR-02). Keep the temporary text, as real data so it survives
        // our owner window, and hold the snapshot for the explicit action.
        match owner.materialize() {
            Some(seq) => {
                *self.hold.slot() = Some(Held {
                    snap: p.snap,
                    owner,
                    seq,
                    at: Instant::now(),
                });
                Settled::Held
            }
            None => {
                self.owner = Some(owner);
                Settled::Kept
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // A paste nobody settled is an unconfirmed paste.
        if self.pending.is_some() {
            self.settle(Some(false), false);
        }
    }
}

fn copy_inner(
    owner: &DelayedOwner,
    snap: &Snapshot,
    target_pid: u32,
    max_units: usize,
    check: &dyn Fn() -> Result<(), Abort>,
) -> Result<Option<UserText>, W2Error> {
    wait_modifiers()?;
    check()?;
    let seq_before = clipboard::seq();
    if input::send_chord(Chord::Copy) != CHORD_EVENTS {
        return Err(W2Error::Failed);
    }
    if !wait_until(COPY_WAIT, || clipboard::seq() != seq_before) {
        // Nothing was copied: no selection. The clipboard is untouched.
        return Ok(None);
    }
    let Some(read) = clipboard::read_text_guarded(SNAPSHOT_DEADLINE, max_units) else {
        return Err(W2Error::Failed);
    };
    // Only a copy made by the target's process is the selection. Anything else is another
    // writer: its data is not ours to use or to overwrite.
    if clipboard::window_pid(read.owner) != target_pid {
        return Err(W2Error::Interference);
    }
    // Guard: still the target's copy (same owner window, same sequence number).
    let _ = clipboard::restore_snapshot(owner.hwnd, read.owner, read.seq_after, snap);
    Ok(read.text.filter(|t| !t.is_empty()))
}

fn paste_inner(
    owner: &DelayedOwner,
    snap: Snapshot,
    target_pid: u32,
    text: &UserText,
    check: &dyn Fn() -> Result<(), Abort>,
) -> Result<Pending, W2Error> {
    wait_modifiers()?;
    let restore = |put_seq: u32| {
        let expected = expected_seq(put_seq, &owner.renders());
        let _ = clipboard::restore_snapshot(owner.hwnd, owner.hwnd, expected, &snap);
    };
    let put_seq = match owner.put_delayed(text.expose(), true) {
        Ok(seq) => seq,
        Err(_) => {
            // If the failure came after EmptyClipboard we own an emptied clipboard: the
            // guard lets the user's data back in exactly then.
            restore(clipboard::seq());
            return Err(W2Error::Failed);
        }
    };
    // Nobody has been asked to paste yet: whoever renders now is a listener (XR-01a).
    if wait_until(QUIET, || !owner.renders().is_empty()) {
        restore(put_seq);
        return Err(W2Error::Interference);
    }
    if let Err(abort) = check() {
        restore(put_seq);
        return Err(abort.into());
    }
    if input::send_chord(Chord::Paste) == 0 {
        restore(put_seq);
        return Err(W2Error::Failed);
    }
    let signal = wait_until(SIGNAL, || {
        owner
            .renders()
            .iter()
            .any(|r| r.rendered && r.requester_pid == target_pid)
    });
    Ok(Pending {
        snap,
        put_seq,
        signal,
    })
}
