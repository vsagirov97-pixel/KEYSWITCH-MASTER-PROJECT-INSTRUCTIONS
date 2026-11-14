//! Tray menu model, icon state (active / paused / attention) and commands (the icon itself
//! is `ks_platform_win::tray`). Mirrors the UI's tray model (`app/src/tray/model.ts`): same command
//! ids, same structure, labels in the UI language (texts copied from `app/src/i18n`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::os::{self, TrayEntry, TrayEvent, TrayModel, TrayState};
use crate::settings::UiLanguage;
use crate::{Pause, Shared, lock};

struct Labels {
    open: &'static str,
    pause: &'static str,
    pause_15m: &'static str,
    pause_1h: &'static str,
    pause_until_restart: &'static str,
    resume: &'static str,
    paused_until: &'static str,
    paused_until_restart: &'static str,
    settings: &'static str,
    restore_clipboard: &'static str,
    quit: &'static str,
    status_active: &'static str,
    status_paused: &'static str,
    status_attention: &'static str,
}

fn labels(language: UiLanguage) -> Labels {
    match language {
        UiLanguage::Ru => Labels {
            open: "Открыть KeySwitch",
            pause: "Приостановить",
            pause_15m: "15 минут",
            pause_1h: "1 час",
            pause_until_restart: "До перезапуска",
            resume: "Возобновить",
            paused_until: "Приостановлено до",
            paused_until_restart: "Приостановлено до перезапуска",
            settings: "Настройки",
            restore_clipboard: "Вернуть буфер обмена",
            quit: "Выход",
            status_active: "активно",
            status_paused: "приостановлено",
            status_attention: "требует внимания",
        },
        UiLanguage::En => Labels {
            open: "Open KeySwitch",
            pause: "Pause",
            pause_15m: "15 minutes",
            pause_1h: "1 hour",
            pause_until_restart: "Until restart",
            resume: "Resume",
            paused_until: "Paused until",
            paused_until_restart: "Paused until restart",
            settings: "Settings",
            restore_clipboard: "Restore clipboard",
            quit: "Quit",
            status_active: "active",
            status_paused: "paused",
            status_attention: "needs attention",
        },
    }
}

// Command ids (the UI model's `TrayCommand`, as menu ids).
const CMD_OPEN: u32 = 1;
const CMD_PAUSE_15M: u32 = 2;
const CMD_PAUSE_1H: u32 = 3;
const CMD_PAUSE_UNTIL_RESTART: u32 = 4;
const CMD_RESUME: u32 = 5;
const CMD_SETTINGS: u32 = 6;
const CMD_QUIT: u32 = 7;
/// Shown only while a clipboard snapshot is held after an unconfirmed paste (D-26).
const CMD_RESTORE_CLIPBOARD: u32 = 8;
/// Information line (disabled).
const CMD_INFO: u32 = 100;

fn item(id: u32, label: &str) -> TrayEntry {
    TrayEntry::Item {
        id,
        label: label.to_owned(),
        enabled: true,
        default: id == CMD_OPEN,
    }
}

fn build_model(shared: &Shared) -> TrayModel {
    let (language, hotkey) = {
        let stored = lock(&shared.stored);
        (stored.ui_language, stored.hotkey.label())
    };
    let l = labels(language);
    let pause = shared.pause_state();
    let mut menu = vec![item(CMD_OPEN, l.open), TrayEntry::Separator];
    match &pause {
        Pause::Active => menu.push(TrayEntry::Submenu {
            label: l.pause.to_owned(),
            items: vec![
                item(CMD_PAUSE_15M, l.pause_15m),
                item(CMD_PAUSE_1H, l.pause_1h),
                item(CMD_PAUSE_UNTIL_RESTART, l.pause_until_restart),
            ],
        }),
        paused => {
            let info = match paused {
                Pause::Until { label, .. } => format!("{} {label}", l.paused_until),
                _ => l.paused_until_restart.to_owned(),
            };
            menu.push(TrayEntry::Item {
                id: CMD_INFO,
                label: info,
                enabled: false,
                default: false,
            });
            menu.push(item(CMD_RESUME, l.resume));
        }
    }
    if shared.worker.get().is_some_and(os::clipboard_held) {
        menu.extend([
            TrayEntry::Separator,
            item(CMD_RESTORE_CLIPBOARD, l.restore_clipboard),
        ]);
    }
    menu.extend([
        TrayEntry::Separator,
        item(CMD_SETTINGS, l.settings),
        TrayEntry::Separator,
        item(CMD_QUIT, l.quit),
    ]);

    let (state, status) = if !matches!(pause, Pause::Active) {
        (TrayState::Paused, l.status_paused)
    } else if lock(&shared.registration).is_none() || shared.worker.get().is_none() {
        // Hotkey conflict / registration error, or the worker thread could not start.
        (TrayState::Attention, l.status_attention)
    } else {
        (TrayState::Active, l.status_active)
    };
    let mut tooltip = format!("KeySwitch — {status} · {hotkey}");
    // Last operation: shown only when it was refused or failed — a reason message from
    // the UI dictionary, never text from the user's application.
    let reason = (*lock(&shared.last_outcome))
        .and_then(|last| last.notice_code())
        .and_then(|code| crate::feedback::text(language, code));
    if let Some(reason) = reason {
        tooltip.push_str(" · ");
        tooltip.push_str(reason);
    }
    TrayModel {
        tooltip,
        menu,
        state,
    }
}

/// Shows (and focuses) the settings window.
pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn pause_for(shared: &Arc<Shared>, minutes: u64) {
    let (h, m) = os::local_hour_minute();
    let total = (u64::from(h) * 60 + u64::from(m) + minutes) % (24 * 60);
    let wait = Duration::from_secs(minutes * 60);
    shared.set_pause(Pause::Until {
        deadline: Instant::now() + wait,
        label: format!("{:02}:{:02}", total / 60, total % 60),
    });
    // Refresh the menu when the pause runs out (the pause itself expires by the clock).
    let shared = Arc::clone(shared);
    let _ = std::thread::Builder::new()
        .name("ks-pause".into())
        .spawn(move || {
            std::thread::sleep(wait + Duration::from_millis(200));
            refresh(&shared);
        });
}

/// Handles a tray event on the main thread.
fn on_event(app: &AppHandle, shared: &Arc<Shared>, event: TrayEvent) {
    match event {
        TrayEvent::LeftClick | TrayEvent::Command(CMD_OPEN | CMD_SETTINGS) => show_window(app),
        TrayEvent::Command(CMD_PAUSE_15M) => pause_for(shared, 15),
        TrayEvent::Command(CMD_PAUSE_1H) => pause_for(shared, 60),
        TrayEvent::Command(CMD_PAUSE_UNTIL_RESTART) => shared.set_pause(Pause::UntilRestart),
        TrayEvent::Command(CMD_RESUME) => shared.set_pause(Pause::Active),
        TrayEvent::Command(CMD_RESTORE_CLIPBOARD) => {
            if let Some(worker) = shared.worker.get() {
                let _ = os::restore_clipboard(worker);
            }
        }
        TrayEvent::Command(CMD_QUIT) => {
            shared.set_pause(Pause::UntilRestart);
            app.exit(0);
            return;
        }
        TrayEvent::Command(_) => return,
    }
    refresh(shared);
}

/// Creates the tray icon. Called once from `setup`.
pub fn create(app: &AppHandle, shared: &Arc<Shared>) {
    let (handle, for_events) = (app.clone(), Arc::clone(shared));
    let tray = os::spawn_tray(build_model(shared), move |event| {
        // Runs on the tray thread: hand over to the main thread.
        let (app, shared) = (handle.clone(), Arc::clone(&for_events));
        let _ = handle.run_on_main_thread(move || on_event(&app, &shared, event));
    });
    if let Some(tray) = tray {
        let _ = shared.tray.set(tray);
    }
}

/// Rebuilds the menu and tooltip (language, hotkey, pause or status changed). Any thread.
pub fn refresh(shared: &Shared) {
    if let Some(tray) = shared.tray.get() {
        os::set_tray(tray, build_model(shared));
    }
}
