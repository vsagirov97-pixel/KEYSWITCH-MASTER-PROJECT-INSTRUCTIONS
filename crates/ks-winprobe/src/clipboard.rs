//! Clipboard contract basics (SYSTEM_ARCHITECTURE §4.1): snapshot without reading,
//! privacy formats (D-13), sequence number and restore-only-if-unchanged.

use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::core::w;

use crate::fixture::Hwnd;

/// Open clipboard; closed on drop.
#[derive(Debug)]
pub struct Open(());

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: this guard exists only while we hold the clipboard open.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Opens the clipboard with `owner`, retrying for up to ~1 s (another process may hold it).
pub fn open(owner: Hwnd) -> Result<Open, String> {
    for _ in 0..50 {
        // SAFETY: `owner` is a window of this process; success is paired with CloseClipboard.
        if unsafe { OpenClipboard(Some(owner.raw())) }.is_ok() {
            return Ok(Open(()));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err("OpenClipboard failed for 1 s".into())
}

/// Current clipboard sequence number (0 = no WINSTA_ACCESSCLIPBOARD).
pub fn seq() -> u32 {
    // SAFETY: trivial query.
    unsafe { GetClipboardSequenceNumber() }
}

/// Format ids currently on the clipboard — enumerated, no data is read.
pub fn format_ids(_open: &Open) -> Vec<u32> {
    let mut out = Vec::new();
    let mut f = 0u32;
    loop {
        // SAFETY: clipboard is open (`_open`); EnumClipboardFormats only lists ids.
        f = unsafe { EnumClipboardFormats(f) };
        if f == 0 {
            break;
        }
        out.push(f);
    }
    out
}

/// The three D-13 privacy formats, in order: Exclude…MonitorProcessing,
/// CanIncludeInClipboardHistory, CanUploadToCloudClipboard. `0` = registration failed.
pub fn privacy_formats() -> [u32; 3] {
    // SAFETY: registering well-known format names (static NUL-terminated strings).
    unsafe {
        [
            RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")),
            RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
            RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
        ]
    }
}

/// `true` if `format` is available (no data read).
pub fn available(format: u32) -> bool {
    // SAFETY: trivial query.
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

fn set_bytes(format: u32, bytes: &[u8]) -> Result<(), String> {
    // SAFETY: we allocate a movable global block of the exact size, copy `bytes` into it
    // while locked, unlock, and hand it to SetClipboardData. On success the system owns the
    // block; on failure we free it ourselves.
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|e| format!("GlobalAlloc: {e}"))?;
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            let _ = GlobalFree(Some(h));
            return Err("GlobalLock failed".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
        if let Err(e) = SetClipboardData(format, Some(HANDLE(h.0))) {
            let _ = GlobalFree(Some(h));
            return Err(format!("SetClipboardData({format}): {e}"));
        }
    }
    Ok(())
}

fn text_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Replaces the clipboard with `text` (CF_UNICODETEXT) and, if `private`, the three
/// privacy formats (DWORD 0 each).
pub fn put_text(open: &Open, text: &str, private: bool) -> Result<(), String> {
    let _ = open;
    // SAFETY: clipboard is open by us (`open`); EmptyClipboard makes our window the owner.
    unsafe { EmptyClipboard() }.map_err(|e| format!("EmptyClipboard: {e}"))?;
    set_bytes(u32::from(CF_UNICODETEXT.0), &text_bytes(text))?;
    if private {
        for f in privacy_formats() {
            if f == 0 {
                return Err("RegisterClipboardFormatW failed".into());
            }
            set_bytes(f, &0u32.to_le_bytes())?;
        }
    }
    Ok(())
}

/// Reads CF_UNICODETEXT. Test-only, used on synthetic content to verify restore.
pub fn read_text(_open: &Open) -> Option<String> {
    // SAFETY: clipboard is open; the handle is owned by the system, we only lock it for
    // reading, bound the read by GlobalSize and stop at the first NUL, then unlock.
    unsafe {
        let h = GetClipboardData(u32::from(CF_UNICODETEXT.0)).ok()?;
        let hg = HGLOBAL(h.0);
        let size = GlobalSize(hg) / 2;
        let p = GlobalLock(hg) as *const u16;
        if p.is_null() {
            return None;
        }
        let units = std::slice::from_raw_parts(p, size);
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        let s = String::from_utf16_lossy(&units[..end]);
        let _ = GlobalUnlock(hg);
        Some(s)
    }
}

/// Outcome of [`restore_if_unchanged`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restore {
    /// Sequence number matched our write; the snapshot was put back.
    Restored,
    /// Someone wrote after us; restore skipped (their data is kept).
    SkippedSeqChanged,
}

/// Restores `snapshot` only if the clipboard sequence number (checked **under**
/// `OpenClipboard`) still equals `our_seq`, i.e. nobody wrote after our temporary data.
pub fn restore_if_unchanged(owner: Hwnd, our_seq: u32, snapshot: &str) -> Result<Restore, String> {
    let o = open(owner)?;
    if seq() != our_seq {
        return Ok(Restore::SkippedSeqChanged);
    }
    put_text(&o, snapshot, false)?;
    Ok(Restore::Restored)
}
