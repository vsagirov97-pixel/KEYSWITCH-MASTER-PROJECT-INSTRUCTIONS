//! Windows implementation: common start-up checks, console prompts and report saving.

mod clip;
mod fixture;
mod input;
mod kbd;
mod survey;
mod sys;
mod uia;
mod window;

use std::io::BufRead as _;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::policy::{Mode, Report};
use crate::say;

/// File name on the Desktop.
pub const REPORT_FILE: &str = "KeySwitch-probe-report.txt";
const REPORT_MARKER: &str = "=== KeySwitch probe report";

/// Reads one console line (trimmed). `None` on EOF.
pub fn read_line() -> Option<String> {
    let mut s = String::new();
    match std::io::stdin().lock().read_line(&mut s) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(s.trim().to_string()),
    }
}

/// Asks until the owner answers with one of `options` (lowercase, single letters).
pub fn ask(prompt: &str, options: &[&str]) -> String {
    loop {
        say(prompt);
        let Some(a) = read_line() else {
            return String::new();
        };
        let a = a.to_lowercase();
        let a = match a.as_str() {
            // Russian keyboard layout: the same physical keys as y / n / d / q.
            "н" => "y".to_string(),
            "т" => "n".to_string(),
            "в" => "d".to_string(),
            "й" => "q".to_string(),
            _ => a,
        };
        if options.contains(&a.as_str()) {
            return a;
        }
        say(&format!("  Ответьте одной буквой: {}", options.join(" / ")));
    }
}

/// Entry point after argument parsing.
pub fn run(mode: Mode) -> ExitCode {
    say("==============================================================");
    say(" KeySwitch - ручной тест Windows (W-MAN-1). Не продукт, а проверка.");
    say(" Отчёт содержит только служебные данные: длины, флаги, имена");
    say(" классов окон и exe-файлов, коды ошибок. Ваш текст не читается в");
    say(" отчёт, не сохраняется и никуда не отправляется (сети нет).");
    say("==============================================================");

    let me = sys::self_facts();
    if me.elevated != Some(false) {
        say("");
        say("ОТКАЗ: программа запущена с правами администратора (или это не удалось");
        say("проверить). Закройте это окно PowerShell и откройте PowerShell обычным");
        say("способом (НЕ «Запуск от имени администратора»), затем повторите.");
        return ExitCode::from(3);
    }
    let (session, input_desktop) = sys::session();
    if session == 0 || !input_desktop {
        say("ОТКАЗ: нет доступа к интерактивному рабочему столу (session 0 / RDP?).");
        return ExitCode::from(4);
    }

    let mut r = Report::default();
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    r.note(&format!(
        "{REPORT_MARKER} (W-MAN-1) mode={} tool={} commit={} unix_time={started} ===",
        mode.label(),
        env!("CARGO_PKG_VERSION"),
        option_env!("GITHUB_SHA").map_or("local", |s| &s[..s.len().min(12)]),
    ));
    let mut env = sys::os_facts();
    env.push(("session", session.to_string()));
    env.push(("input_desktop", input_desktop.to_string()));
    env.push(("self_elevated", "false".into()));
    env.push(("self_integrity", me.integrity.unwrap_or("?").to_string()));
    let layouts = kbd::installed_layouts();
    env.push((
        "installed_layouts",
        layouts
            .iter()
            .map(|h| h.hex())
            .collect::<Vec<_>>()
            .join(","),
    ));
    r.line("ENV", "INFO", &env);

    let code = match mode {
        Mode::Fixture => fixture::run(&mut r),
        Mode::Survey => survey::run(&mut r),
    };
    r.note(&format!(
        "=== end mode={} lines={} ===",
        mode.label(),
        r.len()
    ));
    save_and_print(&r);
    code
}

/// Writes the report to the Desktop (appending after earlier runs of this tool) and prints it.
pub fn save_and_print(r: &Report) {
    let text = r.render();
    say("");
    say("------------------------- ОТЧЁТ -------------------------");
    say(&text);
    say("---------------------------------------------------------");
    match save(&text) {
        Ok(path) => {
            say(&format!("Отчёт сохранён: {}", path.display()));
            say("Откройте этот файл (двойной щелчок), выделите всё (Ctrl+A),");
            say("скопируйте (Ctrl+C) и вставьте в чат.");
        }
        Err(e) => say(&format!(
            "Не удалось сохранить файл ({e}). Скопируйте отчёт выше из окна PowerShell."
        )),
    }
}

/// Saves silently (used during survey after every capture so nothing is lost).
pub fn save_quiet(r: &Report) {
    let _ = save(&r.render());
}

static PREVIOUS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn save(current: &str) -> Result<std::path::PathBuf, String> {
    let dir = sys::desktop_dir().ok_or("Desktop folder not found")?;
    let path = dir.join(REPORT_FILE);
    // Keep earlier sections written by this tool (e.g. fixture, then survey); never
    // overwrite a foreign file with the same name.
    let prev = PREVIOUS.get_or_init(|| match std::fs::read_to_string(&path) {
        Ok(s) if s.starts_with(REPORT_MARKER) => s,
        Ok(_) => "\u{0}foreign".into(),
        Err(_) => String::new(),
    });
    let path = if prev.starts_with('\u{0}') {
        dir.join("KeySwitch-probe-report-2.txt")
    } else {
        path
    };
    let prev = if prev.starts_with('\u{0}') {
        ""
    } else {
        prev.as_str()
    };
    std::fs::write(&path, format!("{prev}{current}")).map_err(|e| e.kind().to_string())?;
    Ok(path)
}
