//! Keyboard layouts (query + switch among ALREADY installed layouts; never load/unload)
//! and `RegisterHotKey` on a dedicated thread (D-14; no hooks).

use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Foundation::{GetLastError, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardLayoutList, HKL, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL,
    MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostMessageW, PostThreadMessageW, WM_HOTKEY,
    WM_INPUTLANGCHANGEREQUEST, WM_QUIT,
};

use super::sys::Hwnd;

/// HKL as an integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hkl(pub isize);

impl Hkl {
    /// LANGID (low word).
    pub fn lang_id(self) -> u16 {
        (self.0 as usize & 0xFFFF) as u16
    }
    /// Full HKL in hex (layout id high word, language low word) — metadata.
    pub fn hex(self) -> String {
        format!("{:08x}", self.0 as usize as u32)
    }
}

/// Active layout of a thread.
pub fn thread_layout(tid: u32) -> Hkl {
    // SAFETY: trivial query.
    Hkl(unsafe { GetKeyboardLayout(tid) }.0 as isize)
}

/// Layouts installed in this session (read-only).
pub fn installed_layouts() -> Vec<Hkl> {
    // SAFETY: first call returns the count, second fills a buffer of that size.
    unsafe {
        let n = GetKeyboardLayoutList(None).max(0) as usize;
        let mut v = vec![HKL::default(); n];
        let got = GetKeyboardLayoutList(Some(&mut v)).max(0) as usize;
        v.truncate(got);
        v.into_iter().map(|h| Hkl(h.0 as isize)).collect()
    }
}

/// Posts `WM_INPUTLANGCHANGEREQUEST(0, hkl)` to one of OUR windows.
pub fn request_switch(own_window: Hwnd, h: Hkl) -> bool {
    // SAFETY: documented message with integer parameters, posted to our own window.
    unsafe {
        PostMessageW(
            Some(own_window.raw()),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(h.0),
        )
    }
    .is_ok()
}

/// Ctrl+Alt+Shift (the kit's hotkey modifiers).
pub fn ctrl_alt_shift() -> HOT_KEY_MODIFIERS {
    MOD_CONTROL | MOD_ALT | MOD_SHIFT
}

/// Thread owning one registered hotkey.
#[derive(Debug)]
pub struct HotkeyThread {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
    /// One message per WM_HOTKEY.
    pub events: mpsc::Receiver<i32>,
}

/// Registers `mods | MOD_NOREPEAT + vk` on a new thread. `Err(win32 error)` on failure.
pub fn spawn_hotkey(
    id: i32,
    mods: HOT_KEY_MODIFIERS,
    vk: VIRTUAL_KEY,
) -> Result<HotkeyThread, u32> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, u32>>();
    let (ev_tx, ev_rx) = mpsc::channel::<i32>();
    let join = std::thread::Builder::new()
        .name(format!("ks-manual-hotkey-{id}"))
        .spawn(move || {
            let mut msg = MSG::default();
            // SAFETY: creates this thread's message queue; `msg` is local.
            unsafe {
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            }
            // SAFETY: thread hotkey (hwnd NULL); unregistered below on this thread.
            let reg = unsafe { RegisterHotKey(None, id, mods | MOD_NOREPEAT, u32::from(vk.0)) };
            if reg.is_err() {
                // SAFETY: thread-local last error of RegisterHotKey.
                let _ = ready_tx.send(Err(unsafe { GetLastError() }.0));
                return;
            }
            // SAFETY: trivial query.
            let _ = ready_tx.send(Ok(unsafe { GetCurrentThreadId() }));
            loop {
                // SAFETY: `msg` is a local buffer.
                let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
                if r == 0 || r == -1 || msg.message == WM_QUIT {
                    break;
                }
                if msg.message == WM_HOTKEY {
                    let _ = ev_tx.send(msg.wParam.0 as i32);
                }
            }
            // SAFETY: unregisters the hotkey this thread registered.
            let _ = unsafe { UnregisterHotKey(None, id) };
        })
        .map_err(|_| u32::MAX)?;
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(thread_id)) => Ok(HotkeyThread {
            thread_id,
            join: Some(join),
            events: ev_rx,
        }),
        Ok(Err(e)) => {
            let _ = join.join();
            Err(e)
        }
        Err(_) => Err(u32::MAX),
    }
}

impl Drop for HotkeyThread {
    fn drop(&mut self) {
        // SAFETY: posts WM_QUIT to the hotkey thread's queue.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
