//! S-2a scenario d: clipboard basics (format enumeration without reading, D-13 privacy
//! formats, sequence number, restore only if nobody wrote after us).
//!
//! WARNING: replaces the clipboard of the machine it runs on. Enabled only with
//! `KS_WINPROBE=1` (CI runner); all content is synthetic.
#![cfg(windows)]

mod common;

use std::process::Command;

use ks_winprobe::clipboard::{self, Restore};
use ks_winprobe::fixture::{Fixture, Hwnd};
use ks_winprobe::report::{Report, Status};

const SNAPSHOT: &str = "synthetic-user-clipboard";
const TEMP: &str = "synthetic-keyswitch-temp";
const OTHER: &str = "synthetic-other-writer";
const OTHER_PROCESS: &str = "synthetic-other-process";
const CF_UNICODETEXT: u32 = 13;

fn put(owner: Hwnd, text: &str, private: bool) -> Result<u32, String> {
    {
        let o = clipboard::open(owner)?;
        clipboard::put_text(&o, text, private)?;
    }
    Ok(clipboard::seq())
}

fn read(owner: Hwnd) -> Option<String> {
    let o = clipboard::open(owner).ok()?;
    clipboard::read_text(&o)
}

fn fail(r: &Report, e: &str) {
    common::finish(r, Status::Fail, e);
}

/// `Some(reason)` if the clipboard cannot be used here. A sequence number of 0 alone is not
/// a reason: a fresh session that never wrote to the clipboard also reports 0 (observed on
/// windows-latest, run 42828963511). So we write once and check the number afterwards.
fn unusable(r: &mut Report, owner: Hwnd) -> Option<&'static str> {
    r.field("seq_initial", clipboard::seq());
    if clipboard::open(owner).is_err() {
        return Some("OpenClipboard failed for 1 s");
    }
    match put(owner, TEMP, false) {
        Err(_) => Some("probe write to the clipboard failed"),
        Ok(0) => Some("sequence number still 0 after a write (no WINSTA_ACCESSCLIPBOARD)"),
        Ok(_) => None,
    }
}

/// d1: our temporary item carries the three privacy formats; enumeration reads no data
/// and does not change the sequence number; writing does.
#[test]
fn d1_privacy_formats_and_enumeration() {
    let id = "d1.clipboard.privacy_formats";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    if let Some(why) = unusable(&mut r, owner) {
        return common::finish(&r, Status::Skip, why);
    }
    let seq0 = clipboard::seq();
    let seq1 = match put(owner, TEMP, true) {
        Ok(s) => s,
        Err(e) => return fail(&r, &e),
    };
    let ids = match clipboard::open(owner) {
        Ok(o) => clipboard::format_ids(&o),
        Err(e) => return fail(&r, &e),
    };
    let seq2 = clipboard::seq();
    let [exclude, history, cloud] = clipboard::privacy_formats();
    let has = |f: u32| f != 0 && ids.contains(&f) && clipboard::available(f);
    let mut problems: Vec<&'static str> = Vec::new();
    r.field("format_count", ids.len())
        .field("has_unicode_text", ids.contains(&CF_UNICODETEXT))
        .field("has_exclude_monitor", has(exclude))
        .field("has_can_include_in_history", has(history))
        .field("has_can_upload_to_cloud", has(cloud))
        .field("seq_delta_after_write", seq1.wrapping_sub(seq0))
        .field("seq_delta_after_enumeration", seq2.wrapping_sub(seq1));
    if !ids.contains(&CF_UNICODETEXT) {
        problems.push("CF_UNICODETEXT missing");
    }
    if !(has(exclude) && has(history) && has(cloud)) {
        problems.push("a D-13 privacy format is missing");
    }
    if seq1 == seq0 {
        problems.push("sequence number did not change after our write");
    }
    if seq2 != seq1 {
        problems.push("enumeration changed the sequence number");
    }
    common::verdict(&r, &problems);
}

/// d2: restore happens when nobody wrote after us, and is skipped when another writer
/// (another window in this process, then another process) changed the clipboard.
#[test]
fn d2_restore_only_if_unchanged() {
    let id = "d2.clipboard.restore_guard";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    if let Some(why) = unusable(&mut r, owner) {
        return common::finish(&r, Status::Skip, why);
    }
    let mut problems: Vec<&'static str> = Vec::new();

    // Positive control: nobody interferes → restored.
    let res = put(owner, SNAPSHOT, false)
        .and_then(|_| put(owner, TEMP, true))
        .and_then(|our| clipboard::restore_if_unchanged(owner, our, SNAPSHOT));
    let restored_ok =
        matches!(res, Ok(Restore::Restored)) && read(owner).as_deref() == Some(SNAPSHOT);
    r.field("undisturbed_restored", restored_ok);
    if !restored_ok {
        problems.push("undisturbed restore did not put the snapshot back");
    }

    // Second writer: another window (own thread) in this process.
    let other = match Fixture::spawn() {
        Ok(f) => f,
        Err(e) => return fail(&r, &e),
    };
    let res = put(owner, SNAPSHOT, false)
        .and_then(|_| put(owner, TEMP, true))
        .and_then(|our| {
            put(other.controls.main, OTHER, false)?;
            clipboard::restore_if_unchanged(owner, our, SNAPSHOT)
        });
    let skipped = matches!(res, Ok(Restore::SkippedSeqChanged));
    let kept = read(owner).as_deref() == Some(OTHER);
    r.field("other_window_restore_skipped", skipped)
        .field("other_window_data_kept", kept);
    if !(skipped && kept) {
        problems.push("restore overwrote data of a second writer (other window)");
    }

    // Second writer: another process (PowerShell Set-Clipboard).
    let mut ps_ran = false;
    let res = put(owner, SNAPSHOT, false)
        .and_then(|_| put(owner, TEMP, true))
        .and_then(|our| {
            let st = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command"])
                .arg(format!("Set-Clipboard -Value '{OTHER_PROCESS}'"))
                .status()
                .map_err(|e| format!("spawn powershell: {e}"))?;
            ps_ran = st.success();
            clipboard::restore_if_unchanged(owner, our, SNAPSHOT)
        });
    if ps_ran {
        let skipped = matches!(res, Ok(Restore::SkippedSeqChanged));
        let kept = read(owner).as_deref() == Some(OTHER_PROCESS);
        r.field("other_process_restore_skipped", skipped)
            .field("other_process_data_kept", kept);
        if !(skipped && kept) {
            problems.push("restore overwrote data of a second writer (other process)");
        }
    } else {
        r.field("other_process", "not_evaluated_powershell_failed");
    }
    common::verdict(&r, &problems);
}
