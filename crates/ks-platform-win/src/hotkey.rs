//! `RegisterHotKey` on a dedicated thread with its own message loop (D-14; no hooks).

use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Foundation::{GetLastError, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_HOTKEY, WM_QUIT,
};

/// A thread that owns one registered hotkey and reports `WM_HOTKEY` deliveries.
#[derive(Debug)]
pub struct HotkeyThread {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
    /// Receives the hotkey id for each `WM_HOTKEY`.
    pub events: mpsc::Receiver<i32>,
}

/// Spawns a thread that calls `RegisterHotKey(NULL, id, mods | MOD_NOREPEAT, vk)`.
/// `Err(win32_error)` if registration failed (the thread has exited then).
pub fn spawn(id: i32, mods: HOT_KEY_MODIFIERS, vk: VIRTUAL_KEY) -> Result<HotkeyThread, u32> {
    let (ev_tx, ev_rx) = mpsc::channel::<i32>();
    let (thread_id, join) = spawn_inner(id, mods, vk, move |id| {
        let _ = ev_tx.send(id);
    })?;
    Ok(HotkeyThread {
        thread_id,
        join: Some(join),
        events: ev_rx,
    })
}

/// `MOD_ALT` for [`register`].
pub const MOD_ALT_BIT: u32 = 0x1;
/// `MOD_CONTROL` for [`register`].
pub const MOD_CONTROL_BIT: u32 = 0x2;
/// `MOD_SHIFT` for [`register`].
pub const MOD_SHIFT_BIT: u32 = 0x4;
/// `MOD_WIN` for [`register`].
pub const MOD_WIN_BIT: u32 = 0x8;
/// `ERROR_HOTKEY_ALREADY_REGISTERED`: another application owns the combination.
pub const ERROR_HOTKEY_ALREADY_REGISTERED: u32 = 1409;

/// A registered global hotkey; dropping it unregisters the hotkey and stops its thread.
#[derive(Debug)]
pub struct Registration {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

/// Registers a global hotkey (plain integers: `MOD_*_BIT` flags and a virtual-key code) and
/// calls `on_hotkey` on the hotkey thread for every press. `Err(win32_error)` on failure.
pub fn register(
    mods: u32,
    vk: u32,
    on_hotkey: impl Fn() + Send + 'static,
) -> Result<Registration, u32> {
    let vk = u16::try_from(vk).map_err(|_| u32::MAX)?;
    let (thread_id, join) = spawn_inner(1, HOT_KEY_MODIFIERS(mods), VIRTUAL_KEY(vk), move |_| {
        on_hotkey()
    })?;
    Ok(Registration {
        thread_id,
        join: Some(join),
    })
}

impl Drop for Registration {
    fn drop(&mut self) {
        stop(self.thread_id, &mut self.join);
    }
}

fn stop(thread_id: u32, join: &mut Option<JoinHandle<()>>) {
    // SAFETY: posts WM_QUIT to the hotkey thread's queue (created before registration).
    let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    if let Some(j) = join.take() {
        let _ = j.join();
    }
}

fn spawn_inner(
    id: i32,
    mods: HOT_KEY_MODIFIERS,
    vk: VIRTUAL_KEY,
    on_hotkey: impl Fn(i32) + Send + 'static,
) -> Result<(u32, JoinHandle<()>), u32> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, u32>>();
    let join = std::thread::Builder::new()
        .name(format!("ks-hotkey-{id}"))
        .spawn(move || {
            let mut msg = MSG::default();
            // SAFETY: forces creation of this thread's message queue so PostThreadMessageW
            // from the owner cannot be lost; `msg` is a local buffer.
            unsafe {
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            }
            // SAFETY: registers a thread hotkey (hwnd = NULL: WM_HOTKEY goes to this
            // thread's queue); unregistered below on the same thread.
            let reg = unsafe { RegisterHotKey(None, id, mods | MOD_NOREPEAT, u32::from(vk.0)) };
            if reg.is_err() {
                // SAFETY: reads the thread-local last error set by RegisterHotKey.
                let err = unsafe { GetLastError() }.0;
                let _ = ready_tx.send(Err(err));
                return;
            }
            // SAFETY: trivial query.
            let _ = ready_tx.send(Ok(unsafe { GetCurrentThreadId() }));
            loop {
                // SAFETY: `msg` is a valid local MSG buffer.
                let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
                if r == 0 || r == -1 || msg.message == WM_QUIT {
                    break;
                }
                if msg.message == WM_HOTKEY {
                    on_hotkey(msg.wParam.0 as i32);
                }
            }
            // SAFETY: unregisters the hotkey this thread registered.
            let _ = unsafe { UnregisterHotKey(None, id) };
        })
        .map_err(|_| u32::MAX)?;
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(thread_id)) => Ok((thread_id, join)),
        Ok(Err(e)) => {
            let _ = join.join();
            Err(e)
        }
        Err(_) => Err(u32::MAX),
    }
}

impl Drop for HotkeyThread {
    fn drop(&mut self) {
        stop(self.thread_id, &mut self.join);
    }
}
