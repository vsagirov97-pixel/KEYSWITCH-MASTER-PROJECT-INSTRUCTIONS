//! Single-instance guard: a named mutex decides who is first; a named auto-reset event lets a
//! second launch ask the first instance to show its window. Session-local names (`Local\`),
//! no window messages to other processes, no shared memory.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject,
};
use windows::core::PCWSTR;

/// Held by the first instance for the lifetime of the process.
#[derive(Debug)]
pub struct Instance {
    // Handle values as integers so the guard is `Send`; they are never closed: the mutex and
    // the event must live exactly as long as the process.
    _mutex: isize,
    event: isize,
}

/// Result of [`acquire`].
#[derive(Debug)]
pub enum Acquire {
    /// This process is the only instance.
    First(Instance),
    /// Another instance is running; it has been asked to show its window.
    Second,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Claims the single-instance role for `name` (an identifier such as `KeySwitch`).
/// If the objects cannot be created at all, the process is treated as first (the app must
/// still start).
pub fn acquire(name: &str) -> Acquire {
    let mutex_name = wide(&format!("Local\\{name}.instance"));
    let event_name = wide(&format!("Local\\{name}.activate"));
    // SAFETY: the names are NUL-terminated and outlive the calls; default security; the last
    // error is read immediately after CreateMutexW on the same thread.
    let (mutex, exists, event) = unsafe {
        let mutex = CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr()));
        let exists = GetLastError() == ERROR_ALREADY_EXISTS;
        let event = CreateEventW(None, false, false, PCWSTR(event_name.as_ptr()));
        (mutex, exists, event)
    };
    let event = event.map_or(0, |h| h.0 as isize);
    match mutex {
        Ok(_) if exists => {
            if event != 0 {
                // SAFETY: `event` is a live event handle created/opened above.
                let _ = unsafe { SetEvent(HANDLE(event as *mut core::ffi::c_void)) };
            }
            Acquire::Second
        }
        Ok(m) => Acquire::First(Instance {
            _mutex: m.0 as isize,
            event,
        }),
        Err(_) => Acquire::First(Instance { _mutex: 0, event }),
    }
}

impl Instance {
    /// Calls `on_activate` (on a background thread) every time another launch is attempted.
    pub fn on_activate(&self, on_activate: impl Fn() + Send + 'static) {
        let event = self.event;
        if event == 0 {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("ks-instance".into())
            .spawn(move || {
                loop {
                    // SAFETY: `event` is a live event handle that is never closed.
                    let r = unsafe {
                        WaitForSingleObject(HANDLE(event as *mut core::ffi::c_void), INFINITE)
                    };
                    if r != WAIT_OBJECT_0 {
                        break;
                    }
                    on_activate();
                }
            });
    }
}
