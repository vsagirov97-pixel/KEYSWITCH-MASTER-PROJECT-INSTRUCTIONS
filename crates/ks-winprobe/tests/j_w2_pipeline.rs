//! Beta 1 scenarios j8–j12: the product pipeline with the **clipboard methods** (W2 paste
//! and Ctrl+C selection read, `ks_platform_win::w2`) against the fixture process, enabled
//! by a test profile row for the fixture executable.
//!
//! WARNING: replaces the clipboard of the machine it runs on. Enabled only with
//! `KS_WINPROBE=1` (CI runner); all content is synthetic; reports carry metadata only.
#![cfg(windows)]

mod common;

use std::time::Duration;

use ks_engine::{Config, Outcome, Refusal};
use ks_platform_win::adapter::WinPlatform;
use ks_platform_win::profiles::{Profile, ReadMode};
use ks_platform_win::w2::{ClipboardHold, HoldRestore, Settled};
use ks_winprobe::child::ChildFixture;
use ks_winprobe::clipboard;
use ks_winprobe::fixture::{self, Fixture, Hwnd};
use ks_winprobe::gate::Gate;
use ks_winprobe::report::{Report, Status};
use ks_winprobe::uia::{ComGuard, Uia};

const WRONG_LAYOUT: &str = "ghbdtn";
const CONVERTED: &str = "привет";
const USER: &str = "synthetic-user-clipboard";
const CF_UNICODETEXT: u32 = 13;

const fn profile(read: ReadMode, signal_confirmed: bool) -> Profile {
    Profile {
        exe: "ks-winprobe-fixture.exe",
        paste: true,
        read,
        focus_classes: &[],
        signal_confirmed,
        verified_on_real_app: false,
    }
}

/// One pipeline operation with an explicit profile table and clipboard hold.
fn run(r: &Report, profiles: &[Profile], hold: &ClipboardHold) -> (Outcome, Settled) {
    let fail = |e: &str| -> ! {
        common::finish(r, Status::Fail, e);
        unreachable!("finish panics on FAIL")
    };
    let com = ComGuard::mta().unwrap_or_else(|e| fail(&e));
    let uia = Uia::new().unwrap_or_else(|e| fail(&e));
    let converter = ks_core::builtin().expect("built-in layouts parse");
    let gate = Gate::new();
    let result = {
        let mut platform = WinPlatform::with_clipboard(&uia, &gate, profiles, hold.clone());
        let outcome = ks_engine::run(&mut platform, &converter, &Config::default());
        (outcome, platform.clipboard_settled())
    };
    drop(uia);
    drop(com);
    result
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

/// Puts the synthetic "user clipboard" (plus `extra` formats) through window `owner`.
fn put_user(owner: Hwnd, extra: &[(u32, Vec<u8>)]) -> bool {
    let mut items = vec![text_item(USER)];
    items.extend_from_slice(extra);
    clipboard::open(owner)
        .and_then(|o| clipboard::put_items(&o, &items))
        .is_ok()
}

fn clipboard_is(owner: Hwnd, expected: &str) -> bool {
    clipboard::open(owner)
        .ok()
        .and_then(|o| clipboard::read_text(&o))
        .as_deref()
        == Some(expected)
}

/// The fixture's RichEdit (not class `Edit`, so a profiled write goes through W2), filled,
/// selected and focused in the foreground child.
fn prepare(
    c: &ChildFixture,
    text: &str,
    sel: (i32, i32),
    r: &mut Report,
    env: &common::Env,
) -> Result<Hwnd, &'static str> {
    let k = c.controls.rich.ok_or("RICHEDIT50W is not available")?;
    if !fixture::set_text(k, text) {
        return Err("WM_SETTEXT failed");
    }
    common::child_target(c, k, r, env)?;
    fixture::set_sel(k, sel.0, sel.1);
    Ok(k)
}

fn field_becomes(k: Hwnd, expected: &str) -> bool {
    fixture::wait_until(Duration::from_secs(2), || fixture::get_text(k) == expected).is_some()
}

fn user_window(r: &Report) -> Fixture {
    common::fixture(r)
}

/// j8: W2 paste with a readable target — signal from the target and a positive post-check,
/// so the user's clipboard is restored and nothing is held.
#[test]
fn j8_w2_paste_verified_restores_clipboard() {
    let id = "j8.pipeline.w2_paste_restored";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let user = user_window(&r);
    let u = user.controls.main;
    let c = common::child(&r, "edit");
    if !put_user(u, &[]) {
        return common::finish(
            &r,
            Status::Fail,
            "could not set the synthetic user clipboard",
        );
    }
    let k = match prepare(&c, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let hold = ClipboardHold::new();
    let (o, settled) = run(&r, &[profile(ReadMode::Uia, false)], &hold);
    let arrived = field_becomes(k, CONVERTED);
    let user_back = clipboard_is(u, USER);
    r.field("outcome", o.code())
        .field("settled", format!("{settled:?}"))
        .field("text_arrived", arrived)
        .field("user_clipboard_back", user_back)
        .field("held", hold.is_held());
    let mut problems = Vec::new();
    if !matches!(o, Outcome::Converted { verified: true, .. }) {
        problems.push("outcome is not a verified CONVERTED");
    }
    if !arrived {
        problems.push("field does not hold the converted word");
    }
    if settled != Settled::Restored || !user_back {
        problems.push("user clipboard was not restored after signal + post-check");
    }
    if hold.is_held() {
        problems.push("a snapshot is held although the paste was confirmed");
    }
    let _ = put_user(u, &[]);
    common::verdict(&r, &problems);
}

/// j9: selection read via Ctrl+C, then W2 paste. The result cannot be read back and the
/// profile's signal is not confirmed, so nothing is restored blindly: the temporary text
/// stays with its privacy markers, the snapshot is held, and the explicit action restores it.
#[test]
fn j9_copy_read_unconfirmed_holds_then_explicit_restore() {
    let id = "j9.pipeline.w2_copy_read_hold";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let user = user_window(&r);
    let u = user.controls.main;
    let c = common::child(&r, "edit");
    if !put_user(u, &[]) {
        return common::finish(
            &r,
            Status::Fail,
            "could not set the synthetic user clipboard",
        );
    }
    let k = match prepare(&c, "abc ghbdtn xyz", (4, 10), &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let hold = ClipboardHold::new();
    let (o, settled) = run(&r, &[profile(ReadMode::Copy, false)], &hold);
    let arrived = field_becomes(k, "abc привет xyz");
    let temp_stays = clipboard_is(u, CONVERTED);
    let markers = clipboard::privacy_formats()
        .iter()
        .all(|&f| f != 0 && clipboard::available(f));
    let held = hold.is_held();
    let action = hold.restore();
    let user_back = clipboard_is(u, USER);
    r.field("outcome", o.code())
        .field("settled", format!("{settled:?}"))
        .field("text_arrived", arrived)
        .field("temp_text_stays", temp_stays)
        .field("temp_has_privacy_markers", markers)
        .field("held", held)
        .field("explicit_restore", format!("{action:?}"))
        .field("user_clipboard_back", user_back);
    let mut problems = Vec::new();
    if !matches!(
        o,
        Outcome::Converted {
            verified: false,
            layout_switched: false
        }
    ) {
        problems.push("outcome is not CONVERTED_UNVERIFIED without a layout switch");
    }
    if !arrived {
        problems.push("field is not prefix + converted selection + suffix");
    }
    if settled != Settled::Held || !held {
        problems.push("unconfirmed paste did not hold the snapshot");
    }
    if !temp_stays || !markers {
        problems.push("temporary text with privacy markers is not on the clipboard");
    }
    if action != HoldRestore::Restored || !user_back {
        problems.push("explicit restore did not bring the user clipboard back");
    }
    let _ = put_user(u, &[]);
    common::verdict(&r, &problems);
}

/// j10: Ctrl+C read is selection-only — with a caret and no selection nothing is converted
/// and neither the field nor the clipboard changes. A profile with a confirmed signal then
/// restores the clipboard on the signal alone.
#[test]
fn j10_copy_read_needs_a_selection_and_confirmed_signal_restores() {
    let id = "j10.pipeline.w2_copy_read_selection_only";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let user = user_window(&r);
    let u = user.controls.main;
    let c = common::child(&r, "edit");
    if !put_user(u, &[]) {
        return common::finish(
            &r,
            Status::Fail,
            "could not set the synthetic user clipboard",
        );
    }
    let mut problems = Vec::new();
    let hold = ClipboardHold::new();

    // (a) caret after the word, no selection.
    let k = match prepare(&c, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let seq_before = clipboard::seq();
    let (o, settled) = run(&r, &[profile(ReadMode::Copy, true)], &hold);
    let seq_same = clipboard::seq() == seq_before;
    r.field("no_selection_outcome", o.code())
        .field("no_selection_clipboard_seq_unchanged", seq_same);
    if o != Outcome::Nothing || settled != Settled::Nothing {
        problems.push("caret without selection: outcome is not NOTHING");
    }
    if fixture::get_text(k) != WRONG_LAYOUT {
        problems.push("caret without selection: field changed");
    }
    if !seq_same || !clipboard_is(u, USER) {
        problems.push("caret without selection: clipboard changed");
    }

    // (b) selection, profile with a confirmed signal.
    let k = match prepare(&c, WRONG_LAYOUT, (0, 6), &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let (o, settled) = run(&r, &[profile(ReadMode::Copy, true)], &hold);
    let arrived = field_becomes(k, CONVERTED);
    let user_back = clipboard_is(u, USER);
    r.field("selection_outcome", o.code())
        .field("selection_settled", format!("{settled:?}"))
        .field("text_arrived", arrived)
        .field("user_clipboard_back", user_back);
    if !matches!(o, Outcome::Converted { .. }) || !arrived {
        problems.push("selection: not converted");
    }
    if settled != Settled::Restored || !user_back || hold.is_held() {
        problems.push("selection: confirmed signal did not restore the user clipboard");
    }
    let _ = put_user(u, &[]);
    common::verdict(&r, &problems);
}

/// j11: a format outside the whitelist on the user's clipboard refuses W2 before anything
/// is touched — field, clipboard content and sequence number stay as they were.
#[test]
fn j11_foreign_clipboard_format_refuses_w2() {
    let id = "j11.pipeline.w2_foreign_format";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let user = user_window(&r);
    let u = user.controls.main;
    let c = common::child(&r, "edit");
    // SAFETY: registering a static NUL-terminated name.
    let private = unsafe {
        windows::Win32::System::DataExchange::RegisterClipboardFormatW(windows::core::w!(
            "KsWinProbePrivateFormat"
        ))
    };
    if !put_user(u, &[(private, vec![1, 2, 3, 4])]) {
        return common::finish(
            &r,
            Status::Fail,
            "could not set the synthetic user clipboard",
        );
    }
    let k = match prepare(&c, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    let seq_before = clipboard::seq();
    let hold = ClipboardHold::new();
    let (o, settled) = run(&r, &[profile(ReadMode::Uia, false)], &hold);
    let seq_same = clipboard::seq() == seq_before;
    // Give a wrongly sent Ctrl+V time to land before judging the field.
    std::thread::sleep(Duration::from_millis(300));
    r.field("outcome", o.code())
        .field("settled", format!("{settled:?}"))
        .field("clipboard_seq_unchanged", seq_same)
        .field("foreign_format_still_there", clipboard::available(private));
    let mut problems = Vec::new();
    if o != Outcome::Refused(Refusal::Clipboard) {
        problems.push("outcome is not CLIPBOARD_UNAVAILABLE");
    }
    if fixture::get_text(k) != WRONG_LAYOUT {
        problems.push("field changed although W2 was refused");
    }
    if !seq_same || !clipboard::available(private) || !clipboard_is(u, USER) {
        problems.push("clipboard was touched although W2 was refused");
    }
    let _ = put_user(u, &[]);
    common::verdict(&r, &problems);
}

/// j12: a clipboard listener renders the temporary text before Ctrl+V. W2 is aborted without
/// pasting, the user's clipboard is restored, and the short single-line text goes through W3.
#[test]
fn j12_listener_before_paste_aborts_w2_and_falls_back_to_w3() {
    let id = "j12.pipeline.w2_listener_interference";
    let Some(env) = common::start(id) else { return };
    let mut r = common::report(id, &env);
    let user = user_window(&r);
    let u = user.controls.main;
    let c = common::child(&r, "edit");
    if !put_user(u, &[]) {
        return common::finish(
            &r,
            Status::Fail,
            "could not set the synthetic user clipboard",
        );
    }
    let listener = common::child(&r, "listener");
    std::thread::sleep(Duration::from_millis(200));
    let k = match prepare(&c, WRONG_LAYOUT, (6, 6), &mut r, &env) {
        Ok(k) => k,
        Err(why) => return common::finish(&r, Status::Skip, why),
    };
    while listener.events.try_recv().is_ok() {}
    let hold = ClipboardHold::new();
    let (o, settled) = run(&r, &[profile(ReadMode::Uia, false)], &hold);
    let listener_read = listener
        .wait_event("listener_read", Duration::from_secs(2))
        .is_some();
    let arrived = field_becomes(k, CONVERTED);
    let user_back = clipboard_is(u, USER);
    r.field("outcome", o.code())
        .field("settled", format!("{settled:?}"))
        .field("listener_read_event", listener_read)
        .field("text_arrived", arrived)
        .field("user_clipboard_back", user_back)
        .field("held", hold.is_held());
    let mut problems = Vec::new();
    if !listener_read {
        problems.push("the listener fixture did not read the clipboard (scenario ineffective)");
    }
    // `Settled::Nothing` = no paste was ever pending, i.e. Ctrl+V was not sent by W2.
    if settled != Settled::Nothing {
        problems.push("W2 was not aborted before Ctrl+V");
    }
    if !matches!(o, Outcome::Converted { .. }) || !arrived {
        problems.push("W3 fallback did not convert the word");
    }
    if !user_back || hold.is_held() {
        problems.push("user clipboard not restored after the abort");
    }
    let _ = put_user(u, &[]);
    common::verdict(&r, &problems);
}
