//! S-2a scenario b: write method W1 (`EM_REPLACESEL`, fCanUndo = TRUE) and XR-03 (RTF in
//! RichEdit).
#![cfg(windows)]

mod common;

use ks_winprobe::fixture::{self, Hwnd};
use ks_winprobe::report::Status;
use windows::Win32::UI::Controls::{EM_CANUNDO, EM_EMPTYUNDOBUFFER, EM_UNDO};

// Synthetic content only.
const ORIGINAL: &str = "Ghbdtn vbh";
const REPLACED: &str = "Привет vbh";
const RTF: &str = r"{\rtf1 \b x}";

/// b1: replace a word in Edit (single and multi-line) and undo it with one EM_UNDO.
#[test]
fn b1_em_replacesel_one_step_undo() {
    let id = "b1.w1.em_replacesel_undo";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let mut problems: Vec<&'static str> = Vec::new();
    for (label, h) in [("single", fx.controls.single), ("multi", fx.controls.multi)] {
        fixture::set_text(h, ORIGINAL);
        fixture::send_simple(h, EM_EMPTYUNDOBUFFER);
        fixture::set_sel(h, 0, 6);
        fixture::replace_sel(h, "Привет", true);
        let replaced = fixture::get_text(h) == REPLACED;
        let caret = fixture::get_sel(h);
        let can_undo = fixture::send_simple(h, EM_CANUNDO) == Some(1);
        let undo_ret = fixture::send_simple(h, EM_UNDO).unwrap_or(0);
        let undone = fixture::get_text(h) == ORIGINAL;
        // Classic Edit has a single undo level; a second EM_UNDO redoes (recorded, no verdict).
        fixture::send_simple(h, EM_UNDO);
        let second_undo_redoes = fixture::get_text(h) == REPLACED;
        r.field(
            label,
            format!(
                "replaced={replaced} caret={}..{} can_undo={can_undo} undo_ret={undo_ret} \
                 one_step_undo_restores={undone} second_undo_redoes={second_undo_redoes}",
                caret.0, caret.1
            ),
        );
        if !replaced {
            problems.push("EM_REPLACESEL did not produce the expected text");
        }
        if !(can_undo && undone) {
            problems.push("one EM_UNDO did not restore the original text");
        }
    }
    // fCanUndo = FALSE leaves nothing to undo after an emptied buffer (recorded).
    let h = fx.controls.single;
    fixture::set_text(h, ORIGINAL);
    fixture::send_simple(h, EM_EMPTYUNDOBUFFER);
    fixture::set_sel(h, 0, 6);
    fixture::replace_sel(h, "Привет", false);
    r.field(
        "single_fcanundo_false_can_undo",
        fixture::send_simple(h, EM_CANUNDO) == Some(1),
    );
    common::verdict(&r, &problems);
}

fn rtf_outcome(h: Hwnd) -> String {
    let t = fixture::get_text(h);
    if t == RTF {
        "literal".into()
    } else if t.trim_end() == "x" {
        "parsed_as_rtf".into()
    } else {
        format!("other_len_{}", t.encode_utf16().count())
    }
}

/// b2 (XR-03): what RichEdit does with "{\rtf1 \b x}" via EM_REPLACESEL and WM_SETTEXT.
#[test]
fn b2_richedit_rtf_literal_vs_parsed() {
    let id = "b2.w1.richedit_rtf_xr03";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let fx = common::fixture(&r);
    let Some(h) = fx.controls.rich else {
        return common::finish(&r, Status::Skip, "Msftedit.dll / RICHEDIT50W not available");
    };
    fixture::set_text(h, "plain");
    fixture::send_simple(h, EM_EMPTYUNDOBUFFER);
    fixture::set_sel(h, 0, -1);
    fixture::replace_sel(h, RTF, true);
    let replacesel = rtf_outcome(h);
    let can_undo = fixture::send_simple(h, EM_CANUNDO) == Some(1);
    fixture::send_simple(h, EM_UNDO);
    let undo_restores = fixture::get_text(h) == "plain";

    // Ordinary text through EM_REPLACESEL into RichEdit (control for the RTF case).
    fixture::set_text(h, ORIGINAL);
    fixture::send_simple(h, EM_EMPTYUNDOBUFFER);
    fixture::set_sel(h, 0, 6);
    fixture::replace_sel(h, "Привет", true);
    let plain_ok = fixture::get_text(h) == REPLACED;
    fixture::send_simple(h, EM_UNDO);
    let plain_undo = fixture::get_text(h) == ORIGINAL;

    fixture::set_text(h, RTF);
    let settext = rtf_outcome(h);

    r.field("em_replacesel_rtf", replacesel)
        .field("em_replacesel_rtf_can_undo", can_undo)
        .field("em_replacesel_rtf_undo_restores", undo_restores)
        .field("wm_settext_rtf", settext)
        .field("plain_replace_ok", plain_ok)
        .field("plain_one_step_undo", plain_undo);
    // XR-03 is a fact-finding item: no expectation, the outcome decides W1 eligibility.
    common::finish(&r, Status::Observed, "XR-03 fact; see em_replacesel_rtf");
}
