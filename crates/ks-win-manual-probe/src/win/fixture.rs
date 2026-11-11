//! Mode `fixture`: S-2a scenarios on the real desktop, but only against the tool's own
//! window. Order: UIA (A), W1 (B), W3 (C), layouts (E) run without prompts while our window
//! is foreground; clipboard (D) and hotkey (H) need the owner's answers in the console.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use windows::Win32::UI::Controls::{EM_CANUNDO, EM_UNDO};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_F12;

use super::clip::{self, Restore, SnapOutcome};
use super::input::{self, SendResult};
use super::kbd::{self, Hkl};
use super::sys::{self, Hwnd};
use super::uia::{self, ComGuard, GET_TEXT_CALLS, Uia};
use super::window::{self, TestWindow, W1Refusal};
use super::{ask, read_line};
use crate::policy::{self, LayoutKind, Report};
use crate::say;

type Fields = Vec<(&'static str, String)>;

fn b(v: bool) -> String {
    v.to_string()
}

fn ms(d: Option<Duration>) -> String {
    d.map_or_else(|| "timeout".into(), |d| d.as_millis().to_string())
}

fn status(ok: bool) -> &'static str {
    if ok { "PASS" } else { "FAIL" }
}

/// Makes our window foreground: request first, then ask the owner to click it.
fn ensure_foreground(w: &TestWindow) -> (bool, &'static str) {
    if w.is_foreground() || w.request_foreground() {
        return (true, "request");
    }
    say("");
    say(">>> Щёлкните мышью по ЗАГОЛОВКУ окна «KeySwitch probe - test window»");
    say("    (справа на экране). Ждём до 30 секунд...");
    let ok = window::wait_until(Duration::from_secs(30), || w.is_foreground()).is_some();
    if ok {
        say("    Спасибо. Не трогайте клавиатуру и мышь, пока тест идёт.");
    }
    (ok, if ok { "owner_click" } else { "not_foreground" })
}

/// Runs all fixture scenarios.
pub fn run(r: &mut Report) -> ExitCode {
    say("");
    say("Режим fixture (~5 минут). Программа откроет СВОЁ тестовое окно и будет");
    say("печатать только в него. Временно затрагиваются: раскладка клавиатуры");
    say("(EN/RU, вернётся обратно) и буфер обмена (только если в нём простой текст");
    say("или пусто; будет восстановлен). Новые раскладки не добавляются.");
    say("Закройте или сверните окна, где идёт набор текста. Пока идут тесты A-E,");
    say("не трогайте клавиатуру и мышь.");
    say("");
    say("Нажмите Enter, чтобы начать (или закройте окно, чтобы отказаться).");
    let _ = read_line();

    let com = match ComGuard::mta() {
        Ok(c) => c,
        Err(e) => {
            r.line("A0.com", "FAIL", &[("error", e)]);
            return ExitCode::from(1);
        }
    };
    let ui = match Uia::new() {
        Ok(u) => Some(u),
        Err(e) => {
            r.line("A0.uia", "FAIL", &[("error", e)]);
            None
        }
    };
    let w = match TestWindow::spawn() {
        Ok(w) => w,
        Err(e) => {
            r.line("F0.window", "FAIL", &[("error", e)]);
            return ExitCode::from(1);
        }
    };
    std::thread::sleep(Duration::from_millis(300));
    let (fg, how) = ensure_foreground(&w);
    r.line(
        "F0.window",
        if fg { "PASS" } else { "SKIP" },
        &[
            ("foreground", b(fg)),
            ("how", how.into()),
            ("richedit", b(w.controls.rich.is_some())),
        ],
    );

    if let Some(ui) = &ui {
        scenario_a(r, ui, &w, fg);
    }
    scenario_b(r, &w);
    scenario_c(r, &w, fg);
    scenario_e(r, &w, fg);
    scenario_d(r, &w);
    scenario_h(r);

    drop(w);
    drop(ui);
    drop(com);
    say("");
    say("Режим fixture завершён. Следующий шаг - режим survey (см. инструкцию).");
    ExitCode::SUCCESS
}

const SYNTH: &str = "KeySwitch fixture 123";

fn scenario_a(r: &mut Report, ui: &Uia, w: &TestWindow, fg: bool) {
    say("[A] UI Automation на своём окне...");
    if !fg {
        r.line(
            "A1.uia_focus",
            "SKIP",
            &[("reason", "test window not foreground".into())],
        );
        return;
    }
    let c = w.controls;
    window::set_text(c.single, SYNTH);
    window::set_text(c.multi, SYNTH);
    window::set_text(c.password, SYNTH);
    if let Some(h) = c.rich {
        window::set_text(h, SYNTH);
    }
    let mut all_ok = true;
    let mut controls: Vec<(&'static str, Hwnd, bool)> = vec![
        ("single", c.single, false),
        ("multi", c.multi, false),
        ("password", c.password, true),
    ];
    if let Some(h) = c.rich {
        controls.push(("richedit", h, false));
    }
    for (name, h, expect_pw) in controls {
        let focused = w.focus(h);
        let mut f = uia::Facts::default();
        let t = window::wait_until(Duration::from_secs(2), || {
            if let Ok(el) = ui.focused() {
                f = uia::facts(&el);
                f.hwnd == Some(h)
            } else {
                false
            }
        });
        let ok = focused && t.is_some() && f.treat_as_password() == expect_pw;
        all_ok &= ok;
        r.line(
            "A1.uia_focus",
            status(ok),
            &[
                ("control", name.into()),
                ("focus_ms", ms(t)),
                ("control_type", f.control_type.to_string()),
                ("localized_type", f.localized_type.clone()),
                ("framework", f.framework.clone()),
                ("is_password", format!("{:?}", f.is_password)),
                ("es_password", b(f.es_password)),
                ("text_pattern", b(f.text)),
                ("text_pattern2", b(f.text2)),
                ("value_pattern", b(f.value)),
            ],
        );
    }
    let _ = all_ok;

    // A2: the password guard never calls GetText.
    let before = GET_TEXT_CALLS.load(std::sync::atomic::Ordering::SeqCst);
    let mut fields: Fields = Vec::new();
    let mut pw_ok = false;
    if w.focus(c.password)
        && let Ok(el) = ui.focused()
    {
        let f = uia::facts(&el);
        let p = uia::text_probe(&el, &f);
        let after = GET_TEXT_CALLS.load(std::sync::atomic::Ordering::SeqCst);
        pw_ok = f.treat_as_password() && after == before;
        fields.push(("treated_as_password", b(f.treat_as_password())));
        fields.push(("doc_len", p.doc_len));
        fields.push(("get_text_calls_delta", (after - before).to_string()));
    }
    r.line("A2.password_guard", status(pw_ok), &fields);

    // A3: read (synthetic) + select via UIA on the single-line Edit.
    let mut fields: Fields = Vec::new();
    let mut ok = false;
    if w.focus(c.single)
        && let Ok(el) = ui.focused()
    {
        let f = uia::facts(&el);
        if let Some(tp) = uia::text_pattern(&el) {
            // SAFETY: method call on a live interface.
            let eq = unsafe { tp.DocumentRange() }
                .ok()
                .map(|d| uia::guarded_equals(&f, &d, SYNTH));
            let read_ok = matches!(eq, Some(Ok(true)));
            let sel = uia::prefix_range(&tp, 9).and_then(|rg| uia::select(&rg));
            let (s, e) = window::get_sel(c.single);
            let ranges = uia::selection(&tp).map(|v| v.len()).unwrap_or(0);
            let sel_len = uia::selection(&tp)
                .ok()
                .and_then(|v| v.into_iter().next())
                .and_then(|rg| uia::guarded_len(&f, &rg).ok());
            ok = read_ok && sel.is_ok() && (s, e) == (0, 9) && ranges == 1 && sel_len == Some(9);
            fields.push(("doc_text_matches_synthetic", b(read_ok)));
            fields.push(("select_ok", b(sel.is_ok())));
            fields.push(("em_getsel", format!("{s}..{e}")));
            fields.push(("selection_ranges", ranges.to_string()));
            fields.push(("selection_len", format!("{sel_len:?}")));
        } else {
            fields.push(("text_pattern", "false".into()));
        }
    }
    r.line("A3.uia_read_select", status(ok), &fields);
}

fn scenario_b(r: &mut Report, w: &TestWindow) {
    say("[B] Запись W1 (EM_REPLACESEL) в своё поле Edit + отмена...");
    let c = w.controls;
    for (name, h) in [("single", c.single), ("multi", c.multi)] {
        let original = "hello world";
        window::set_text(h, original);
        window::set_sel(h, 0, 5);
        let wrote = window::w1_replace(h, "привет");
        let after = window::get_text(h);
        let replaced = after == "привет world";
        let caret = window::get_sel(h);
        let can_undo = window::send_simple(h, EM_CANUNDO) == Some(1);
        let _ = window::send_simple(h, EM_UNDO);
        let undone = window::get_text(h) == original;
        let ok = matches!(wrote, Ok(true)) && replaced && can_undo && undone;
        r.line(
            "B1.w1_edit_undo",
            status(ok),
            &[
                ("control", name.into()),
                ("guard", format!("{:?}", wrote.map(|_| ()))),
                ("replaced_ok", b(replaced)),
                ("caret", format!("{}..{}", caret.0, caret.1)),
                ("can_undo", b(can_undo)),
                ("undo_restored", b(undone)),
            ],
        );
    }
    let pw = window::w1_guard(c.password);
    let rich = c.rich.map(window::w1_guard);
    let ok =
        pw == Err(W1Refusal::Password) && rich.is_none_or(|g| g == Err(W1Refusal::NotEditClass));
    r.line(
        "B2.w1_guard_refusals",
        status(ok),
        &[
            ("password", format!("{pw:?}")),
            ("richedit", format!("{rich:?}")),
        ],
    );
}

fn scenario_c(r: &mut Report, w: &TestWindow, fg: bool) {
    say("[C] Ввод W3 (SendInput Unicode) в своё поле...");
    let c = w.controls;
    if !fg || !w.request_foreground() {
        r.line(
            "C1.w3_short",
            "SKIP",
            &[("reason", "test window not foreground".into())],
        );
        return;
    }
    let short = "Hello Привет 123 \u{1F600}";
    let long: String = "Съешь же ещё этих мягких французских булок abc 0123456789 "
        .chars()
        .cycle()
        .take(200)
        .collect::<String>()
        .trim_end()
        .to_string();
    for (id, text) in [("C1.w3_short", short.to_string()), ("C2.w3_200", long)] {
        window::set_text(c.single, "");
        let focused = w.focus(c.single);
        let mut checks = 0u32;
        let t0 = Instant::now();
        let res = input::send_unicode(&text, || {
            checks += 1;
            w.is_foreground() && w.has_focus(c.single) && !sys::modifiers_down()
        });
        let units = text.encode_utf16().count();
        let arrived = window::wait_until(Duration::from_secs(5), || {
            window::text_len(c.single) >= units
        })
        .map(|_| t0.elapsed());
        let exact = window::get_text(c.single) == text;
        let can_undo = window::send_simple(c.single, EM_CANUNDO) == Some(1);
        let _ = window::send_simple(c.single, EM_UNDO);
        let undo_len = window::text_len(c.single);
        let ok = focused && matches!(res, SendResult::Sent { .. }) && exact;
        r.line(
            id,
            status(ok),
            &[
                ("units", units.to_string()),
                ("result", format!("{res:?}")),
                ("pre_batch_checks", checks.to_string()),
                ("arrived_ms", ms(arrived)),
                ("exact", b(exact)),
                ("can_undo", b(can_undo)),
                ("len_after_one_undo", undo_len.to_string()),
            ],
        );
        if !ok && matches!(res, SendResult::TargetLost { .. }) {
            say("    Ввод остановлен: окно потеряло фокус или была нажата клавиша.");
        }
    }
    // C3: refused before any input.
    window::set_text(c.single, "x");
    let mut checks = 0u32;
    let mut refused = 0;
    let bad = [
        "a\nb",
        "a\tb",
        "a\r\nb",
        "a\u{2028}b",
        "a\u{202E}b",
        "a\u{200B}b",
    ];
    for t in bad {
        if input::send_unicode(t, || {
            checks += 1;
            false
        }) == SendResult::Refused
        {
            refused += 1;
        }
    }
    let unchanged = window::get_text(c.single) == "x";
    r.line(
        "C3.w3_refusals",
        status(refused == bad.len() && checks == 0 && unchanged),
        &[
            ("refused", format!("{refused}/{}", bad.len())),
            ("pre_batch_checks", checks.to_string()),
            ("field_unchanged", b(unchanged)),
        ],
    );
}

fn scenario_e(r: &mut Report, w: &TestWindow, fg: bool) {
    say("[E] Раскладки EN/RU (только уже установленные)...");
    let installed = kbd::installed_layouts();
    let en = installed
        .iter()
        .copied()
        .find(|h| policy::layout_kind(h.lang_id()) == LayoutKind::En);
    let ru = installed
        .iter()
        .copied()
        .find(|h| policy::layout_kind(h.lang_id()) == LayoutKind::Ru);
    let initial = kbd::thread_layout(w.thread_id);
    let mut f: Fields = vec![
        (
            "installed",
            installed
                .iter()
                .map(|h| h.hex())
                .collect::<Vec<_>>()
                .join(","),
        ),
        ("initial", initial.hex()),
        (
            "initial_kind",
            format!("{:?}", policy::layout_kind(initial.lang_id())),
        ),
    ];
    let (Some(en), Some(ru)) = (en, ru) else {
        f.push(("reason", "EN and RU layouts are not both installed".into()));
        r.line("E1.layout_switch", "SKIP", &f);
        return;
    };
    if !fg || !w.request_foreground() || !w.focus(w.controls.single) {
        f.push(("reason", "test window not foreground".into()));
        r.line("E1.layout_switch", "SKIP", &f);
        return;
    }
    let target = w.controls.single;
    let switch = |to: Hkl| -> Option<Duration> {
        if !kbd::request_switch(target, to) {
            return None;
        }
        window::wait_until(Duration::from_secs(3), || {
            kbd::thread_layout(w.thread_id) == to
        })
    };
    let mut plan: Vec<Hkl> = Vec::new();
    match policy::layout_kind(initial.lang_id()) {
        LayoutKind::Ru => plan.push(en),
        LayoutKind::En => plan.push(ru),
        LayoutKind::Other => {
            plan.push(ru);
            plan.push(en);
        }
    }
    let mut ok = true;
    for (i, h) in plan.iter().enumerate() {
        let t = switch(*h);
        ok &= t.is_some();
        f.push((
            if i == 0 { "switch1" } else { "switch2" },
            format!("{}:{}", h.hex(), ms(t)),
        ));
    }
    let back = switch(initial);
    ok &= back.is_some();
    f.push(("restored_initial_ms", ms(back)));
    let unchanged = kbd::installed_layouts() == installed;
    f.push(("installed_list_unchanged", b(unchanged)));
    r.line("E1.layout_switch", status(ok && unchanged), &f);
    if back.is_none() {
        say("    ВНИМАНИЕ: раскладку не удалось вернуть автоматически - переключите её");
        say("    обычным способом (Win+Пробел), если нужно.");
    }
}

const CLIP_TEST: &str = "KeySwitch-TEST-7731";

fn scenario_d(r: &mut Report, w: &TestWindow) {
    say("");
    say("[D] Буфер обмена.");
    say("    Сейчас ничего не копируйте, пока тест не скажет, что закончил.");
    let owner = w.controls.main;
    let snap = match clip::snapshot(owner) {
        SnapOutcome::Taken(s) => s,
        SnapOutcome::NotRestorable(formats) => {
            r.line(
                "D1.clipboard",
                "SKIP",
                &[
                    (
                        "reason",
                        "clipboard holds non-text formats; not touched".into(),
                    ),
                    ("format_count", formats.len().to_string()),
                ],
            );
            say("    Пропущено: в буфере не простой текст (картинка/файлы/таблица).");
            say("    Ваш буфер не тронут. Чтобы проверить буфер, скопируйте любое");
            say("    простое слово из Блокнота и запустите fixture ещё раз (необязательно).");
            return;
        }
        SnapOutcome::Failed(e) => {
            r.line(
                "D1.clipboard",
                "SKIP",
                &[("reason", "snapshot failed".into()), ("error", e)],
            );
            return;
        }
    };
    let mut f: Fields = vec![
        ("snapshot_formats", snap.formats.len().to_string()),
        ("snapshot_text_bytes", snap.unicode_bytes().to_string()),
        (
            "snapshot_privacy_markers",
            snap.privacy_ids().len().to_string(),
        ),
    ];
    let our_seq = match clip::put_private_test(owner, CLIP_TEST, &snap) {
        Ok(s) => s,
        Err(e) => {
            f.push(("put_error", e));
            r.line("D1.clipboard", "FAIL", &f);
            return;
        }
    };
    let pf = clip::privacy_formats();
    let markers =
        pf.iter().all(|&x| clip::available(x)) && clip::available(policy::cf::UNICODETEXT);
    f.push(("markers_present", b(markers)));

    say("");
    say(&format!(
        ">>> Нажмите Win+V (журнал буфера обмена). Есть ли в списке строка «{CLIP_TEST}»?"
    ));
    say("    Ничего в списке не выбирайте; закройте журнал клавишей Esc и ответьте здесь:");
    let a = ask(
        "    y = да, строка видна;  n = журнал открылся, строки НЕТ;  d = журнал выключен / не знаю",
        &["y", "n", "d"],
    );
    let winv = match a.as_str() {
        "y" => "visible",
        "n" => "not_visible",
        _ => "history_off_or_unknown",
    };
    f.push(("winv_owner_answer", winv.into()));

    let res = clip::restore_if_unchanged(owner, our_seq, &snap);
    f.push(("restore", format!("{res:?}")));
    let mut restored_ok = false;
    if res == Ok(Restore::Restored) {
        match clip::verify_restored(owner, &snap) {
            Ok((t, p)) => {
                restored_ok = t && p;
                f.push(("restored_text_equal", b(t)));
                f.push(("restored_privacy_markers_equal", b(p)));
            }
            Err(e) => f.push(("verify_error", e)),
        }
        say("    Буфер обмена восстановлен.");
    } else if res == Ok(Restore::SkippedSeqChanged) {
        say("    Буфер изменился во время теста - не восстанавливаем, чтобы не затереть");
        say("    новые данные.");
    }
    drop(snap);
    let st = if !markers || res.is_err() || winv == "visible" {
        "FAIL"
    } else if winv == "not_visible" && restored_ok {
        "PASS"
    } else {
        "OBSERVED"
    };
    r.line("D1.clipboard_privacy_restore", st, &f);
    if winv == "visible" {
        say("    Строку теста можно удалить из журнала: Win+V -> «...» у строки -> Удалить.");
    }
}

fn scenario_h(r: &mut Report) {
    say("");
    say("[H] Горячая клавиша (RegisterHotKey).");
    let hk = match kbd::spawn_hotkey(1, kbd::ctrl_alt_shift(), VK_F12) {
        Ok(h) => h,
        Err(e) => {
            r.line(
                "H1.hotkey",
                "OBSERVED",
                &[
                    ("register", "failed".into()),
                    ("win32_error", e.to_string()),
                ],
            );
            say("    Сочетание Ctrl+Alt+Shift+F12 занято другой программой - записано в отчёт.");
            return;
        }
    };
    let dup = kbd::spawn_hotkey(2, kbd::ctrl_alt_shift(), VK_F12).err();
    say(">>> Нажмите Ctrl+Alt+Shift+F12 и подержите около 2 секунд, затем отпустите.");
    say("    (На ноутбуке может понадобиться ещё клавиша Fn.) Ждём до 30 секунд...");
    let t0 = Instant::now();
    let first = hk.events.recv_timeout(Duration::from_secs(30)).ok();
    let first_ms = first.map(|_| t0.elapsed().as_millis());
    let mut count = u32::from(first.is_some());
    if first.is_some() {
        let until = Instant::now() + Duration::from_secs(4);
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            if hk.events.recv_timeout(left).is_ok() {
                count += 1;
            } else {
                break;
            }
        }
    }
    drop(hk);
    let st = match (first.is_some(), count) {
        (true, 1) => "PASS",
        (true, _) => "OBSERVED",
        (false, _) => "FAIL",
    };
    r.line(
        "H1.hotkey",
        st,
        &[
            ("register", "ok".into()),
            ("duplicate_register_error", format!("{dup:?}")),
            ("received", b(first.is_some())),
            (
                "first_ms",
                first_ms.map_or_else(|| "timeout".into(), |m| m.to_string()),
            ),
            ("events_while_held", count.to_string()),
        ],
    );
    say(if first.is_some() {
        "    Получено."
    } else {
        "    Нажатие не получено за 30 секунд - записано в отчёт."
    });
}
