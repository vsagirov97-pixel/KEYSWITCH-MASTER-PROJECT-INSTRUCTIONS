//! Spike M-2a probe runner (custom harness: runs on the process main thread).
//!
//! Prints one `PROBE {json}` line per scenario and a summary; exits non-zero only on FAIL.
//! SKIP always carries a reason. Field text is never printed (lengths/booleans only).
#![allow(clippy::print_stdout)] // the probe's report is its stdout

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("ks-probe-mac: not macOS, probe not applicable on this host");
}

#[cfg(target_os = "macos")]
fn main() {
    std::process::exit(mac::run());
}

#[cfg(target_os = "macos")]
mod mac {
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc::{Receiver, channel};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use ks_probe_mac::ax::{self, Element, FieldClass, attr};
    use ks_probe_mac::carbon::{self, mods};
    use ks_probe_mac::events::{self, Delivery, vk};
    use ks_probe_mac::pasteboard::{self, Board, RestoreOutcome, SnapshotError};
    use ks_probe_mac::report::{Outcome, Status};
    use ks_probe_mac::synthetic as syn;
    use objc2_core_graphics::CGEventFlags;

    const FIXTURE: &str = env!("CARGO_BIN_EXE_ks-mac-fixture");
    const AX_TIMEOUT_S: f32 = 2.0;

    // ---------------------------------------------------------------- fixture process

    struct Fixture {
        child: Child,
        pid: i32,
        lines: Receiver<String>,
        backlog: Vec<String>,
    }

    impl Fixture {
        fn spawn(args: &[&str]) -> Result<Fixture, String> {
            let mut child = Command::new(FIXTURE)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("spawn: {:?}", e.kind()))?;
            let pid = child.id() as i32;
            let out = child.stdout.take().ok_or("no stdout")?;
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            let mut fx = Fixture {
                child,
                pid,
                lines: rx,
                backlog: Vec::new(),
            };
            if fx.wait_line("READY", Duration::from_secs(20)).is_none() {
                return Err("fixture did not print READY within 20 s".into());
            }
            Ok(fx)
        }

        /// Waits for a control line starting with `prefix`; returns the remainder.
        fn wait_line(&mut self, prefix: &str, timeout: Duration) -> Option<String> {
            if let Some(i) = self.backlog.iter().position(|l| l.starts_with(prefix)) {
                let l = self.backlog.remove(i);
                return Some(l[prefix.len()..].trim().to_owned());
            }
            let end = Instant::now() + timeout;
            loop {
                let left = end.checked_duration_since(Instant::now())?;
                match self.lines.recv_timeout(left) {
                    Ok(l) if l.starts_with(prefix) => {
                        return Some(l[prefix.len()..].trim().to_owned());
                    }
                    Ok(l) => self.backlog.push(l),
                    Err(_) => return None,
                }
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            sleep(Duration::from_millis(300));
        }
    }

    // ---------------------------------------------------------------- helpers

    struct Env {
        ax_trusted: bool,
        can_post: bool,
    }

    struct Focus {
        el: Element,
        via_system_wide: bool,
        frontmost: bool,
        wait_ms: u128,
    }

    /// Finds the fixture's focused element; prefers the system-wide path.
    fn focus_of(fx: &Fixture) -> Result<Focus, String> {
        let app = Element::application(fx.pid);
        app.set_timeout(AX_TIMEOUT_S);
        let sys = Element::system_wide();
        sys.set_timeout(AX_TIMEOUT_S);
        let t0 = Instant::now();
        let mut raised = false;
        while t0.elapsed() < Duration::from_secs(10) {
            if !raised {
                // Ask AX to make the fixture frontmost (needed for HID-delivered events).
                app.set_bool(attr::FRONTMOST, true);
                raised = true;
            }
            if let Ok(el) = sys.element_attr(attr::FOCUSED_UI_ELEMENT)
                && el.pid() == Ok(fx.pid)
            {
                let frontmost = app.bool_attr(attr::FRONTMOST).unwrap_or(false);
                return Ok(Focus {
                    el,
                    via_system_wide: true,
                    frontmost,
                    wait_ms: t0.elapsed().as_millis(),
                });
            }
            sleep(Duration::from_millis(100));
        }
        let frontmost = app.bool_attr(attr::FRONTMOST).unwrap_or(false);
        app.element_attr(attr::FOCUSED_UI_ELEMENT)
            .map(|el| Focus {
                el,
                via_system_wide: false,
                frontmost,
                wait_ms: t0.elapsed().as_millis(),
            })
            .map_err(|e| format!("no focused element (AX {e})"))
    }

    fn role_of(el: &Element) -> (String, Option<String>) {
        let role = el
            .meta_attr(attr::ROLE)
            .unwrap_or_else(|e| format!("err{e}"));
        let sub = el.meta_attr(attr::SUBROLE).ok();
        (role, sub)
    }

    fn class_of(el: &Element) -> FieldClass {
        let (r, s) = role_of(el);
        ax::classify(&r, s.as_deref())
    }

    /// Polls `AXValue` until it equals `expected` or the timeout elapses.
    fn wait_value(el: &Element, expected: &str, timeout: Duration) -> (bool, u128, usize) {
        let t0 = Instant::now();
        let mut last_len = 0;
        loop {
            if let Ok(v) = el.text_attr(attr::VALUE) {
                last_len = v.utf16_len();
                if v.matches(expected) {
                    return (true, t0.elapsed().as_millis(), last_len);
                }
            }
            if t0.elapsed() > timeout {
                return (false, t0.elapsed().as_millis(), last_len);
            }
            sleep(Duration::from_millis(20));
        }
    }

    fn sh(cmd: &str, args: &[&str]) -> String {
        Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_else(|| "unavailable".into())
    }

    fn utf16_prefix(s: &str, n: usize) -> String {
        String::from_utf16_lossy(&s.encode_utf16().take(n).collect::<Vec<_>>())
    }

    fn ax_skip(o: &mut Outcome, env: &Env) -> bool {
        if !env.ax_trusted {
            o.skip("AXIsProcessTrusted=false on this runner; AX scenario not executed");
            return true;
        }
        false
    }

    // ---------------------------------------------------------------- scenarios

    fn m_p00(env: &Env) -> Outcome {
        let mut o = Outcome::new("M-P00");
        o.set_status(Status::Fact);
        o.meta("os_version", &sh("sw_vers", &["-productVersion"]));
        o.meta("os_build", &sh("sw_vers", &["-buildVersion"]));
        o.meta("arch", std::env::consts::ARCH);
        let sip = sh("csrutil", &["status"]);
        let sip = if sip.contains("enabled") {
            "enabled"
        } else if sip.contains("disabled") {
            "disabled"
        } else {
            "unknown"
        };
        o.meta("sip", sip);
        o.flag("ax_trusted", env.ax_trusted);
        o.flag("post_event_preflight", env.can_post);
        o.flag(
            "listen_event_preflight_diag_only",
            events::listen_access_preflight(),
        );
        o.flag("secure_event_input", carbon::secure_event_input_enabled());
        o
    }

    fn m_p01(env: &Env) -> Vec<Outcome> {
        let cases: [(&str, &str, FieldClass); 3] = [
            ("field", "AXTextField", FieldClass::Text),
            ("view", "AXTextArea", FieldClass::Text),
            ("secure", "AXTextField", FieldClass::Secure),
        ];
        let mut out = Vec::new();
        for (focus, want_role, want_class) in cases {
            let mut o = Outcome::new("M-P01");
            o.meta("fixture", focus);
            if ax_skip(&mut o, env) {
                out.push(o);
                continue;
            }
            let fx = match Fixture::spawn(&["--focus", focus]) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            match focus_of(&fx) {
                Ok(f) => {
                    let (role, sub) = role_of(&f.el);
                    let class = ax::classify(&role, sub.as_deref());
                    o.meta("role", &role)
                        .meta("subrole", sub.as_deref().unwrap_or("-"));
                    o.flag("via_system_wide", f.via_system_wide)
                        .flag("frontmost", f.frontmost);
                    o.num("focus_wait_ms", f.wait_ms);
                    o.meta("class", &format!("{class:?}"));
                    if class == FieldClass::Secure {
                        // Policy: refuse before any value access. Only the global flag is read.
                        o.flag("refused_secure", true);
                        o.flag(
                            "secure_event_input_while_focused",
                            carbon::secure_event_input_enabled(),
                        );
                    }
                    if role != want_role || class != want_class {
                        o.fail("role/class mismatch");
                    } else if !f.via_system_wide {
                        o.fail("system-wide AXFocusedUIElement did not resolve to the fixture");
                    }
                }
                Err(e) => {
                    o.fail(&e);
                }
            }
            out.push(o);
        }
        out
    }

    fn m_p02(env: &Env) -> Vec<Outcome> {
        let mut out = Vec::new();
        for (focus, expected, long) in [
            ("field", syn::FIELD_TEXT, false),
            ("view", syn::VIEW_TEXT, false),
            ("view", "", true),
        ] {
            let mut o = Outcome::new("M-P02");
            o.meta("fixture", if long { "view-long" } else { focus });
            if ax_skip(&mut o, env) {
                out.push(o);
                continue;
            }
            let args: Vec<&str> = if long {
                vec!["--focus", focus, "--long"]
            } else {
                vec!["--focus", focus]
            };
            let fx = match Fixture::spawn(&args) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            let f = match focus_of(&fx) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            if class_of(&f.el) != FieldClass::Text {
                o.fail("focused element is not a plain text element");
                out.push(o);
                continue;
            }
            // Limit is checked *before* reading the value.
            let n = f.el.number_attr(attr::NUMBER_OF_CHARACTERS);
            match n {
                Ok(n) => o.num("ax_number_of_characters", n),
                Err(e) => o.num("ax_number_of_characters_err", e),
            };
            let too_long = matches!(n, Ok(n) if n as usize > syn::READ_LIMIT);
            if long {
                o.flag("refused_too_long", too_long);
                if !too_long {
                    o.fail("over-limit field was not refused before reading");
                }
            } else if too_long {
                o.fail("unexpected TooLong");
            } else {
                let t0 = Instant::now();
                match f.el.text_attr(attr::VALUE) {
                    Ok(v) => {
                        o.num("read_us", t0.elapsed().as_micros());
                        o.num("utf16_len", v.utf16_len());
                        let ok = v.matches(expected);
                        o.flag("value_matches", ok);
                        if !ok {
                            o.fail("AXValue differs from fixture text");
                        }
                    }
                    Err(e) => {
                        o.num("ax_err", e);
                        o.fail("AXValue read failed");
                    }
                }
            }
            out.push(o);
        }
        out
    }

    fn m_p03(env: &Env) -> Vec<Outcome> {
        let view_line1 = "Synthetic line one\n".encode_utf16().count() as isize;
        let view_line2 = "Ghbdtn, rfr ltkf?";
        let mut out = Vec::new();
        for (focus, loc, len, expected) in [
            ("field", 0isize, 6isize, utf16_prefix(syn::FIELD_TEXT, 6)),
            (
                "view",
                view_line1,
                view_line2.encode_utf16().count() as isize,
                view_line2.to_owned(),
            ),
        ] {
            let mut o = Outcome::new("M-P03");
            o.meta("fixture", focus);
            if ax_skip(&mut o, env) {
                out.push(o);
                continue;
            }
            let fx = match Fixture::spawn(&["--focus", focus]) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            let f = match focus_of(&fx) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            o.flag(
                "range_settable",
                f.el.settable(attr::SELECTED_TEXT_RANGE).unwrap_or(false),
            );
            let st = f.el.set_range(attr::SELECTED_TEXT_RANGE, loc, len);
            o.num("set_range_ax", st);
            sleep(Duration::from_millis(100));
            let back = f.el.range_attr(attr::SELECTED_TEXT_RANGE);
            let range_ok = back == Ok((loc, len));
            o.flag("range_readback_ok", range_ok);
            let sel_ok =
                f.el.text_attr(attr::SELECTED_TEXT)
                    .map(|t| t.matches(&expected))
                    .unwrap_or(false);
            o.flag("selected_text_matches", sel_ok);
            if st != 0 || !range_ok || !sel_ok {
                o.fail("selection not applied or not read back");
            }
            out.push(o);
        }
        out
    }

    fn m_p05(env: &Env) -> Vec<Outcome> {
        let view_line1 = "Synthetic line one\n".encode_utf16().count() as isize;
        let mut out = Vec::new();
        for (focus, original, loc) in [
            ("field", syn::FIELD_TEXT, 0isize),
            ("view", syn::VIEW_TEXT, view_line1),
        ] {
            let mut o = Outcome::new("M-P05");
            o.meta("fixture", focus);
            if ax_skip(&mut o, env) {
                out.push(o);
                continue;
            }
            let fx = match Fixture::spawn(&["--focus", focus]) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            let f = match focus_of(&fx) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            // Pre-write checks (refuse rather than modify): plain text class and known content.
            let pre_ok = class_of(&f.el) == FieldClass::Text
                && f.el
                    .text_attr(attr::VALUE)
                    .map(|v| v.matches(original))
                    .unwrap_or(false);
            if !pre_ok {
                o.fail("pre-write verification failed; nothing written");
                out.push(o);
                continue;
            }
            f.el.set_range(attr::SELECTED_TEXT_RANGE, loc, 6);
            sleep(Duration::from_millis(100));
            let settable = f.el.settable(attr::SELECTED_TEXT).unwrap_or(false);
            o.flag("selected_text_settable", settable);
            if !settable {
                o.fail("kAXSelectedText not settable; M1 refused");
                out.push(o);
                continue;
            }
            let mut units: Vec<u16> = original.encode_utf16().collect();
            let lo = loc as usize;
            units.splice(lo..lo + 6, syn::M1_REPLACEMENT.encode_utf16());
            let expected = String::from_utf16_lossy(&units);
            let t0 = Instant::now();
            let st = f.el.set_string(attr::SELECTED_TEXT, syn::M1_REPLACEMENT);
            o.num("set_selected_text_ax", st);
            let (ok, ms, _) = wait_value(&f.el, &expected, Duration::from_secs(2));
            o.flag("replacement_verified", ok)
                .num("write_verify_ms", ms.max(t0.elapsed().as_millis()));
            if !ok {
                o.fail("M1 replacement not observed");
                out.push(o);
                continue;
            }
            // Undo: Cmd+Z through the fixture's Edit menu; count steps back to the original.
            if !env.can_post {
                o.skip("CGPreflightPostEventAccess=false; undo not tested");
                out.push(o);
                continue;
            }
            let delivery = if f.via_system_wide && f.frontmost {
                Delivery::Hid
            } else {
                Delivery::Pid(fx.pid)
            };
            o.meta("undo_delivery", delivery.name());
            let mut steps = -1i32;
            for i in 1..=3 {
                events::key_chord(delivery, vk::ANSI_Z, CGEventFlags::MaskCommand);
                let (back, _, _) = wait_value(&f.el, original, Duration::from_millis(800));
                if back {
                    steps = i;
                    break;
                }
            }
            o.num("undo_steps_to_original", steps);
            out.push(o);
        }
        out
    }

    fn m_p06(env: &Env) -> Vec<Outcome> {
        let payload = syn::m3_payload();
        let units = payload.encode_utf16().count();
        let mut out = Vec::new();
        // (fixture focus, delivery kind, chunk size)
        let variants: [(&str, &str, usize); 5] = [
            ("field", "pid", 1),
            ("field", "pid", 20),
            ("field", "hid", 20),
            ("view", "pid", 20),
            ("field", "pid", 40),
        ];
        for (focus, kind, chunk) in variants {
            let mut o = Outcome::new("M-P06");
            o.meta("fixture", focus)
                .meta("delivery", kind)
                .num("chunk_utf16", chunk);
            if ax_skip(&mut o, env) {
                out.push(o);
                continue;
            }
            if !env.can_post {
                o.skip("CGPreflightPostEventAccess=false");
                out.push(o);
                continue;
            }
            let fx = match Fixture::spawn(&["--focus", focus, "--empty"]) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            let f = match focus_of(&fx) {
                Ok(f) => f,
                Err(e) => {
                    o.fail(&e);
                    out.push(o);
                    continue;
                }
            };
            let delivery = if kind == "hid" {
                if !(f.via_system_wide && f.frontmost) {
                    o.skip("fixture is not frontmost/key; HID-delivered events would go elsewhere");
                    out.push(o);
                    continue;
                }
                Delivery::Hid
            } else {
                Delivery::Pid(fx.pid)
            };
            let empty =
                f.el.text_attr(attr::VALUE)
                    .map(|v| v.is_empty())
                    .unwrap_or(false);
            if class_of(&f.el) != FieldClass::Text || !empty {
                o.fail("target not an empty plain text element; nothing typed");
                out.push(o);
                continue;
            }
            let t0 = Instant::now();
            let n = events::type_unicode(delivery, &payload, chunk, 0);
            let post_ms = t0.elapsed().as_millis();
            let (ok, _, got_len) = wait_value(&f.el, &payload, Duration::from_secs(8));
            let total = t0.elapsed();
            o.num("utf16_units", units).num("key_downs", n.unwrap_or(0));
            o.num("post_ms", post_ms)
                .num("until_verified_ms", total.as_millis());
            o.num("received_utf16", got_len);
            o.flag("text_matches", ok);
            if ok {
                o.num(
                    "units_per_sec",
                    (units as f64 / total.as_secs_f64()).round(),
                );
            }
            if chunk > 20 {
                // Documented limit is not stated by Apple; this variant records the fact.
                o.set_status(Status::Fact);
            } else if !ok {
                o.fail("typed text not observed");
            }
            out.push(o);
        }
        out
    }

    fn m_p08() -> Vec<Outcome> {
        let pb = Board::general();
        let mut out = Vec::new();

        // (a) whitelisted snapshot -> our write with privacy markers -> restore -> byte equality
        let mut o = Outcome::new("M-P08");
        o.meta("case", "restore-when-unchanged");
        let c0 = pb.change_count();
        pb.clear();
        pb.set_string(pasteboard::T_UTF8, "synthetic original clipboard");
        pb.set_data(pasteboard::T_HTML, b"<b>synthetic</b>");
        let c_seed = pb.change_count();
        o.num("cc_delta_seed", c_seed - c0);
        match pb.snapshot() {
            Ok(snap) => {
                o.num("snapshot_types", snap.type_count())
                    .num("snapshot_bytes", snap.byte_len());
                let ours = pb.write_ours(syn::M1_REPLACEMENT);
                o.num("cc_delta_ours", ours - c_seed);
                let types = pb.types();
                let markers = types.iter().any(|t| t == pasteboard::T_TRANSIENT)
                    && types.iter().any(|t| t == pasteboard::T_CONCEALED);
                o.flag("privacy_markers_present", markers);
                let r = pb.restore_if_unchanged(ours, &snap);
                o.flag("restored", r == RestoreOutcome::Restored);
                let eq = pb.equals(&snap);
                o.flag("bytes_equal_after_restore", eq);
                if !markers || r != RestoreOutcome::Restored || !eq {
                    o.fail("snapshot/restore round-trip failed");
                }
            }
            Err(e) => {
                o.meta("snapshot_error", &format!("{e:?}"));
                o.fail("whitelisted snapshot refused");
            }
        }
        out.push(o);

        // (b) someone else writes after us -> we must NOT restore
        let mut o = Outcome::new("M-P08");
        o.meta("case", "no-restore-when-changed");
        pb.clear();
        pb.set_string(pasteboard::T_UTF8, "synthetic original clipboard");
        if let Ok(snap) = pb.snapshot() {
            let ours = pb.write_ours(syn::M1_REPLACEMENT);
            pb.clear(); // simulated third-party copy
            pb.set_string(pasteboard::T_UTF8, "synthetic third-party copy");
            let r = pb.restore_if_unchanged(ours, &snap);
            o.flag("restore_refused", r == RestoreOutcome::ChangedByOther);
            let kept = pb.data(pasteboard::T_UTF8).as_deref()
                == Some("synthetic third-party copy".as_bytes());
            o.flag("third_party_content_kept", kept);
            if r != RestoreOutcome::ChangedByOther || !kept {
                o.fail("restored over a newer clipboard");
            }
        } else {
            o.fail("snapshot refused");
        }
        out.push(o);

        // (c) non-whitelisted (private) type -> snapshot refused -> M2 not allowed
        let mut o = Outcome::new("M-P08");
        o.meta("case", "refuse-private-type");
        pb.clear();
        pb.set_string(pasteboard::T_UTF8, "synthetic");
        pb.set_data("com.example.keyswitch.private", b"\x00\x01synthetic");
        let r = pb.snapshot();
        let refused = matches!(r, Err(SnapshotError::UnsupportedType));
        o.flag("snapshot_refused", refused);
        if !refused {
            o.fail("private type was not refused");
        }
        pb.clear();
        out.push(o);
        out
    }

    fn m_p15() -> Outcome {
        let mut o = Outcome::new("M-P15");
        let Some(cur) = carbon::current_keyboard_source() else {
            o.fail("TISCopyCurrentKeyboardInputSource returned NULL");
            return o;
        };
        let cur_id = cur.id();
        o.meta("current", &cur_id);
        let enabled: Vec<_> = carbon::input_sources(false)
            .into_iter()
            .filter(|s| s.is_keyboard() && s.is_select_capable())
            .collect();
        o.num("enabled_selectable_keyboard_sources", enabled.len());
        let ids: Vec<String> = enabled.iter().map(|s| s.id()).collect();
        o.meta("enabled_ids", &ids.join(","));

        // Target: another enabled source, else try to enable the Russian layout (CI VM only).
        let mut enabled_by_us = None;
        let target = match enabled.into_iter().find(|s| s.id() != cur_id) {
            Some(t) => Some(t),
            None => {
                let ru = carbon::input_sources(true)
                    .into_iter()
                    .find(|s| s.id() == "com.apple.keylayout.Russian");
                match ru {
                    Some(ru) => {
                        let st = ru.enable();
                        o.num("enable_russian_status", st);
                        if st == 0 {
                            enabled_by_us = Some(ru.id());
                            Some(ru)
                        } else {
                            None
                        }
                    }
                    None => {
                        o.meta("russian_installed", "no");
                        None
                    }
                }
            }
        };
        let Some(target) = target else {
            o.skip("no second selectable keyboard source available on this runner");
            return o;
        };
        let target_id = target.id();
        o.meta("target", &target_id);
        let st = target.select();
        o.num("select_status", st);
        let mut switched = false;
        for _ in 0..20 {
            if carbon::current_keyboard_source().map(|s| s.id()) == Some(target_id.clone()) {
                switched = true;
                break;
            }
            sleep(Duration::from_millis(50));
        }
        o.flag("switched", switched);
        let st_back = cur.select();
        sleep(Duration::from_millis(200));
        let restored = carbon::current_keyboard_source().map(|s| s.id()) == Some(cur_id.clone());
        o.num("restore_status", st_back).flag("restored", restored);
        if enabled_by_us.is_some() {
            o.num("disable_status", target.disable());
        }
        if st != 0 || !switched || !restored {
            o.fail("TIS switch/restore not confirmed");
        }
        o
    }

    fn m_p18(env: &Env) -> Vec<Outcome> {
        let mut out = Vec::new();
        let cco = mods::CONTROL | mods::OPTION | mods::CMD;

        let mut o = Outcome::new("M-P18");
        o.meta("case", "register-and-duplicate");
        let first = carbon::register_hotkey(vk::ANSI_K as u32, cco, 1);
        o.num(
            "first_status",
            first.as_ref().map(|_| 0).unwrap_or_else(|e| *e),
        );
        let dup = carbon::register_hotkey(vk::ANSI_K as u32, cco, 2);
        let dup_st = dup.as_ref().map(|_| 0).unwrap_or_else(|e| *e);
        o.num("duplicate_same_process_status", dup_st);
        match Fixture::spawn(&[
            "--hotkey-register",
            &vk::ANSI_K.to_string(),
            &cco.to_string(),
        ]) {
            Ok(mut fx) => {
                // HOTKEY_STATUS is printed before READY; it is already queued.
                let st = fx.wait_line("HOTKEY_STATUS", Duration::from_secs(1));
                o.meta(
                    "duplicate_other_process_status",
                    st.as_deref().unwrap_or("none"),
                );
            }
            Err(e) => {
                o.meta("duplicate_other_process_status", &e);
            }
        }
        if first.is_err() || dup_st != carbon::HOTKEY_EXISTS_ERR {
            o.fail("expected success then eventHotKeyExistsErr (-9878)");
        }
        drop(dup);
        drop(first);
        out.push(o);

        let mut o = Outcome::new("M-P18");
        o.set_status(Status::Fact);
        o.meta("case", "modifier-classes");
        for (key, code, m) in [
            ("option_only_status", vk::ANSI_K, mods::OPTION),
            (
                "option_shift_status",
                vk::ANSI_K,
                mods::OPTION | mods::SHIFT,
            ),
            ("ctrl_only_status", vk::ANSI_K, mods::CONTROL),
            ("no_modifier_digit1_status", vk::ANSI_1, 0),
        ] {
            let r = carbon::register_hotkey(code as u32, m, 3);
            o.num(key, r.as_ref().map(|_| 0).unwrap_or_else(|e| *e));
        }
        out.push(o);

        // Firing: fixture registers Ctrl+Opt+Cmd+J and runs NSApp; we post the chord.
        let mut o = Outcome::new("M-P18");
        o.meta("case", "fires-on-synthetic-chord");
        if !env.can_post {
            o.skip("CGPreflightPostEventAccess=false");
            out.push(o);
            return out;
        }
        match Fixture::spawn(&["--hotkey-listen", &vk::ANSI_J.to_string(), &cco.to_string()]) {
            Ok(mut fx) => {
                sleep(Duration::from_millis(500));
                let flags = CGEventFlags::MaskControl
                    | CGEventFlags::MaskAlternate
                    | CGEventFlags::MaskCommand;
                events::key_chord(Delivery::Hid, vk::ANSI_J, flags);
                let fired = fx
                    .wait_line("HOTKEY_FIRED", Duration::from_secs(3))
                    .is_some();
                o.flag("fired", fired);
                if fired {
                    o.set_status(Status::Pass);
                } else {
                    o.skip("synthetic HID chord did not trigger the hotkey; needs a manual test");
                }
            }
            Err(e) => {
                o.fail(&e);
            }
        }
        out.push(o);
        out
    }

    // ---------------------------------------------------------------- runner

    pub fn run() -> i32 {
        let env = Env {
            ax_trusted: ax::is_trusted(),
            can_post: events::can_post(),
        };
        let mut all = vec![m_p00(&env)];
        all.extend(m_p01(&env));
        all.extend(m_p02(&env));
        all.extend(m_p03(&env));
        all.extend(m_p05(&env));
        all.extend(m_p06(&env));
        all.extend(m_p08());
        all.push(m_p15());
        all.extend(m_p18(&env));

        let mut counts = [0usize; 4];
        for o in &all {
            println!("PROBE {}", o.to_json());
            counts[match o.status() {
                Status::Pass => 0,
                Status::Fail => 1,
                Status::Skip => 2,
                Status::Fact => 3,
            }] += 1;
        }
        println!(
            "PROBE-SUMMARY pass={} fail={} skip={} fact={} total={}",
            counts[0],
            counts[1],
            counts[2],
            counts[3],
            all.len()
        );
        if counts[1] > 0 { 1 } else { 0 }
    }
}
