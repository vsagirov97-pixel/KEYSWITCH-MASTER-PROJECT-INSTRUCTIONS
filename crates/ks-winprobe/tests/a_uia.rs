//! S-2a scenario a: UI Automation on the fixture (ControlType, IsPassword, RuntimeId,
//! TextPattern DocumentRange, GetSelection, select + read back + compare).
#![cfg(windows)]

mod common;

use std::sync::atomic::Ordering;
use std::time::Duration;

use ks_winprobe::fixture::{self, Fixture, Hwnd};
use ks_winprobe::report::Status;
use ks_winprobe::uia::{self, ComGuard, GET_TEXT_CALLS, Read, Uia};

const UIA_EDIT: i32 = 50004;
const UIA_DOCUMENT: i32 = 50030;

// Synthetic content only.
const SINGLE_TEXT: &str = "Ghbdtn, rfr ltkf?";
const MULTI_TEXT: &str = "Ghbdtn rfr\r\nltkf 123";
const RICH_TEXT: &str = "Ghbdtn rfr\r\nvbh 456";
const PASSWORD_TEXT: &str = "synthetic-pw-0000";
/// All three multi-line fixtures start with this ASCII line.
const FIRST_LINE: &str = "Ghbdtn rfr";

fn fill(fx: &Fixture) {
    let c = fx.controls;
    fixture::set_text(c.single, SINGLE_TEXT);
    fixture::set_text(c.multi, MULTI_TEXT);
    fixture::set_text(c.password, PASSWORD_TEXT);
    if let Some(rich) = c.rich {
        fixture::set_text(rich, RICH_TEXT);
    }
}

/// Normalises line breaks (RichEdit stores a bare CR).
fn norm(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn uia_client(r: &ks_winprobe::report::Report) -> (ComGuard, Uia) {
    let com = match ComGuard::mta() {
        Ok(c) => c,
        Err(e) => {
            common::finish(r, Status::Fail, &e);
            unreachable!()
        }
    };
    match Uia::new() {
        Ok(u) => (com, u),
        Err(e) => {
            common::finish(r, Status::Fail, &e);
            unreachable!()
        }
    }
}

/// a1: GetFocusedElement for every control; ControlType, IsPassword, RuntimeId.
#[test]
fn a1_focused_element_facts() {
    let id = "a1.uia.focused_element";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    if let Some(why) = env.desktop.unusable_reason() {
        return common::finish(&r, Status::Skip, why);
    }
    let fx = common::fixture(&r);
    fill(&fx);
    let (_com, uia) = uia_client(&r);
    if !common::foreground(&fx, &mut r).ok() {
        return common::finish(
            &r,
            Status::Skip,
            "fixture could not become the foreground window; system focus not ours",
        );
    }
    let calls_before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    let c = fx.controls;
    let mut controls: Vec<(&'static str, Hwnd, bool)> = vec![
        ("single", c.single, false),
        ("multi", c.multi, false),
        ("password", c.password, true),
    ];
    if let Some(rich) = c.rich {
        controls.push(("rich", rich, false));
    } else {
        r.field("rich", "not_created");
    }
    let mut problems: Vec<&'static str> = Vec::new();
    let mut ids: Vec<Vec<i32>> = Vec::new();
    for (label, h, expect_pw) in controls {
        if !fx.focus(h) {
            r.field(label, "set_focus_failed");
            problems.push("SetFocus did not move focus to a fixture control");
            continue;
        }
        let mut found = None;
        let waited = fixture::wait_until(Duration::from_secs(2), || match uia.focused() {
            Ok(el) => {
                let f = uia::facts(&el);
                if f.hwnd == Some(h) {
                    found = Some((el, f));
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        });
        let (Some(wait), Some((el, f))) = (waited, found) else {
            r.field(label, "uia_focus_not_observed_2s");
            problems.push("UIA focused element never matched the focused control");
            continue;
        };
        let rid_again = uia::runtime_id(&el).unwrap_or_default();
        r.field(
            label,
            format!(
                "ct={} pw={} class={} rid_len={} rid_stable={} text_pattern={} pid_is_self={} focus_ms={}",
                f.control_type,
                f.is_password,
                f.class_name,
                f.runtime_id.len(),
                rid_again == f.runtime_id,
                f.has_text_pattern,
                f.pid as u32 == std::process::id(),
                wait.as_millis()
            ),
        );
        if f.is_password != expect_pw {
            problems.push("IsPassword mismatch");
        }
        // Observed on windows-latest (run 42828479342): the UIA proxy reports multi-line
        // EDIT and RICHEDIT50W as Document, single-line and password EDIT as Edit.
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
        if expect_pw {
            // The guarded reader must refuse the focused password element without GetText.
            match uia.read_text(&el, 100) {
                Ok(Read::RefusedPassword) => {}
                _ => problems.push("guarded read did not refuse the focused password element"),
            }
        }
        ids.push(f.runtime_id);
    }
    let unique = ids
        .iter()
        .enumerate()
        .all(|(i, a)| ids[i + 1..].iter().all(|b| a != b));
    r.field("runtime_ids_unique", unique);
    if !unique {
        problems.push("RuntimeIds of different controls collide");
    }
    let calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - calls_before;
    r.field("get_text_calls", calls);
    if calls != 0 {
        problems.push("GetText was called during the facts scenario");
    }
    common::verdict(&r, &problems);
}

/// a2: the password edit is detected via UIA and `GetText` is never called on it.
#[test]
fn a2_password_never_read() {
    let id = "a2.uia.password_guard";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    fill(&fx);
    let (_com, uia) = uia_client(&r);
    let mut problems: Vec<&'static str> = Vec::new();

    let before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    match uia.from_hwnd(fx.controls.password) {
        Ok(el) => {
            let f = uia::facts(&el);
            r.field("password_is_password", f.is_password)
                .field("password_ct", f.control_type)
                .field("password_text_pattern", f.has_text_pattern);
            if !f.is_password {
                problems.push("ES_PASSWORD edit not reported as IsPassword");
            }
            match uia.read_text(&el, 100) {
                Ok(Read::RefusedPassword) => {
                    r.field("guarded_read", "refused_password");
                }
                Ok(_) => problems.push("guarded read did not refuse the password element"),
                Err(_) => problems.push("guarded read returned an error"),
            }
        }
        Err(_) => problems.push("ElementFromHandle(password) failed"),
    }
    let pw_calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - before;
    r.field("get_text_calls_on_password", pw_calls);
    if pw_calls != 0 {
        problems.push("GetText was called on the password element");
    }

    // Control: the counter does move for a normal edit, so a zero above is meaningful.
    let before = GET_TEXT_CALLS.load(Ordering::SeqCst);
    let ctrl_ok = uia
        .from_hwnd(fx.controls.single)
        .and_then(|el| uia.read_text(&el, 100))
        .is_ok_and(|rd| matches!(rd, Read::Text(_)));
    let ctrl_calls = GET_TEXT_CALLS.load(Ordering::SeqCst) - before;
    r.field("control_read_ok", ctrl_ok)
        .field("get_text_calls_on_control", ctrl_calls);
    if !ctrl_ok || ctrl_calls != 1 {
        problems.push("control read on the normal edit did not go through GetText once");
    }
    common::verdict(&r, &problems);
}

/// a3: TextPattern DocumentRange text equals what the fixture set; GetText(5001) bound.
#[test]
fn a3_document_range_text() {
    let id = "a3.uia.document_range";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    fill(&fx);
    let (_com, uia) = uia_client(&r);
    let mut problems: Vec<&'static str> = Vec::new();
    let c = fx.controls;
    let mut cases: Vec<(&'static str, Hwnd, &str)> = vec![
        ("single", c.single, SINGLE_TEXT),
        ("multi", c.multi, MULTI_TEXT),
    ];
    if let Some(rich) = c.rich {
        cases.push(("rich", rich, RICH_TEXT));
    }
    for (label, h, expected) in cases {
        let res = uia.from_hwnd(h).and_then(|el| uia.read_text(&el, 5001));
        match res {
            Ok(Read::Text(t)) => {
                let exact = t == expected;
                let normalized = norm(&t) == norm(expected);
                r.field(
                    label,
                    format!(
                        "len={} exact={exact} eq_after_newline_norm={normalized}",
                        t.encode_utf16().count()
                    ),
                );
                if !normalized {
                    problems.push("DocumentRange text differs from the fixture content");
                }
            }
            Ok(Read::NoTextPattern) => {
                r.field(label, "no_text_pattern");
                problems.push("control has no TextPattern");
            }
            Ok(Read::RefusedPassword) => problems.push("non-password control reported as password"),
            Err(_) => {
                r.field(label, "error");
                problems.push("DocumentRange read failed");
            }
        }
    }
    // Length bound (P-02): 6000 synthetic units, GetText(5001) must return exactly 5001.
    let long: String = "ab".repeat(3000);
    fixture::set_text(c.multi, &long);
    let res = uia
        .from_hwnd(c.multi)
        .and_then(|el| uia.read_text(&el, 5001));
    match res {
        Ok(Read::Text(t)) => {
            let n = t.encode_utf16().count();
            r.field("long_6000_get_text_5001_len", n);
            if n != 5001 {
                problems.push("GetText(5001) on 6000 units did not return 5001 units");
            }
        }
        _ => problems.push("long read failed"),
    }
    common::verdict(&r, &problems);
}

/// a4: GetSelection reflects EM_SETSEL; TextRange.Select() → EM_GETSEL and GetSelection agree.
#[test]
fn a4_selection_roundtrip() {
    let id = "a4.uia.selection_roundtrip";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    fill(&fx);
    let (_com, uia) = uia_client(&r);
    // Best effort: selection APIs should not need foreground; record whether we had it.
    let fg = if env.desktop.unusable_reason().is_none() {
        common::foreground(&fx, &mut r).ok()
    } else {
        r.field("foreground", "not_attempted");
        false
    };
    let mut problems: Vec<&'static str> = Vec::new();
    let c = fx.controls;
    let mut cases: Vec<(&'static str, Hwnd)> = vec![("single", c.single), ("multi", c.multi)];
    if let Some(rich) = c.rich {
        cases.push(("rich", rich));
    }
    for (label, h) in cases {
        if fg {
            let _ = fx.focus(h);
        }
        let Some((el, tp)) = uia
            .from_hwnd(h)
            .ok()
            .and_then(|el| uia::text_pattern(&el).map(|tp| (el, tp)))
        else {
            r.field(label, "no_text_pattern");
            problems.push("no TextPattern");
            continue;
        };
        let mut notes: Vec<String> = Vec::new();
        // 1. EM_SETSEL(2, 5) → UIA GetSelection text == chars 2..5 of the first line.
        fixture::set_sel(h, 2, 5);
        let sel = uia::selection(&tp).unwrap_or_default();
        // WS-03: the text is read only through the guarded selection reader.
        let sel_ok = sel.len() == 1
            && matches!(uia.read_selection(&el, 100), Ok(Some(Read::Text(t))) if t == FIRST_LINE[2..5]);
        notes.push(format!(
            "getsel_ranges={} getsel_matches_em_setsel={sel_ok}",
            sel.len()
        ));
        if !sel_ok {
            problems.push("GetSelection does not match EM_SETSEL");
        }
        // 2. Select the first 6 characters via UIA, read back via EM_GETSEL and GetSelection.
        match uia::prefix_range(&tp, 6) {
            Ok(pr) => {
                let selected = uia::select(&pr).is_ok();
                let em = fixture::get_sel(h);
                let back = uia::selection(&tp).unwrap_or_default();
                let cmp = back.len() == 1 && uia::same_range(&back[0], &pr);
                let txt = back.len() == 1
                    && matches!(uia.read_selection(&el, 100), Ok(Some(Read::Text(t))) if t == FIRST_LINE[..6]);
                notes.push(format!(
                    "select_ok={selected} em_getsel={}..{} compare={cmp} text_matches={txt}",
                    em.0, em.1
                ));
                if !(selected && em == (0, 6) && cmp && txt) {
                    problems.push("prefix Select() round trip mismatch");
                }
            }
            Err(_) => problems.push("prefix range construction failed"),
        }
        // 3. Whole-document range Select() → same EM_GETSEL as EM_SETSEL(0, -1).
        fixture::set_sel(h, 0, -1);
        let all = fixture::get_sel(h);
        fixture::set_sel(h, 0, 0);
        let doc_sel = uia::prefix_range(&tp, i32::MAX)
            .and_then(|d| uia::select(&d))
            .is_ok();
        let em = fixture::get_sel(h);
        notes.push(format!(
            "select_all_ok={doc_sel} em_getsel={}..{} em_select_all={}..{}",
            em.0, em.1, all.0, all.1
        ));
        // RichEdit keeps an implicit final paragraph mark: EM_SETSEL(0, -1) includes it, the
        // UIA DocumentRange does not (observed, run 42828479342). Both cover all the text.
        let rich_final_mark = label == "rich" && em.0 == 0 && em.1 + 1 == all.1;
        if rich_final_mark {
            notes.push("doc_range_excludes_final_paragraph_mark=true".into());
        }
        if !(doc_sel && (em == all || rich_final_mark) && all.1 > 0) {
            problems.push("select-all via TextRange does not cover the control");
        }
        r.field(label, notes.join(" "));
    }
    common::verdict(&r, &problems);
}
