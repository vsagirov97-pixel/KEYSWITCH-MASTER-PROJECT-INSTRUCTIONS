//! Clipboard: whitelist snapshot of the owner's clipboard, a private test write and a
//! restore that happens only if nobody wrote after us.
//!
//! - WS-01: our sequence number is read while the clipboard is still open by us.
//! - WS-02: the snapshot keeps the D-13 privacy markers with their original bytes and the
//!   restore puts them back, so a password copied from a password manager stays excluded
//!   from Win+V history / cloud clipboard.
//! - Only plain-text clipboards are snapshotted (policy::clipboard_restorable); otherwise the
//!   clipboard scenarios are skipped and the clipboard is not touched at all.
//! - Snapshot bytes live only in memory, are never printed, and are zeroed on drop.

use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::core::w;

use super::sys::Hwnd;
use crate::policy::{self, cf};

/// Clipboard opened by us; closed on drop.
#[derive(Debug)]
pub struct Open(());

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: exists only while we hold the clipboard open.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Opens the clipboard with `owner`, retrying for ~1 s.
pub fn open(owner: Hwnd) -> Result<Open, String> {
    for _ in 0..50 {
        // SAFETY: `owner` is our own window; success is paired with CloseClipboard.
        if unsafe { OpenClipboard(Some(owner.raw())) }.is_ok() {
            return Ok(Open(()));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err("OpenClipboard failed for 1 s".into())
}

/// Clipboard sequence number.
pub fn seq() -> u32 {
    // SAFETY: trivial query.
    unsafe { GetClipboardSequenceNumber() }
}

/// Format ids on the clipboard (ids only, no data).
pub fn format_ids(_open: &Open) -> Vec<u32> {
    let mut out = Vec::new();
    let mut f = 0u32;
    loop {
        // SAFETY: clipboard is open; only lists ids.
        f = unsafe { EnumClipboardFormats(f) };
        if f == 0 {
            break;
        }
        out.push(f);
    }
    out
}

/// D-13 privacy formats: ExcludeClipboardContentFromMonitorProcessing,
/// CanIncludeInClipboardHistory, CanUploadToCloudClipboard (0 = registration failed).
pub fn privacy_formats() -> [u32; 3] {
    // SAFETY: registering well-known names (static strings).
    unsafe {
        [
            RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
            RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
            RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
        ]
    }
}

/// `true` if `format` is available.
pub fn available(format: u32) -> bool {
    // SAFETY: trivial query.
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Bytes that must never be printed; zeroed on drop. No Debug/Display.
pub struct Secret(Vec<u8>);

impl Drop for Secret {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            // SAFETY: `b` is a valid exclusive reference into our own buffer; volatile so
            // the zeroing is not optimised away.
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

impl Secret {
    /// Length in bytes (metadata).
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Constant-shape comparison helper (equality only; result is a bool).
    pub fn same_as(&self, other: &Secret) -> bool {
        self.0 == other.0
    }
}

/// Whitelisted snapshot of the owner's clipboard.
pub struct Snapshot {
    /// Format ids at snapshot time (metadata).
    pub formats: Vec<u32>,
    unicode: Option<Secret>,
    privacy: Vec<(u32, Secret)>,
}

impl Snapshot {
    /// Byte length of the saved Unicode text (metadata).
    pub fn unicode_bytes(&self) -> usize {
        self.unicode.as_ref().map_or(0, Secret::len)
    }
    /// Ids of saved privacy markers.
    pub fn privacy_ids(&self) -> Vec<u32> {
        self.privacy.iter().map(|(f, _)| *f).collect()
    }
}

fn read_bytes(_open: &Open, format: u32) -> Option<Secret> {
    // SAFETY: clipboard is open; the handle stays owned by the system; we lock it, copy at
    // most GlobalSize bytes and unlock.
    unsafe {
        let h = GetClipboardData(format).ok()?;
        let hg = HGLOBAL(h.0);
        let size = GlobalSize(hg);
        let p = GlobalLock(hg) as *const u8;
        if p.is_null() {
            return None;
        }
        let v = std::slice::from_raw_parts(p, size).to_vec();
        let _ = GlobalUnlock(hg);
        Some(Secret(v))
    }
}

fn set_bytes(format: u32, bytes: &[u8]) -> Result<(), String> {
    // SAFETY: allocate a movable block of exact size, copy while locked, unlock, hand to
    // SetClipboardData (system owns it on success; we free it on failure).
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1))
            .map_err(|e| format!("GlobalAlloc:{:#x}", e.code().0))?;
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            let _ = GlobalFree(Some(h));
            return Err("GlobalLock".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
        if let Err(e) = SetClipboardData(format, Some(HANDLE(h.0))) {
            let _ = GlobalFree(Some(h));
            return Err(format!("SetClipboardData({format}):{:#x}", e.code().0));
        }
    }
    Ok(())
}

/// Result of [`snapshot`].
pub enum SnapOutcome {
    /// Snapshot taken; scenarios may proceed.
    Taken(Snapshot),
    /// Clipboard contains non-whitelisted formats (ids listed); nothing touched.
    NotRestorable(Vec<u32>),
    /// An error occurred while reading; nothing touched.
    Failed(String),
}

/// Takes a whitelist snapshot (reads the data only if every format is restorable).
pub fn snapshot(owner: Hwnd) -> SnapOutcome {
    let privacy = privacy_formats();
    let o = match open(owner) {
        Ok(o) => o,
        Err(e) => return SnapOutcome::Failed(e),
    };
    let formats = format_ids(&o);
    if !policy::clipboard_restorable(&formats, &privacy) {
        return SnapOutcome::NotRestorable(formats);
    }
    let unicode = if formats.contains(&cf::UNICODETEXT) {
        match read_bytes(&o, cf::UNICODETEXT) {
            Some(s) => Some(s),
            None => return SnapOutcome::Failed("read CF_UNICODETEXT".into()),
        }
    } else {
        None
    };
    let mut saved = Vec::new();
    for f in privacy {
        if formats.contains(&f) {
            match read_bytes(&o, f) {
                Some(s) => saved.push((f, s)),
                None => return SnapOutcome::Failed(format!("read privacy format {f}")),
            }
        }
    }
    SnapOutcome::Taken(Snapshot {
        formats,
        unicode,
        privacy: saved,
    })
}

fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn write_snapshot(_open: &Open, snap: &Snapshot) -> Result<(), String> {
    // SAFETY: clipboard is open by us (`_open`).
    unsafe { EmptyClipboard() }.map_err(|e| format!("EmptyClipboard:{:#x}", e.code().0))?;
    if let Some(u) = &snap.unicode {
        set_bytes(cf::UNICODETEXT, &u.0)?;
    }
    for (f, b) in &snap.privacy {
        set_bytes(*f, &b.0)?;
    }
    Ok(())
}

/// Writes our synthetic `text` with the three privacy markers; returns our sequence number
/// read **before** CloseClipboard (WS-01). If the write fails after the clipboard was
/// emptied, the snapshot is written back within the same open session.
pub fn put_private_test(owner: Hwnd, text: &str, snap: &Snapshot) -> Result<u32, String> {
    let privacy = privacy_formats();
    if privacy.contains(&0) {
        return Err("RegisterClipboardFormatW failed".into());
    }
    let o = open(owner)?;
    let write = || -> Result<(), String> {
        // SAFETY: clipboard is open by us; EmptyClipboard makes our window the owner.
        unsafe { EmptyClipboard() }.map_err(|e| format!("EmptyClipboard:{:#x}", e.code().0))?;
        set_bytes(cf::UNICODETEXT, &utf16z(text))?;
        for f in privacy {
            set_bytes(f, &0u32.to_le_bytes())?;
        }
        Ok(())
    };
    if let Err(e) = write() {
        let back = write_snapshot(&o, snap);
        return Err(format!("{e}; snapshot_rewritten={}", back.is_ok()));
    }
    let s = seq();
    drop(o);
    Ok(s)
}

/// Outcome of [`restore_if_unchanged`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restore {
    /// Snapshot put back.
    Restored,
    /// Someone wrote after us; their data is kept.
    SkippedSeqChanged,
}

/// Restores `snap` only if the sequence number, checked under OpenClipboard, still equals
/// `our_seq`. Returns the new sequence number on success, too.
pub fn restore_if_unchanged(owner: Hwnd, our_seq: u32, snap: &Snapshot) -> Result<Restore, String> {
    let o = open(owner)?;
    if seq() != our_seq {
        return Ok(Restore::SkippedSeqChanged);
    }
    write_snapshot(&o, snap)?;
    drop(o);
    Ok(Restore::Restored)
}

/// After a restore: same Unicode bytes and same privacy markers? (booleans only).
pub fn verify_restored(owner: Hwnd, snap: &Snapshot) -> Result<(bool, bool), String> {
    let o = open(owner)?;
    let text_ok = match (&snap.unicode, read_bytes(&o, cf::UNICODETEXT)) {
        (None, None) => true,
        // GlobalSize may round up; compare the saved prefix.
        (Some(a), Some(b)) => {
            b.0.len() >= a.0.len() && Secret(b.0[..a.0.len()].to_vec()).same_as(a)
        }
        _ => false,
    };
    let now = format_ids(&o);
    let privacy_ok = snap.privacy.iter().all(|(f, _)| now.contains(f))
        && privacy_formats()
            .iter()
            .all(|f| now.contains(f) == snap.privacy.iter().any(|(g, _)| g == f));
    Ok((text_ok, privacy_ok))
}
