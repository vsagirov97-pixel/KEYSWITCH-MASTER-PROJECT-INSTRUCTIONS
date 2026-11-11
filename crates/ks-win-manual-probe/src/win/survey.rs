//! Mode `survey` (read-only). The owner focuses an input field in a real application and
//! presses the hotkey; the tool records metadata of the focused element: exe file name,
//! window classes, UIA properties and patterns, success of DocumentRange/GetSelection and
//! the LENGTHS of document/selection text. No writes, no synthetic input, no clipboard,
//! no window titles, no UIA Name/Value, and GetText is never called on password fields.

use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_F10, VK_F11, VK_F12};

use super::kbd::{self, HotkeyThread};
use super::save_quiet;
use super::sys;
use super::uia::{self, ComGuard, Uia};
use crate::policy::Report;
use crate::say;

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "?".into(), |x| x.to_string())
}

fn register() -> Option<(HotkeyThread, &'static str)> {
    let keys: [(VIRTUAL_KEY, &'static str); 3] = [
        (VK_F12, "Ctrl+Alt+Shift+F12"),
        (VK_F11, "Ctrl+Alt+Shift+F11"),
        (VK_F10, "Ctrl+Alt+Shift+F10"),
    ];
    keys.into_iter().enumerate().find_map(|(i, (vk, name))| {
        kbd::spawn_hotkey(10 + i as i32, kbd::ctrl_alt_shift(), vk)
            .ok()
            .map(|h| (h, name))
    })
}

/// Runs the survey loop until the owner types `q`.
pub fn run(r: &mut Report) -> ExitCode {
    let com = match ComGuard::mta() {
        Ok(c) => c,
        Err(e) => {
            r.line("S0.com", "FAIL", &[("error", e)]);
            return ExitCode::from(1);
        }
    };
    let ui = match Uia::new() {
        Ok(u) => u,
        Err(e) => {
            r.line("S0.uia", "FAIL", &[("error", e)]);
            return ExitCode::from(1);
        }
    };
    let Some((hk, chord)) = register() else {
        r.line(
            "S0.hotkey",
            "FAIL",
            &[("reason", "F12/F11/F10 chords all taken".into())],
        );
        say("Не удалось зарегистрировать горячую клавишу (все варианты заняты).");
        return ExitCode::from(1);
    };
    r.line("S0.start", "INFO", &[("hotkey", chord.into())]);

    say("");
    say("Режим survey: ТОЛЬКО ЧТЕНИЕ. Программа ничего не пишет и не нажимает.");
    say("Для каждого приложения из инструкции:");
    say("  1) откройте приложение и щёлкните в нужное поле ввода (курсор мигает в нём);");
    say(&format!(
        "  2) нажмите {chord} (на ноутбуке, возможно, ещё Fn);"
    ));
    say("  3) вернитесь в это окно: появится строка #N. Можно ввести короткую метку");
    say("     (например: chrome поиск) и Enter, или просто Enter. Без личных данных!");
    say("Когда закончите все приложения - введите q и Enter.");
    say("");

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::Builder::new()
        .name("ks-manual-stdin".into())
        .spawn(move || {
            while let Some(l) = super::read_line() {
                if tx.send(l).is_err() {
                    break;
                }
            }
        })
        .ok();

    let mut n = 0u32;
    loop {
        if hk.events.recv_timeout(Duration::from_millis(100)).is_ok() {
            n += 1;
            capture(r, &ui, n);
            save_quiet(r);
            say(&format!(
                "#{n} записано. Метка (или просто Enter), затем следующее приложение; q = конец."
            ));
        }
        match rx.try_recv() {
            Ok(line) => {
                let l = line.trim();
                if l.eq_ignore_ascii_case("q") || l == "й" {
                    break;
                }
                if !l.is_empty() && n > 0 {
                    r.line(&format!("S{n}.label"), "INFO", &[("label", l.to_string())]);
                    save_quiet(r);
                    say(&format!("  метка для #{n} сохранена."));
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    drop(hk);
    r.line("S0.end", "INFO", &[("captures", n.to_string())]);
    drop(ui);
    drop(com);
    ExitCode::SUCCESS
}

fn capture(r: &mut Report, ui: &Uia, n: u32) {
    let t0 = Instant::now();
    let fg = sys::foreground();
    let (tid, pid) = sys::window_thread_process(fg);
    let pf = sys::process_facts(pid);
    let focus = sys::focus_of_thread(tid);
    let mut f: Vec<(&'static str, String)> = vec![
        ("exe", pf.exe.clone().unwrap_or_else(|| "?".into())),
        ("integrity", pf.integrity.unwrap_or("?").into()),
        ("elevated", opt(pf.elevated)),
        ("proc_error", opt(pf.error)),
        ("fg_class", sys::class_name(fg)),
        (
            "focus_hwnd_class",
            if focus.is_null() {
                "none".into()
            } else {
                sys::class_name(focus)
            },
        ),
        (
            "focus_hwnd_real_class",
            if focus.is_null() {
                "none".into()
            } else {
                sys::real_class_name(focus)
            },
        ),
    ];
    match ui.focused() {
        Ok(el) => {
            let x = uia::facts(&el);
            let tp = uia::text_probe(&el, &x);
            let uia_exe = if x.pid == pid {
                "same".to_string()
            } else {
                sys::process_facts(x.pid).exe.unwrap_or_else(|| "?".into())
            };
            f.extend([
                ("uia", "ok".to_string()),
                ("uia_process", uia_exe),
                ("control_type", x.control_type.to_string()),
                ("localized_type", x.localized_type.clone()),
                ("class_name", x.class_name.clone()),
                ("framework", x.framework.clone()),
                ("is_password", opt(x.is_password)),
                ("es_password", x.es_password.to_string()),
                ("treated_as_password", x.treat_as_password().to_string()),
                ("value_read_only", opt(x.value_read_only)),
                ("enabled", opt(x.enabled)),
                ("has_focus", opt(x.has_focus)),
                (
                    "uia_hwnd_class",
                    x.hwnd.map_or_else(|| "none".into(), sys::class_name),
                ),
                ("p_text", x.text.to_string()),
                ("p_text2", x.text2.to_string()),
                ("p_value", x.value.to_string()),
                ("p_text_edit", x.text_edit.to_string()),
                ("doc_range", tp.doc_range),
                ("doc_len", tp.doc_len),
                ("selection", tp.selection),
                ("sel_count", tp.sel_count),
                ("sel_len", tp.sel_len),
                ("supported_selection", tp.supported_selection),
                ("caret_range", tp.caret_range),
                ("text_ms", tp.ms.to_string()),
            ]);
        }
        Err(e) => f.push(("uia", e)),
    }
    f.push(("total_ms", t0.elapsed().as_millis().to_string()));
    let exe = f[0].1.clone();
    r.line(&format!("S{n}"), "INFO", &f);
    say(&format!("  #{n}: {exe}"));
}
