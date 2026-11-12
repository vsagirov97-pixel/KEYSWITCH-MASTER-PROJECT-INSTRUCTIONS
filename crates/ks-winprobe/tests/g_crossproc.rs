//! S-2b scenario g: the S-2a checks a1–a4, b1, c1 repeated against a fixture in a
//! **separate process** (same integrity level), plus D-30 W1 class gate and P-16 hung
//! target (WS-04). Synthetic strings only; reports carry metadata.
#![cfg(windows)]

mod common;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use ks_winprobe::child::ChildFixture;
use ks_winprobe::fixture::{self, Hwnd};
use ks_winprobe::gate::{self, Gate};
use ks_winprobe::input::{self, SendError};
use ks_winprobe::policy::W1Refusal;
use ks_winprobe::report::{Report, Status};
use ks_winprobe::uia::{self, ComGuard, GET_TEXT_CALLS, PasswordBy, Read, Uia};
use windows::Win32::UI::Controls::{EM_CANUNDO, EM_EMPTYUNDOBUFFER, EM_UNDO};
use windows::Win32::UI::WindowsAndMessaging::IsHungAppWindow;

const UIA_EDIT: i32 = 50004;
const UIA_DOCUMENT: i32 = 50030;
const SINGLE_TEXT: &str = "Ghbdtn, rfr ltkf?";
const MULTI_TEXT: &str = "Ghbdtn rfr\r\nltkf 123";
const RICH_TEXT: &str = "Ghbdtn rfr\r\nvbh 456";
const PASSWORD_TEXT: &str = "synthetic-pw-0000";
const FIRST_LINE: &str = "Ghbdtn rfr";

fn fill(c: &ChildFixture) -> bool {
    let k = c.controls;
    let mut ok = fixture::set_text(k.single, SINGLE_TEXT)
        & fixture::set_text(k.multi, MULTI_TEXT)
        & fixture::set_text(k.password, PASSWORD_TEXT);
    if let Some(rich) = k.rich {
        ok &= fixture::set_text(rich, RICH_TEXT);
    }
    ok
}

fn norm(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn uia_client(r: &Report) -> (ComGuard, Uia) {
    let com = ComGuard::mta().unwrap_or_else(|e| {
        common::finish(r, Status::Fail, &e);
        unreachable!()
    });
    let u = Uia::new().unwrap_or_else(|e| {
        common::finish(r, Status::Fail, &e);
        unreachable!()
    });
    (com, u)
}

/// g1 (= a1 across processes): focused element facts; pid is the child's, not ours.
#[test]
fn g1_cross_focused_element() {
    let id = "g1.cross.uia_focused_element";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let filled = fill(&c);
    r.field("wm_settext_cross_process_ok", filled);
    let (_com, u) = uia_client(&r);
    if let Err(why) = common::child_target(&c, c.controls.single, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let calls_before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    let k = c.controls;
    let mut cases: Vec<(&'static str, Hwnd, bool)> = vec![
        ("single", k.single, false),
        ("multi", k.multi, false),
        ("password", k.password, true),
    ];
    if let Some(rich) = k.rich {
        cases.push(("rich", rich, false));
    }
    let mut problems: Vec<&'static str> = Vec::new();
    let mut ids: Vec<Vec<i32>> = Vec::new();
    for (label, h, expect_pw) in cases {
        if !c.focus(h) {
            r.field(label, "set_focus_failed");
            problems.push("SetFocus in the child did not move focus");
            continue;
        }
        let mut found = None;
        let waited = fixture::wait_until(Duration::from_secs(2), || match u.focused() {
            Ok(el) => {
                let f = uia::facts(&el);
                let hit = f.hwnd == Some(h);
                if hit {
                    found = Some((el, f));
                }
                hit
            }
            Err(_) => false,
        });
        let (Some(wait), Some((el, f))) = (waited, found) else {
            r.field(label, "uia_focus_not_observed_2s");
            problems.push("UIA focused element never matched the child control");
            continue;
        };
        let rid_again = uia::runtime_id(&el).unwrap_or_default();
        let pid_ok = f.pid as u32 == c.pid;
        r.field(
            label,
            format!(
                "ct={} pw={} class={} rid_len={} rid_stable={} text_pattern={} pid_is_child={pid_ok} focus_ms={}",
                f.control_type,
                f.is_password,
                f.class_name,
                f.runtime_id.len(),
                rid_again == f.runtime_id,
                f.has_text_pattern,
                wait.as_millis()
            ),
        );
        if !pid_ok {
            problems.push("UIA ProcessId is not the child's pid");
        }
        if f.is_password != expect_pw {
            problems.push("IsPassword mismatch");
        }
        let ct_ok = if label == "multi" || label == "rich" {
            f.control_type == UIA_EDIT || f.control_type == UIA_DOCUMENT
        } else {
            f.control_type == UIA_EDIT
        };
        if !ct_ok {
            problems.push("unexpected ControlType");
        }
        if f.runtime_id.is_empty() || rid_again != f.runtime_id {
            problems.push("RuntimeId empty or unstable");
        }
        if expect_pw && !matches!(u.read_text(&el, 100), Ok(Read::RefusedPassword)) {
            problems.push("guarded read did not refuse the focused password element");
        }
        ids.push(f.runtime_id);
    }
    let unique = ids
        .iter()
        .enumerate()
        .all(|(i, a)| ids[i + 1..].iter().all(|b| a != b));
    let calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - calls_before;
    r.field("runtime_ids_unique", unique)
        .field("get_text_calls", calls);
    if !unique {
        problems.push("RuntimeIds collide");
    }
    if calls != 0 {
        problems.push("GetText was called during the facts scenario");
    }
    common::verdict(&r, &problems);
}

/// g2 (= a2 across processes, WS-03): password refused by IsPassword and, independently, by
/// `ES_PASSWORD` in `GWL_STYLE` read across the process boundary; GetText never called.
#[test]
fn g2_cross_password_guard() {
    let id = "g2.cross.password_guard";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    fill(&c);
    let (_com, u) = uia_client(&r);
    let mut problems: Vec<&'static str> = Vec::new();
    let k = c.controls;
    let style_pw = gate::hwnd_is_password(k.password);
    let style_single = gate::hwnd_is_password(k.single);
    r.field("style_es_password_on_password", style_pw)
        .field("style_es_password_on_single", style_single);
    if !style_pw || style_single {
        problems.push("GWL_STYLE ES_PASSWORD check wrong across processes");
    }
    let before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    match u.from_hwnd(k.password) {
        Ok(el) => {
            let f = uia::facts(&el);
            let guard = uia::password_guard(&el);
            r.field("password_is_password", f.is_password)
                .field("password_text_pattern", f.has_text_pattern)
                .field(
                    "guard",
                    match guard {
                        Some(PasswordBy::UiaIsPassword) => "uia_is_password",
                        Some(PasswordBy::EsPasswordStyle) => "es_password_style",
                        None => "none",
                    },
                );
            if guard.is_none() {
                problems.push("password guard did not trigger");
            }
            if !matches!(u.read_text(&el, 100), Ok(Read::RefusedPassword)) {
                problems.push("guarded read did not refuse the password element");
            }
            if !matches!(u.read_selection(&el, 100), Ok(Some(Read::RefusedPassword))) {
                problems.push("guarded selection read did not refuse the password element");
            }
        }
        Err(_) => problems.push("ElementFromHandle(password) failed"),
    }
    let pw_calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - before;
    r.field("get_text_calls_on_password", pw_calls);
    if pw_calls != 0 {
        problems.push("GetText was called on the password element");
    }
    let before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    let ctrl_ok = u
        .from_hwnd(k.single)
        .and_then(|el| u.read_text(&el, 100))
        .is_ok_and(|rd| matches!(rd, Read::Text(t) if t == SINGLE_TEXT));
    let ctrl_calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - before;
    r.field("control_read_ok", ctrl_ok)
        .field("get_text_calls_on_control", ctrl_calls);
    if !ctrl_ok || ctrl_calls != 1 {
        problems.push("control read on the normal edit failed");
    }
    common::verdict(&r, &problems);
}

/// g3 (= a3 across processes): DocumentRange text equals what was set via WM_SETTEXT.
#[test]
fn g3_cross_document_range() {
    let id = "g3.cross.document_range";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    fill(&c);
    let (_com, u) = uia_client(&r);
    let mut problems: Vec<&'static str> = Vec::new();
    let k = c.controls;
    let mut cases: Vec<(&'static str, Hwnd, &str)> = vec![
        ("single", k.single, SINGLE_TEXT),
        ("multi", k.multi, MULTI_TEXT),
    ];
    if let Some(rich) = k.rich {
        cases.push(("rich", rich, RICH_TEXT));
    }
    for (label, h, expected) in cases {
        let t0 = Instant::now();
        match u.from_hwnd(h).and_then(|el| u.read_text(&el, 5001)) {
            Ok(Read::Text(t)) => {
                let normalized = norm(&t) == norm(expected);
                r.field(
                    label,
                    format!(
                        "len={} exact={} eq_after_newline_norm={normalized} read_ms={}",
                        t.encode_utf16().count(),
                        t == expected,
                        t0.elapsed().as_millis()
                    ),
                );
                if !normalized {
                    problems.push("DocumentRange text differs from the child content");
                }
            }
            _ => {
                r.field(label, "read_failed");
                problems.push("DocumentRange read failed");
            }
        }
    }
    let long: String = "ab".repeat(3000);
    fixture::set_text(k.multi, &long);
    match u.from_hwnd(k.multi).and_then(|el| u.read_text(&el, 5001)) {
        Ok(Read::Text(t)) => {
            let n = t.encode_utf16().count();
            r.field("long_6000_get_text_5001_len", n);
            if n != 5001 {
                problems.push("GetText(5001) bound not respected across processes");
            }
        }
        _ => problems.push("long read failed"),
    }
    common::verdict(&r, &problems);
}

/// g4 (= a4 across processes): GetSelection reflects EM_SETSEL; TextRange.Select() →
/// EM_GETSEL agrees.
#[test]
fn g4_cross_selection_roundtrip() {
    let id = "g4.cross.selection_roundtrip";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    fill(&c);
    let (_com, u) = uia_client(&r);
    let fg = common::child_target(&c, c.controls.single, &mut r, &env).is_ok();
    r.field("had_foreground", fg);
    let mut problems: Vec<&'static str> = Vec::new();
    let k = c.controls;
    let mut cases = vec![("single", k.single), ("multi", k.multi)];
    if let Some(rich) = k.rich {
        cases.push(("rich", rich));
    }
    for (label, h) in cases {
        if fg {
            let _ = c.focus(h);
        }
        let Some((el, tp)) = u
            .from_hwnd(h)
            .ok()
            .and_then(|el| uia::text_pattern(&el).map(|tp| (el, tp)))
        else {
            problems.push("no TextPattern");
            continue;
        };
        fixture::set_sel(h, 2, 5);
        let sel_ok =
            matches!(u.read_selection(&el, 100), Ok(Some(Read::Text(t))) if t == FIRST_LINE[2..5]);
        let mut notes = format!("getsel_matches_em_setsel={sel_ok}");
        if !sel_ok {
            problems.push("GetSelection does not match EM_SETSEL");
        }
        match uia::prefix_range(&tp, 6) {
            Ok(pr) => {
                let selected = uia::select(&pr).is_ok();
                let em = fixture::get_sel(h);
                let back = uia::selection(&tp).unwrap_or_default();
                let cmp = back.len() == 1 && uia::same_range(&back[0], &pr);
                notes.push_str(&format!(
                    " select_ok={selected} em_getsel={}..{} compare={cmp}",
                    em.0, em.1
                ));
                if !(selected && em == (0, 6) && cmp) {
                    problems.push("prefix Select() round trip mismatch");
                }
            }
            Err(_) => problems.push("prefix range construction failed"),
        }
        r.field(label, notes);
    }
    common::verdict(&r, &problems);
}

/// g5 (= b1 across processes) + D-30: W1 class gate (GetClassNameW + RealGetWindowClassW,
/// no ES_PASSWORD / ES_READONLY), then EM_REPLACESEL + one-step undo in the child.
#[test]
fn g5_cross_w1_class_gate_and_undo() {
    let id = "g5.cross.w1_class_gate_undo";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let k = c.controls;
    let mut problems: Vec<&'static str> = Vec::new();
    let label = |x: Result<(), W1Refusal>| match x {
        Ok(()) => "permitted",
        Err(W1Refusal::NotEditClass) => "refused_not_edit_class",
        Err(W1Refusal::Password) => "refused_password",
        Err(W1Refusal::ReadOnly) => "refused_readonly",
    };
    let single = gate::w1_permitted(k.single);
    let multi = gate::w1_permitted(k.multi);
    let pw = gate::w1_permitted(k.password);
    r.field("w1_single", label(single))
        .field("w1_multi", label(multi))
        .field("w1_password", label(pw));
    if single.is_err() || multi.is_err() || pw != Err(W1Refusal::Password) {
        problems.push("D-30 W1 gate wrong for Edit controls");
    }
    if let Some(rich) = k.rich {
        let rg = gate::w1_permitted(rich);
        let (cls, real) = gate::class_names(rich);
        r.field("w1_rich", label(rg))
            .field("rich_class", cls)
            .field("rich_real_class", real);
        if rg != Err(W1Refusal::NotEditClass) {
            problems.push("D-30: W1 not refused for RichEdit");
        }
    }
    for (lbl, h) in [("single", k.single), ("multi", k.multi)] {
        fixture::set_text(h, "Ghbdtn vbh");
        fixture::send_simple(h, EM_EMPTYUNDOBUFFER);
        fixture::set_sel(h, 0, 6);
        let sent = fixture::replace_sel(h, "Привет", true);
        let replaced = fixture::get_text(h) == "Привет vbh";
        let caret = fixture::get_sel(h);
        let can_undo = fixture::send_simple(h, EM_CANUNDO) == Some(1);
        fixture::send_simple(h, EM_UNDO);
        let undone = fixture::get_text(h) == "Ghbdtn vbh";
        r.field(
            lbl,
            format!(
                "sent={sent} replaced={replaced} caret={}..{} can_undo={can_undo} one_step_undo_restores={undone}",
                caret.0, caret.1
            ),
        );
        if !(sent && replaced && can_undo && undone) {
            problems.push("cross-process EM_REPLACESEL / undo failed");
        }
    }
    common::verdict(&r, &problems);
}

/// g6 (= c1 across processes, with the WS-05 gate before every batch): ASCII + Cyrillic and
/// a surrogate pair arrive intact in the child's edit.
#[test]
fn g6_cross_w3_gated() {
    let id = "g6.cross.w3_sendinput_gated";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let h = c.controls.single;
    if let Err(why) = common::child_target(&c, h, &mut r, &env) {
        return common::finish(&r, Status::Skip, why);
    }
    let mut problems: Vec<&'static str> = Vec::new();
    let g = Gate::new();
    for (label, text) in [
        ("ascii_cyrillic", "Hello Привет"),
        ("surrogate_pair", "a\u{1F600}b"),
    ] {
        fixture::set_text(h, "");
        let fp = match gate::capture() {
            Ok(fp) => fp,
            Err(_) => {
                problems.push("fingerprint capture failed");
                continue;
            }
        };
        let fp_ok = fp.focus == h && fp.pid == c.pid && fp.start_time != 0;
        let not_self = gate::classify(&fp).is_ok();
        let tok = g.begin();
        let mut abort = None;
        let res = input::send_unicode(text, || match gate::pre_batch(&fp, &g, tok) {
            Ok(()) => true,
            Err(a) => {
                abort = Some(a);
                false
            }
        });
        let arrived = fixture::wait_until(Duration::from_secs(3), || fixture::get_text(h) == text);
        r.field(
            label,
            format!(
                "fingerprint_matches_child={fp_ok} not_self={not_self} sent={} abort={} arrived_ms={}",
                res.is_ok(),
                abort.map_or("none", |a| a.label()),
                common::ms(arrived)
            ),
        );
        match res {
            Err(SendError::Blocked { injected: 0, .. }) => {
                return common::finish(&r, Status::Skip, "SendInput injected 0 events");
            }
            Err(_) => problems.push("gated W3 send did not complete"),
            Ok(_) => {}
        }
        if !fp_ok || !not_self {
            problems.push("fingerprint does not describe the child target");
        }
        if arrived.is_none() {
            problems.push("text did not arrive intact in the child within 3 s");
        }
    }
    common::verdict(&r, &problems);
}

/// g7 P-16 / WS-04: the child's GUI thread hangs; `SendMessageTimeoutW(SMTO_ABORTIFHUNG)`
/// returns within the timeout and the caller is not blocked. Long hang: after > 5 s the
/// window is reported hung and SMTO_ABORTIFHUNG returns without waiting (recorded).
#[test]
fn g7_hung_target_bounded() {
    let id = "g7.cross.hung_target_p16";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let c = common::child(&r, "edit");
    let h = c.controls.single;
    let main = c.controls.main;
    let mut problems: Vec<&'static str> = Vec::new();
    // Responsive baseline.
    let t = Instant::now();
    let base = fixture::text_len_timeout(h, 300).is_some();
    r.field("baseline_ok", base)
        .field("baseline_ms", t.elapsed().as_millis());
    // Short hang (2.5 s) with a 300 ms timeout.
    if !fixture::post_hang(main, 2500) {
        return common::finish(&r, Status::Fail, "PostMessage(hang) failed");
    }
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    let res = fixture::text_len_timeout(h, 300);
    let took = t.elapsed();
    // SAFETY: trivial query on a window handle of the child (stale handle → FALSE).
    let hung_short = unsafe { IsHungAppWindow(main.raw()) }.as_bool();
    r.field(
        "short_hang_result",
        if res.is_some() { "answered" } else { "timeout" },
    )
    .field("short_hang_call_ms", took.as_millis())
    .field("short_hang_is_hung_app_window", hung_short);
    if res.is_some() {
        problems.push("hung target answered (hang not effective)");
    }
    if took > Duration::from_millis(400) {
        problems.push("SendMessageTimeoutW blocked well beyond its 300 ms timeout");
    }
    let recovered = fixture::wait_until(Duration::from_secs(5), || fixture::ping(main, 200));
    r.field("recovered_ms", common::ms(recovered));
    // Long hang: 6.5 s; probe after 5.6 s.
    fixture::post_hang(main, 6500);
    std::thread::sleep(Duration::from_millis(5600));
    // SAFETY: as above.
    let hung_long = unsafe { IsHungAppWindow(main.raw()) }.as_bool();
    let t = Instant::now();
    let res = fixture::text_len_timeout(h, 2000);
    let took_long = t.elapsed();
    r.field("long_hang_is_hung_app_window", hung_long)
        .field(
            "long_hang_result",
            if res.is_some() {
                "answered"
            } else {
                "timeout_or_abort"
            },
        )
        .field("long_hang_call_ms", took_long.as_millis());
    if res.is_some() {
        problems.push("hung target answered during the long hang");
    }
    if took_long > Duration::from_millis(2100) {
        problems.push("SendMessageTimeoutW exceeded its 2000 ms timeout");
    }
    let _ = fixture::wait_until(Duration::from_secs(5), || fixture::ping(main, 200));
    common::verdict(&r, &problems);
}
