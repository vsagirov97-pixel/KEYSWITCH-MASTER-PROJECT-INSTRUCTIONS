//! S-2b scenario h: clipboard contract for W2 (SYSTEM_ARCHITECTURE §4.1).
//! h1 P-08/WS-02 whitelist snapshot + restore with privacy markers; h2 WS-01 sequence number
//! under OpenClipboard vs. an interleaving writer; h3 XR-01(a) listener renders before
//! Ctrl+V; h4 XR-01(b) consumption signal after Ctrl+V attributed by pid; h5 XR-02 late
//! reader (no blind restore); h6 XR-04 hung clipboard owner (refusal ≤ 300 ms).
//!
//! WARNING: replaces the clipboard of the machine it runs on. Enabled only with
//! `KS_WINPROBE=1` (CI runner); all content is synthetic; reports carry metadata only.
#![cfg(windows)]

mod common;

use std::time::{Duration, Instant};

use ks_winprobe::child::{self, ChildFixture};
use ks_winprobe::clipboard::{self, DelayedOwner, Render, Restore, SnapRefusal, Snapshot};
use ks_winprobe::fixture::{self, Fixture, Hwnd};
use ks_winprobe::input;
use ks_winprobe::report::Status;
use windows::Win32::UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_CONTROL};

const USER: &str = "synthetic-user-clipboard";
const TEMP: &str = "synthetic-keyswitch-temp";
const OTHER: &str = "synthetic-other-writer";
const CF_UNICODETEXT: u32 = 13;
const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56);
/// "Quiet window" after the delayed put before Ctrl+V (XR-01 a).
const QUIET_MS: u64 = 400;
/// Deadline for the target's consumption signal after Ctrl+V.
const SIGNAL_MS: u64 = 500;

fn units(s: &str) -> i64 {
    s.encode_utf16().count() as i64
}

fn text_item(s: &str) -> (u32, Vec<u8>) {
    (
        CF_UNICODETEXT,
        s.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
}

fn markers() -> Vec<(u32, Vec<u8>)> {
    clipboard::privacy_formats()
        .into_iter()
        .map(|f| (f, 0u32.to_le_bytes().to_vec()))
        .collect()
}

fn put_items(owner: Hwnd, items: &[(u32, Vec<u8>)]) -> Result<u32, String> {
    let o = clipboard::open(owner)?;
    clipboard::put_items(&o, items)
}

fn snap(owner: Hwnd) -> Result<Snapshot, SnapRefusal> {
    let o = clipboard::open(owner).map_err(|_| SnapRefusal::OpenFailed)?;
    clipboard::snapshot(&o)
}

fn read(owner: Hwnd) -> Option<String> {
    let o = clipboard::open(owner).ok()?;
    clipboard::read_text(&o)
}

fn refusal_label(e: SnapRefusal) -> String {
    match e {
        SnapRefusal::ForeignFormat(f) => format!("foreign_format_{f}"),
        SnapRefusal::TooLarge => "too_large".into(),
        SnapRefusal::ReadFailed(f) => format!("read_failed_{f}"),
        SnapRefusal::OpenFailed => "open_failed".into(),
        SnapRefusal::OwnerUnresponsive => "owner_unresponsive".into(),
        SnapRefusal::Deadline => "deadline".into(),
    }
}

/// Expected sequence number for the restore guard: the value after our put or, if our owner
/// rendered on request, the value right after the last render (requester held it open).
fn expected_seq(put_seq: u32, renders: &[Render]) -> u32 {
    renders.last().map_or(put_seq, |r| r.seq_after)
}

fn renders_label(rs: &[Render]) -> String {
    rs.iter()
        .map(|r| {
            format!(
                "{}ms:pid={}:hwnd_null={}:ok={}",
                r.at_ms,
                r.requester_pid,
                r.requester_hwnd.is_null(),
                r.rendered
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// h1 P-08 / WS-02: whitelist snapshot keeps privacy markers and restores byte-for-byte;
/// foreign or oversized content refuses W2 before anything is written.
#[test]
fn h1_snapshot_whitelist_and_markers() {
    let id = "h1.w2.snapshot_whitelist_markers";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    let writer = match Fixture::spawn() {
        Ok(f) => f,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    let w = writer.controls.main;
    let mut problems: Vec<&'static str> = Vec::new();
    let [html, rtf] = clipboard::html_rtf_formats();

    // (i) "password manager": text + the three privacy markers.
    let mut pm = vec![text_item("synthetic-pm-secret")];
    pm.extend(markers());
    // (ii) rich content: text + HTML + RTF.
    let rich = vec![
        text_item("synthetic rich"),
        (
            html,
            b"Version:0.9\r\nStartHTML:0\r\n<b>synthetic</b>\0".to_vec(),
        ),
        (rtf, b"{\\rtf1 synthetic}\0".to_vec()),
    ];
    for (label, items) in [("password_manager", &pm), ("text_html_rtf", &rich)] {
        if put_items(w, items).is_err() {
            problems.push("synthetic writer failed");
            continue;
        }
        let s = match snap(owner) {
            Ok(s) => s,
            Err(e) => {
                r.field(label, format!("snapshot_refused={}", refusal_label(e)));
                problems.push("whitelisted content was refused");
                continue;
            }
        };
        let ours = match clipboard::open(owner)
            .and_then(|o| clipboard::put_text(&o, TEMP, true).map(|_| clipboard::commit(o)))
        {
            Ok(q) => q,
            Err(_) => {
                problems.push("temporary put failed");
                continue;
            }
        };
        let restored = clipboard::restore_snapshot(owner, owner, ours, &s);
        let after = snap(owner);
        let all_items_equal = after.as_ref().is_ok_and(|a| s.subset_of(a));
        let markers_back = clipboard::privacy_formats()
            .iter()
            .all(|&f| !s.format_ids().contains(&f) || clipboard::available(f));
        r.field(
            label,
            format!(
                "formats={} bytes={} restored={} items_byte_equal={all_items_equal} markers_restored={markers_back}",
                s.format_ids().len(),
                s.total_bytes(),
                matches!(restored, Ok(Restore::Restored))
            ),
        );
        if !matches!(restored, Ok(Restore::Restored)) || !all_items_equal || !markers_back {
            problems.push("restore did not reproduce the snapshot with its markers");
        }
        if label == "password_manager" {
            let had = clipboard::privacy_formats()
                .iter()
                .all(|f| s.format_ids().contains(f));
            r.field("pm_snapshot_has_all_markers", had);
            if !had {
                problems.push("snapshot dropped privacy markers");
            }
        }
    }

    // (iii) a foreign (private registered) format → refusal, nothing written.
    // SAFETY: registering a static NUL-terminated name.
    let private = unsafe {
        windows::Win32::System::DataExchange::RegisterClipboardFormatW(windows::core::w!(
            "KsWinProbePrivateFormat"
        ))
    };
    let _ = put_items(w, &[text_item("synthetic"), (private, vec![1, 2, 3, 4])]);
    let seq0 = clipboard::seq();
    let res = snap(owner);
    let unchanged = clipboard::seq() == seq0;
    r.field(
        "foreign_format",
        format!(
            "refusal={} seq_unchanged={unchanged}",
            res.as_ref()
                .err()
                .map_or("none".into(), |e| refusal_label(*e))
        ),
    );
    if !matches!(res, Err(SnapRefusal::ForeignFormat(f)) if f == private) || !unchanged {
        problems.push("foreign format did not refuse W2 cleanly");
    }

    // (iv) > 16 MB of text → TooLarge.
    let big = "x".repeat(8 * 1024 * 1024 + 16); // 16 MB + 32 bytes in UTF-16
    let _ = put_items(w, &[text_item(&big)]);
    let res = snap(owner);
    r.field(
        "oversized",
        res.as_ref()
            .err()
            .map_or("none".into(), |e| refusal_label(*e)),
    );
    if !matches!(res, Err(SnapRefusal::TooLarge)) {
        problems.push("16 MB limit not enforced");
    }
    let _ = put_items(w, &[text_item(USER)]);
    common::verdict(&r, &problems);
}

/// h2 WS-01: a writer slips in right after our CloseClipboard. The sequence number alone
/// (which can only be read after the close) is fooled whenever the writer is faster than
/// our read (`seq_only_guard_fooled`); the owner check under OpenClipboard still refuses
/// every restore.
#[test]
fn h2_restore_guard_vs_interleaving_writer() {
    let id = "h2.w2.restore_guard_ws01";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    let racer = match Fixture::spawn() {
        Ok(f) => f,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    let rh = racer.controls.main;
    let n = 20;
    let (mut skipped, mut by_owner, mut kept, mut fooled) = (0, 0, 0, 0);
    for i in 0..n {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let t = std::thread::spawn(move || {
            if rx.recv().is_ok()
                && let Ok(o) = clipboard::open_for(Some(rh), Duration::from_secs(1))
            {
                let _ = clipboard::put_text(&o, OTHER, false);
            }
        });
        let o = match clipboard::open(owner) {
            Ok(o) => o,
            Err(e) => return common::finish(&r, Status::Fail, &e),
        };
        let _ = clipboard::put_text(&o, TEMP, true);
        let _ = tx.send(());
        drop(o);
        if i % 2 == 1 {
            // Simulated preemption between our CloseClipboard and our read of the number.
            std::thread::sleep(Duration::from_millis(5));
        }
        let ours = clipboard::seq();
        let _ = t.join();
        if ours == clipboard::seq() {
            // The racer had already written when we read "our" number.
            fooled += 1;
        }
        match clipboard::restore_if_unchanged(owner, owner, ours, USER) {
            Ok(Restore::SkippedOwnerChanged) => {
                skipped += 1;
                by_owner += 1;
            }
            Ok(Restore::SkippedSeqChanged) => skipped += 1,
            _ => {}
        }
        if read(owner).as_deref() == Some(OTHER) {
            kept += 1;
        }
    }
    r.field("iterations", n)
        .field("restore_skipped", skipped)
        .field("skipped_by_owner_check", by_owner)
        .field("other_data_kept", kept)
        .field("seq_only_guard_fooled", fooled);
    let _ = put_items(owner, &[text_item(USER)]);
    let problems: Vec<&'static str> = if skipped == n && kept == n {
        Vec::new()
    } else {
        vec!["restore overwrote an interleaving writer"]
    };
    common::verdict(&r, &problems);
}

/// h3 XR-01(a): a hostile listener forces rendering of our delayed text **before** Ctrl+V;
/// the render is attributed to the listener's pid → W2 aborted, Ctrl+V never sent,
/// user clipboard restored.
#[test]
fn h3_listener_renders_before_paste() {
    let id = "h3.w2.xr01a_listener_before_paste";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    let dl = match DelayedOwner::spawn() {
        Ok(d) => d,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    let mut problems: Vec<&'static str> = Vec::new();
    let _ = put_items(owner, &[text_item(USER)]);

    // Control: no listener of ours. Anything rendering now is a listener of the runner.
    let user = match snap(owner) {
        Ok(s) => s,
        Err(e) => return common::finish(&r, Status::Fail, &refusal_label(e)),
    };
    let put0 = dl.put_delayed(TEMP, true).unwrap_or(0);
    std::thread::sleep(Duration::from_millis(QUIET_MS));
    let ctrl = dl.renders();
    r.field("renders_without_our_listener", renders_label(&ctrl));
    let _ = clipboard::restore_snapshot(owner, dl.hwnd, expected_seq(put0, &ctrl), &user);

    let lst = common::child(&r, "listener");
    std::thread::sleep(Duration::from_millis(200));
    while lst.events.try_recv().is_ok() {} // updates caused by the restore above
    let put1 = match dl.put_delayed(TEMP, true) {
        Ok(s) => s,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    let t0 = Instant::now();
    let mut decision = "paste";
    while t0.elapsed() < Duration::from_millis(QUIET_MS) {
        // No paste target yet: any render before Ctrl+V is interference.
        if !dl.renders().is_empty() {
            decision = "abort_listener_interference";
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let rs = dl.renders();
    let by_listener = rs.iter().any(|x| x.requester_pid == lst.pid);
    let ev = lst.wait_event("listener_read", Duration::from_secs(2));
    let listener_units = ev
        .as_ref()
        .and_then(|(l, _)| child::event_field(l, "units"))
        .unwrap_or(-1);
    let restored = if decision != "paste" {
        clipboard::restore_snapshot(owner, dl.hwnd, expected_seq(put1, &rs), &user)
    } else {
        Ok(Restore::SkippedSeqChanged)
    };
    let user_back = read(owner).as_deref() == Some(USER);
    r.field("renders", renders_label(&rs))
        .field("render_attributed_to_listener_pid", by_listener)
        .field(
            "seq_delta_on_render",
            rs.last()
                .map_or(-1, |x| i64::from(x.seq_after.wrapping_sub(put1))),
        )
        .field("decision", decision)
        .field("ctrl_v_sent", false)
        .field("listener_read_units", listener_units)
        .field("temp_units", units(TEMP))
        .field("restore", format!("{restored:?}"))
        .field("user_clipboard_back", user_back);
    if decision == "paste" || !by_listener {
        problems.push("render by the listener before Ctrl+V was not detected/attributed");
    }
    if !matches!(restored, Ok(Restore::Restored)) || !user_back {
        problems.push("user clipboard not restored after the abort");
    }
    common::verdict(&r, &problems);
}

/// h4 XR-01(b): after Ctrl+V the consumption signal (WM_RENDERFORMAT) comes from the target
/// process (requester window of the target pid); post-check, then restore.
#[test]
fn h4_signal_after_paste_attributed_by_pid() {
    let id = "h4.w2.xr01b_signal_by_target_pid";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    let tgt = common::child(&r, "edit");
    let h = tgt.controls.single;
    let dl = match DelayedOwner::spawn() {
        Ok(d) => d,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    let _ = put_items(owner, &[text_item(USER)]);
    let user = match snap(owner) {
        Ok(s) => s,
        Err(e) => return common::finish(&r, Status::Fail, &refusal_label(e)),
    };
    fixture::set_text(h, "");
    if let Err(why) = common::child_target(&tgt, h, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let mut problems: Vec<&'static str> = Vec::new();
    let put = match dl.put_delayed(TEMP, true) {
        Ok(s) => s,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    std::thread::sleep(Duration::from_millis(QUIET_MS));
    let early = dl.renders();
    r.field("renders_before_paste", renders_label(&early));
    if !early.is_empty() {
        let _ = clipboard::restore_snapshot(owner, dl.hwnd, expected_seq(put, &early), &user);
        return common::finish(
            &r,
            Status::Skip,
            "a runner listener rendered before Ctrl+V (XR-01a path); target signal not evaluable",
        );
    }
    let injected = input::send_vk_chord(&[VK_CONTROL, VK_V]);
    let t0 = Instant::now();
    let signal = fixture::wait_until(Duration::from_millis(SIGNAL_MS), || {
        dl.renders().iter().any(|x| x.requester_pid == tgt.pid)
    });
    let rs = dl.renders();
    let from_target = rs.iter().find(|x| x.requester_pid == tgt.pid).copied();
    let arrived = fixture::wait_until(Duration::from_secs(2), || fixture::get_text(h) == TEMP);
    let restored = if signal.is_some() && arrived.is_some() {
        clipboard::restore_snapshot(owner, dl.hwnd, expected_seq(put, &rs), &user)
    } else {
        Ok(Restore::SkippedSeqChanged)
    };
    let user_back = read(owner).as_deref() == Some(USER);
    r.field("ctrl_v_events", injected)
        .field("signal_ms", common::ms(signal))
        .field("renders", renders_label(&rs))
        .field(
            "requester_is_target_edit",
            from_target.is_some_and(|x| x.requester_hwnd == h),
        )
        .field(
            "seq_delta_on_render",
            rs.last()
                .map_or(-1, |x| i64::from(x.seq_after.wrapping_sub(put))),
        )
        .field("post_check_text_arrived_ms", common::ms(arrived))
        .field("restore", format!("{restored:?}"))
        .field("user_clipboard_back", user_back)
        .field("total_ms", t0.elapsed().as_millis());
    if injected == 0 {
        return common::finish(&r, Status::Skip, "SendInput injected 0 events");
    }
    if signal.is_none() {
        problems.push("no WM_RENDERFORMAT from the target pid within 500 ms");
    }
    if arrived.is_none() {
        problems.push("post-check failed: pasted text not in the target");
    }
    if !matches!(restored, Ok(Restore::Restored)) || !user_back {
        problems.push("restore after a verified signal failed");
    }
    common::verdict(&r, &problems);
}

/// h5 XR-02: the target reads the clipboard only 1500 ms after Ctrl+V. No signal within the
/// deadline → no restore; the late read then still gets our temporary text (a blind restore
/// at the deadline would have pasted the user's clipboard instead).
#[test]
fn h5_late_reader_no_blind_restore() {
    let id = "h5.w2.xr02_late_reader";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    let tgt: ChildFixture = common::child(&r, "late");
    let dl = match DelayedOwner::spawn() {
        Ok(d) => d,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    let _ = put_items(owner, &[text_item(USER)]);
    let user = match snap(owner) {
        Ok(s) => s,
        Err(e) => return common::finish(&r, Status::Fail, &refusal_label(e)),
    };
    if let Err(why) = common::child_target(&tgt, tgt.controls.main, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let mut problems: Vec<&'static str> = Vec::new();
    let put = match dl.put_delayed(TEMP, true) {
        Ok(s) => s,
        Err(e) => return common::finish(&r, Status::Fail, &e),
    };
    std::thread::sleep(Duration::from_millis(QUIET_MS));
    let early = dl.renders();
    r.field("renders_before_paste", renders_label(&early));
    let injected = input::send_vk_chord(&[VK_CONTROL, VK_V]);
    let signal = fixture::wait_until(Duration::from_millis(SIGNAL_MS), || {
        dl.renders().iter().any(|x| x.requester_pid == tgt.pid)
    });
    let decision = if signal.is_some() {
        "restore_after_signal"
    } else {
        "no_signal_keep_temp"
    };
    let ev = tgt.wait_event("late_read", Duration::from_secs(3));
    let late_units = ev
        .as_ref()
        .and_then(|(l, _)| child::event_field(l, "units"))
        .unwrap_or(-1);
    let rs = dl.renders();
    let late_render = rs.iter().find(|x| x.requester_pid == tgt.pid).copied();
    r.field("ctrl_v_events", injected)
        .field("decision_at_deadline", decision)
        .field("late_read_units", late_units)
        .field("temp_units", units(TEMP))
        .field("user_units", units(USER))
        .field("late_render_ms", late_render.map_or(-1, |x| x.at_ms as i64))
        .field("renders", renders_label(&rs));
    if injected == 0 {
        return common::finish(&r, Status::Skip, "SendInput injected 0 events");
    }
    if signal.is_some() {
        problems.push("late reader produced a signal within the deadline (fixture ineffective)");
    }
    if late_units != units(TEMP) {
        problems.push("late reader did not receive the temporary text");
    }
    // Clean-up = the user's explicit "restore clipboard" action (contract §4.1 p.5).
    let _ = clipboard::restore_snapshot(owner, dl.hwnd, expected_seq(put, &rs), &user);
    common::verdict(&r, &problems);
}

/// h6 XR-04: hung clipboard owner. (a) whole owner thread hung → WM_NULL ping fails → refuse
/// without opening the clipboard; (b) owner hangs inside WM_RENDERFORMAT → deadline refusal.
/// Both within 300 ms, the caller never blocks.
#[test]
fn h6_hung_clipboard_owner() {
    let id = "h6.w2.xr04_hung_owner";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let owner = fx.controls.main;
    let ho = common::child(&r, "hung-owner");
    let mut problems: Vec<&'static str> = Vec::new();
    let deadline = Duration::from_millis(250);

    // (a) owner thread hung.
    if fixture::request_own_delayed(ho.controls.main).is_none() {
        return common::finish(&r, Status::Fail, "child could not own the clipboard");
    }
    fixture::post_hang(ho.controls.main, 2500);
    std::thread::sleep(Duration::from_millis(50));
    let t = Instant::now();
    let (res_a, worker_a) = clipboard::snapshot_guarded(deadline);
    let took_a = t.elapsed();
    r.field(
        "thread_hung",
        format!(
            "refusal={} ms={} worker_started={}",
            res_a
                .as_ref()
                .err()
                .map_or("none".into(), |e| refusal_label(*e)),
            took_a.as_millis(),
            worker_a.is_some()
        ),
    );
    if !matches!(res_a, Err(SnapRefusal::OwnerUnresponsive)) || took_a > Duration::from_millis(300)
    {
        problems.push("hung owner thread not refused within 300 ms");
    }
    if let Some(w) = worker_a {
        let _ = w.join();
    }
    let _ = fixture::wait_until(Duration::from_secs(5), || {
        fixture::ping(ho.controls.main, 200)
    });

    // (b) owner responsive, hangs inside WM_RENDERFORMAT.
    if fixture::request_own_delayed(ho.controls.main).is_none() {
        return common::finish(&r, Status::Fail, "child could not own the clipboard (b)");
    }
    let t = Instant::now();
    let (res_b, worker_b) = clipboard::snapshot_guarded(deadline);
    let took_b = t.elapsed();
    // While the worker is blocked, can anyone else open the clipboard?
    let others_can_open = clipboard::open_for(Some(owner), Duration::from_millis(100)).is_ok();
    let worker_ms = worker_b
        .and_then(|w| w.join().ok())
        .map_or(-1, |d| d.as_millis() as i64);
    r.field(
        "render_hung",
        format!(
            "refusal={} ms={} others_could_open_during_block={others_can_open} worker_blocked_ms={worker_ms}",
            res_b.as_ref().err().map_or("none".into(), |e| refusal_label(*e)),
            took_b.as_millis()
        ),
    );
    if !matches!(res_b, Err(SnapRefusal::Deadline)) || took_b > Duration::from_millis(300) {
        problems.push("owner hung in WM_RENDERFORMAT not refused within 300 ms");
    }
    let _ = put_items(owner, &[text_item(USER)]);
    common::verdict(&r, &problems);
}
