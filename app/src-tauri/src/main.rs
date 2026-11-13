// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! KeySwitch shell (Beta 1, conversion only): tray, settings window on demand, global hotkey,
//! settings persistence, autostart. All text handling happens in `ks-engine` /
//! `ks-platform-win`; nothing on the IPC boundary can carry user text.

mod feedback;
mod hotkey;
mod macro_slot;
mod os;
mod settings;
mod tray;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, State, WindowEvent};

use hotkey::{Hotkey, Invalid};
use macro_slot::MacroSlot;
use settings::{DEFAULT_EXCLUDED, MAX_USER_EXCLUDED, Stored, Theme, UiLanguage};

/// Locks a mutex, ignoring poisoning (the guarded data are plain settings).
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Pause state chosen from the tray.
#[derive(Clone, Debug)]
enum Pause {
    Active,
    UntilRestart,
    Until {
        deadline: Instant,
        /// Local time "HH:MM" shown in the tray menu.
        label: String,
    },
}

/// State shared by commands, the tray, the hotkey thread and the worker thread.
struct Shared {
    stored: Mutex<Stored>,
    path: Option<PathBuf>,
    pause: Mutex<Pause>,
    /// The registered global hotkey; `None` = registration failed (tray shows "attention").
    registration: Mutex<Option<os::Registration>>,
    /// The registered macro hotkey; `None` = slot off, no hotkey, or registration failed.
    macro_registration: Mutex<Option<os::Registration>>,
    worker: OnceLock<os::Worker>,
    tray: OnceLock<os::Tray>,
    /// Result of the last operation: codes only, never text.
    last_outcome: Mutex<Option<feedback::LastOperation>>,
}

impl Shared {
    /// Current pause state; an elapsed timed pause turns back into `Active`.
    fn pause_state(&self) -> Pause {
        let mut pause = lock(&self.pause);
        if let Pause::Until { deadline, .. } = &*pause
            && Instant::now() >= *deadline
        {
            *pause = Pause::Active;
        }
        pause.clone()
    }

    fn set_pause(&self, pause: Pause) {
        let paused = !matches!(pause, Pause::Active);
        *lock(&self.pause) = pause;
        if paused && let Some(worker) = self.worker.get() {
            os::cancel(worker);
        }
    }

    fn save(&self) -> Result<(), String> {
        settings::save(self.path.as_deref(), &lock(&self.stored))
    }
}

/// `RegisterHotKey` for `hotkey`; every press calls `on_press` with the worker.
fn register_chord(
    shared: &Arc<Shared>,
    hotkey: &Hotkey,
    on_press: fn(&os::Worker),
) -> Result<os::Registration, &'static str> {
    let chord = hotkey.to_chord().map_err(|e| match e {
        Invalid::Invalid => "invalid",
        Invalid::Reserved => "reserved",
    })?;
    let weak = Arc::downgrade(shared);
    os::register_hotkey(chord.mods, chord.vk, move || {
        if let Some(shared) = weak.upgrade()
            && let Some(worker) = shared.worker.get()
        {
            on_press(worker);
        }
    })
    .map_err(|e| match e {
        os::HotkeyFailure::Conflict => "conflict",
        os::HotkeyFailure::Other => "invalid",
    })
}

/// Registers `hotkey` as the global hotkey. On failure the previous registration stays.
fn register_hotkey(shared: &Arc<Shared>, hotkey: &Hotkey) -> Result<(), &'static str> {
    let registration = register_chord(shared, hotkey, os::trigger)?;
    // Replacing the value drops (unregisters) the previous hotkey.
    *lock(&shared.registration) = Some(registration);
    Ok(())
}

/// Registers the hotkey of an enabled macro slot (or releases it for a disabled one). The
/// conversion hotkey always wins: the same combination is a conflict. On failure the
/// previous registration stays.
fn register_macro_hotkey(shared: &Arc<Shared>, slot: &MacroSlot) -> Result<(), &'static str> {
    let Some(hotkey) = slot.active_hotkey() else {
        *lock(&shared.macro_registration) = None;
        return Ok(());
    };
    if *hotkey == lock(&shared.stored).hotkey {
        return Err("conflict");
    }
    let registration = register_chord(shared, hotkey, os::trigger_macro)?;
    *lock(&shared.macro_registration) = Some(registration);
    Ok(())
}

// ---- IPC contract (docs/ui/beta1-ui-notes.md, app/src/api.ts) ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDto {
    hotkey: Hotkey,
    autostart: bool,
    ui_language: UiLanguage,
    theme: Theme,
}

#[derive(Serialize)]
struct HotkeyResult {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

#[derive(Serialize)]
struct ExcludedApp {
    exe: String,
    source: &'static str,
}

fn excluded_list(stored: &Stored) -> Vec<ExcludedApp> {
    let app = |exe: &str, source| ExcludedApp {
        exe: exe.to_owned(),
        source,
    };
    DEFAULT_EXCLUDED
        .iter()
        .map(|e| app(e, "default"))
        .chain(stored.excluded_user.iter().map(|e| app(e, "user")))
        .collect()
}

type Ctx<'a> = State<'a, Arc<Shared>>;

/// Returns the core library version.
#[tauri::command]
fn core_version() -> &'static str {
    ks_core::VERSION
}

/// Beta 1 builds are not code-signed.
#[tauri::command]
fn build_signed() -> bool {
    false
}

#[tauri::command]
fn get_settings(shared: Ctx<'_>) -> SettingsDto {
    let stored = lock(&shared.stored);
    SettingsDto {
        hotkey: stored.hotkey.clone(),
        autostart: os::autostart_enabled(),
        ui_language: stored.ui_language,
        theme: stored.theme,
    }
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<(), String> {
    os::set_autostart(enabled)
}

#[tauri::command]
fn set_ui_language(shared: Ctx<'_>, language: UiLanguage) -> Result<(), String> {
    lock(&shared.stored).ui_language = language;
    tray::refresh(&shared);
    shared.save()
}

#[tauri::command]
fn set_theme(shared: Ctx<'_>, theme: Theme) -> Result<(), String> {
    lock(&shared.stored).theme = theme;
    shared.save()
}

#[tauri::command]
fn set_hotkey(shared: Ctx<'_>, hotkey: Hotkey) -> HotkeyResult {
    let unchanged = lock(&shared.stored).hotkey == hotkey && lock(&shared.registration).is_some();
    let macro_owned = lock(&shared.stored).macro_slot.active_hotkey() == Some(&hotkey);
    if macro_owned {
        // The macro slot already uses this combination.
        return HotkeyResult {
            ok: false,
            error: Some("conflict"),
        };
    }
    if !unchanged {
        if let Err(error) = register_hotkey(&shared, &hotkey) {
            return HotkeyResult {
                ok: false,
                error: Some(error),
            };
        }
        lock(&shared.stored).hotkey = hotkey;
        // The hotkey is already active; a failed save only loses it at the next start.
        let _ = shared.save();
        tray::refresh(&shared);
    }
    HotkeyResult {
        ok: true,
        error: None,
    }
}

#[tauri::command]
fn list_excluded_apps(shared: Ctx<'_>) -> Vec<ExcludedApp> {
    excluded_list(&lock(&shared.stored))
}

#[tauri::command]
fn add_excluded_app(shared: Ctx<'_>, exe: String) -> Result<Vec<ExcludedApp>, String> {
    let name = ks_engine::normalize_exe(&exe).ok_or("invalid executable name")?;
    {
        let mut stored = lock(&shared.stored);
        let known =
            DEFAULT_EXCLUDED.contains(&name.as_str()) || stored.excluded_user.contains(&name);
        if !known {
            if stored.excluded_user.len() >= MAX_USER_EXCLUDED {
                return Err("too many excluded applications".into());
            }
            stored.excluded_user.push(name);
        }
    }
    shared.save()?;
    Ok(excluded_list(&lock(&shared.stored)))
}

#[tauri::command]
fn remove_excluded_app(shared: Ctx<'_>, exe: String) -> Result<Vec<ExcludedApp>, String> {
    lock(&shared.stored).remove_user_excluded(&exe);
    shared.save()?;
    Ok(excluded_list(&lock(&shared.stored)))
}

/// The Macro Studio slot as the UI edits it, plus backend facts.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MacroSlotDto {
    #[serde(flatten)]
    slot: MacroSlot,
    /// The slot's hotkey is registered right now.
    registered: bool,
    /// Test run from the editor: not available in this step.
    test_run_available: bool,
}

#[derive(Serialize)]
struct MacroSlotResult {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

#[tauri::command]
fn get_macro_slot(shared: Ctx<'_>) -> MacroSlotDto {
    MacroSlotDto {
        slot: lock(&shared.stored).macro_slot.clone(),
        registered: lock(&shared.macro_registration).is_some(),
        test_run_available: false,
    }
}

/// Validates and stores the slot and (re-)registers its hotkey. On a validation or
/// registration error nothing changes (the previous slot and registration stay);
/// `save_failed` means the slot is active but not persisted.
#[tauri::command]
fn set_macro_slot(shared: Ctx<'_>, slot: MacroSlot) -> MacroSlotResult {
    let fail = |error| MacroSlotResult {
        ok: false,
        error: Some(error),
    };
    if let Err(error) = macro_slot::validate(&slot) {
        return fail(error);
    }
    let (same_hotkey, registered) = {
        let stored = lock(&shared.stored);
        (
            stored.macro_slot.active_hotkey() == slot.active_hotkey(),
            lock(&shared.macro_registration).is_some(),
        )
    };
    let needs_registration = slot.active_hotkey().is_some() && !registered;
    if (!same_hotkey || needs_registration)
        && let Err(error) = register_macro_hotkey(&shared, &slot)
    {
        return fail(error);
    }
    lock(&shared.stored).macro_slot = slot;
    if shared.save().is_err() {
        // The slot is active in memory but would be lost at the next start.
        return fail("save_failed");
    }
    MacroSlotResult {
        ok: true,
        error: None,
    }
}

/// Result of the last operation for the settings UI: codes only (`null` before the first).
#[tauri::command]
fn last_operation(shared: Ctx<'_>) -> Option<feedback::LastOperation> {
    let mut last = *lock(&shared.last_outcome);
    if let Some(last) = last.as_mut() {
        // The hold expires on its own: report the current state, not the one at the end
        // of the operation.
        last.clipboard_held = shared.worker.get().is_some_and(os::clipboard_held);
    }
    last
}

fn setup(app: &AppHandle, shared: &Arc<Shared>, instance: os::Instance, background: bool) {
    // A second launch asks the running instance to show its window.
    let handle = app.clone();
    os::on_activate(&instance, move || {
        let inner = handle.clone();
        let _ = handle.run_on_main_thread(move || tray::show_window(&inner));
    });
    // `Instance` never closes its handles: the guard lives as long as the process.

    let for_config = Arc::downgrade(shared);
    let for_outcome = Arc::downgrade(shared);
    let for_macro = Arc::downgrade(shared);
    let worker = os::spawn_worker(
        move || {
            let shared = for_config.upgrade()?;
            if !matches!(shared.pause_state(), Pause::Active) {
                return None;
            }
            Some(ks_engine::Config {
                excluded: lock(&shared.stored).excluded_all(),
                ..ks_engine::Config::default()
            })
        },
        move || {
            let shared = for_macro.upgrade()?;
            lock(&shared.stored).macro_slot.to_macro()
        },
        move |outcome| {
            if let Some(shared) = for_outcome.upgrade() {
                let held = shared.worker.get().is_some_and(os::clipboard_held);
                let last = feedback::LastOperation::new(outcome, held);
                *lock(&shared.last_outcome) = Some(last);
                // Feedback only when something went wrong (no success notice).
                let language = lock(&shared.stored).ui_language;
                if let Some(text) = last.notice_code().and_then(|c| feedback::text(language, c)) {
                    os::notice(text);
                }
                tray::refresh(&shared);
            }
        },
    );
    if let Some(worker) = worker {
        let _ = shared.worker.set(worker);
    }
    let hotkey = lock(&shared.stored).hotkey.clone();
    // A conflict at start-up is shown as the tray status "needs attention".
    let _ = register_hotkey(shared, &hotkey);
    // A macro hotkey that cannot be registered shows as `registered: false` in the editor.
    let slot = lock(&shared.stored).macro_slot.clone();
    let _ = register_macro_hotkey(shared, &slot);
    tray::create(app, shared);
    if !background {
        tray::show_window(app);
    }
}

fn main() {
    os::harden();
    let Some(instance) = os::single_instance() else {
        return;
    };
    let background = std::env::args().any(|a| a == "--background");
    let path = settings::default_path();
    let shared = Arc::new(Shared {
        stored: Mutex::new(settings::load(path.as_deref())),
        path,
        pause: Mutex::new(Pause::Active),
        registration: Mutex::new(None),
        macro_registration: Mutex::new(None),
        worker: OnceLock::new(),
        tray: OnceLock::new(),
        last_outcome: Mutex::new(None),
    });
    let for_setup = Arc::clone(&shared);
    let mut instance = Some(instance);
    tauri::Builder::default()
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            core_version,
            build_signed,
            get_settings,
            set_autostart,
            set_ui_language,
            set_theme,
            set_hotkey,
            list_excluded_apps,
            add_excluded_app,
            remove_excluded_app,
            get_macro_slot,
            set_macro_slot,
            last_operation
        ])
        .setup(move |app| {
            if let Some(instance) = instance.take() {
                setup(app.handle(), &for_setup, instance, background);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window keeps KeySwitch running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run KeySwitch");
}
