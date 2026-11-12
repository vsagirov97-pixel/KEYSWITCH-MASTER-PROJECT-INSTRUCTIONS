//! The shell's whole view of the operating system. On Windows it forwards to
//! `ks-platform-win` (the only crate with `unsafe`); elsewhere every function is an inert
//! stub so the crate still compiles (the macOS backend is a separate work stream).

use ks_engine::{Config, Outcome};

/// Why `RegisterHotKey` failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyFailure {
    /// Another application owns the combination.
    Conflict,
    /// Any other failure.
    Other,
}

/// Name of the `Run` value and of the single-instance objects.
const APP_NAME: &str = "KeySwitch";

#[cfg(windows)]
mod imp {
    use super::*;
    use ks_platform_win::{adapter, autostart, hotkey, instance, process, tray};

    pub type Tray = tray::Tray;
    pub type TrayEntry = tray::Entry;
    pub type TrayEvent = tray::Event;
    pub type TrayModel = tray::Model;

    pub fn spawn_tray(model: TrayModel, f: impl Fn(TrayEvent) + Send + 'static) -> Option<Tray> {
        tray::spawn(model, f)
    }

    pub fn set_tray(tray: &Tray, model: TrayModel) {
        tray.set_model(model);
    }

    pub type Worker = adapter::Worker;
    pub type Registration = hotkey::Registration;
    pub type Instance = instance::Instance;

    pub fn harden() {
        process::harden();
    }

    pub fn single_instance() -> Option<Instance> {
        match instance::acquire(APP_NAME) {
            instance::Acquire::First(i) => Some(i),
            instance::Acquire::Second => None,
        }
    }

    pub fn on_activate(instance: &Instance, f: impl Fn() + Send + 'static) {
        instance.on_activate(f);
    }

    pub fn spawn_worker(
        config: impl Fn() -> Option<Config> + Send + 'static,
        on_outcome: impl Fn(Outcome) + Send + 'static,
    ) -> Option<Worker> {
        let converter = ks_core::builtin().ok()?;
        adapter::spawn_worker(converter, config, on_outcome)
    }

    pub fn trigger(worker: &Worker) {
        worker.trigger();
    }

    pub fn cancel(worker: &Worker) {
        worker.cancel();
    }

    pub fn register_hotkey(
        mods: u32,
        vk: u32,
        f: impl Fn() + Send + 'static,
    ) -> Result<Registration, HotkeyFailure> {
        hotkey::register(mods, vk, f).map_err(|code| {
            if code == hotkey::ERROR_HOTKEY_ALREADY_REGISTERED {
                HotkeyFailure::Conflict
            } else {
                HotkeyFailure::Other
            }
        })
    }

    pub fn autostart_enabled() -> bool {
        autostart::is_enabled(APP_NAME)
    }

    pub fn set_autostart(enabled: bool) -> Result<(), String> {
        let command = if enabled {
            let exe = std::env::current_exe().map_err(|_| "executable path is unavailable")?;
            Some(format!("\"{}\" --background", exe.display()))
        } else {
            None
        };
        autostart::set(APP_NAME, command.as_deref())
            .map_err(|code| format!("autostart entry could not be changed (error {code})"))
    }

    pub fn local_hour_minute() -> (u16, u16) {
        process::local_hour_minute()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub struct Worker;
    pub struct Registration;
    pub struct Instance;
    pub struct Tray;

    #[derive(Clone, Debug)]
    pub enum TrayEntry {
        Item {
            id: u32,
            label: String,
            enabled: bool,
            default: bool,
        },
        Separator,
        Submenu {
            label: String,
            items: Vec<TrayEntry>,
        },
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TrayEvent {
        LeftClick,
        Command(u32),
    }

    #[derive(Clone, Debug, Default)]
    pub struct TrayModel {
        pub tooltip: String,
        pub menu: Vec<TrayEntry>,
    }

    pub fn spawn_tray(_model: TrayModel, _f: impl Fn(TrayEvent) + Send + 'static) -> Option<Tray> {
        None
    }
    pub fn set_tray(_tray: &Tray, _model: TrayModel) {}

    pub fn harden() {}
    pub fn single_instance() -> Option<Instance> {
        Some(Instance)
    }
    pub fn on_activate(_instance: &Instance, _f: impl Fn() + Send + 'static) {}
    pub fn spawn_worker(
        _config: impl Fn() -> Option<Config> + Send + 'static,
        _on_outcome: impl Fn(Outcome) + Send + 'static,
    ) -> Option<Worker> {
        None
    }
    pub fn trigger(_worker: &Worker) {}
    pub fn cancel(_worker: &Worker) {}
    pub fn register_hotkey(
        _mods: u32,
        _vk: u32,
        _f: impl Fn() + Send + 'static,
    ) -> Result<Registration, HotkeyFailure> {
        Err(HotkeyFailure::Other)
    }
    pub fn autostart_enabled() -> bool {
        false
    }
    pub fn set_autostart(_enabled: bool) -> Result<(), String> {
        Err(format!(
            "{APP_NAME}: autostart is not available on this platform"
        ))
    }
    pub fn local_hour_minute() -> (u16, u16) {
        (0, 0)
    }
}

pub use imp::*;
