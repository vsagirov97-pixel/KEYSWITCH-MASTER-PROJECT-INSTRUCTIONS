//! Notification-area (tray) icon with a popup menu, on its own thread with a hidden window.
//!
//! Plain `Shell_NotifyIconW` + `TrackPopupMenu`: no extra crates. The menu is rebuilt from a
//! data model every time it is opened; events are reported through a callback that runs on
//! the tray thread (it must only hand work over to another thread).

use std::cell::RefCell;
use std::sync::{Arc, Mutex, mpsc};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetMessageW, HICON, HMENU, IDI_APPLICATION, LoadIconW, MF_DISABLED, MF_GRAYED,
    MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SetForegroundWindow, SetMenuDefaultItem, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_CLOSE,
    WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{PCWSTR, w};

/// One entry of the popup menu.
#[derive(Clone, Debug)]
pub enum Entry {
    /// A command. `id` must be non-zero.
    Item {
        /// Command id reported in [`Event::Command`].
        id: u32,
        /// Visible label.
        label: String,
        /// Disabled entries are shown greyed (used for information lines).
        enabled: bool,
        /// The default entry is shown in bold.
        default: bool,
    },
    /// A separator line.
    Separator,
    /// A nested menu.
    Submenu {
        /// Visible label.
        label: String,
        /// Entries of the nested menu.
        items: Vec<Entry>,
    },
}

/// What the user did with the tray icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Left click on the icon.
    LeftClick,
    /// A menu command was chosen.
    Command(u32),
}

/// Tooltip and menu shown by the icon.
#[derive(Clone, Debug, Default)]
pub struct Model {
    /// Tooltip (truncated to 127 UTF-16 units).
    pub tooltip: String,
    /// Popup menu.
    pub menu: Vec<Entry>,
}

/// Handle of the tray icon; dropping it removes the icon.
#[derive(Debug)]
pub struct Tray {
    hwnd: isize,
    model: Arc<Mutex<Model>>,
}

const WM_TRAY: u32 = WM_APP + 1;
const WM_TRAY_REFRESH: u32 = WM_APP + 2;
const ICON_ID: u32 = 1;
/// Resource id under which tauri-build embeds the application icon.
const APP_ICON_RESOURCE: usize = 32512;

struct State {
    model: Arc<Mutex<Model>>,
    on_event: Box<dyn Fn(Event)>,
    taskbar_created: u32,
    icon: HICON,
}

thread_local! {
    /// State of the tray window that lives on this thread (one tray per thread).
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn lock(m: &Mutex<Model>) -> std::sync::MutexGuard<'_, Model> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn icon_data(hwnd: HWND, icon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    // Keep the last unit as the terminating NUL.
    let limit = data.szTip.len() - 1;
    for (slot, unit) in data
        .szTip
        .iter_mut()
        .zip(tooltip.encode_utf16().take(limit))
    {
        *slot = unit;
    }
    data
}

fn notify(hwnd: HWND, add: bool) {
    STATE.with_borrow(|state| {
        if let Some(state) = state {
            let data = icon_data(hwnd, state.icon, &lock(&state.model).tooltip);
            // SAFETY: `data` is a fully initialised NOTIFYICONDATAW with its size set.
            let _ = unsafe { Shell_NotifyIconW(if add { NIM_ADD } else { NIM_MODIFY }, &data) };
        }
    });
}

/// Appends `entries` to `menu`. Nested menus are owned by their parent once appended.
fn fill_menu(menu: HMENU, entries: &[Entry]) {
    for entry in entries {
        match entry {
            Entry::Separator => {
                // SAFETY: `menu` is a live menu handle; separators carry no data.
                let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };
            }
            Entry::Item {
                id,
                label,
                enabled,
                default,
            } => {
                let text = wide(label);
                let flags = if *enabled {
                    MF_STRING
                } else {
                    MF_STRING | MF_GRAYED | MF_DISABLED
                };
                // SAFETY: `text` is NUL-terminated and outlives the call (the menu copies it).
                unsafe {
                    let _ = AppendMenuW(menu, flags, *id as usize, PCWSTR(text.as_ptr()));
                    if *default {
                        let _ = SetMenuDefaultItem(menu, *id, 0);
                    }
                }
            }
            Entry::Submenu { label, items } => {
                // SAFETY: creates an empty popup menu; ownership passes to `menu` below.
                let Ok(sub) = (unsafe { CreatePopupMenu() }) else {
                    continue;
                };
                fill_menu(sub, items);
                let text = wide(label);
                // SAFETY: `sub` is a live menu handle passed as the item id, as MF_POPUP
                // requires; `text` is NUL-terminated and outlives the call.
                let appended =
                    unsafe { AppendMenuW(menu, MF_POPUP, sub.0 as usize, PCWSTR(text.as_ptr())) };
                if appended.is_err() {
                    // SAFETY: the submenu was not attached, so it is still ours to destroy.
                    let _ = unsafe { DestroyMenu(sub) };
                }
            }
        }
    }
}

/// Shows the popup menu at the cursor; returns the chosen command id (0 = none).
fn show_menu(hwnd: HWND, entries: &[Entry]) -> u32 {
    // SAFETY: the menu is created, shown and destroyed on this thread; `pt` is a valid
    // out-parameter. SetForegroundWindow on our own window and the WM_NULL afterwards are
    // the documented way to make a tray menu close when the user clicks elsewhere.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return 0;
        };
        fill_menu(menu, entries);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        u32::try_from(chosen.0).unwrap_or(0)
    }
}

fn emit(event: Event) {
    STATE.with_borrow(|state| {
        if let Some(state) = state {
            (state.on_event)(event);
        }
    });
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taskbar_created = STATE.with_borrow(|s| s.as_ref().map_or(0, |s| s.taskbar_created));
    match msg {
        WM_TRAY => {
            // With the classic callback format the mouse message is in the low word.
            match (lparam.0 as u32) & 0xFFFF {
                WM_LBUTTONUP => emit(Event::LeftClick),
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    let entries = STATE.with_borrow(|s| {
                        s.as_ref()
                            .map(|s| lock(&s.model).menu.clone())
                            .unwrap_or_default()
                    });
                    let chosen = show_menu(hwnd, &entries);
                    if chosen != 0 {
                        emit(Event::Command(chosen));
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TRAY_REFRESH => {
            notify(hwnd, false);
            LRESULT(0)
        }
        WM_DESTROY => {
            let data = icon_data(hwnd, HICON::default(), "");
            // SAFETY: removes the icon this window added; then ends this thread's loop.
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        // Explorer restarted: the icon must be added again.
        m if m == taskbar_created && m != 0 => {
            notify(hwnd, true);
            LRESULT(0)
        }
        // SAFETY: default handling for every other message of our own window.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Creates the tray icon on a new thread. `on_event` runs on that thread and must return
/// quickly. `None` if the window or the thread could not be created.
pub fn spawn(model: Model, on_event: impl Fn(Event) + Send + 'static) -> Option<Tray> {
    let model = Arc::new(Mutex::new(model));
    let thread_model = Arc::clone(&model);
    let (ready_tx, ready_rx) = mpsc::channel::<isize>();
    std::thread::Builder::new()
        .name("ks-tray".into())
        .spawn(move || {
            let class = w!("KeySwitchTrayWindow");
            // SAFETY: registers a window class with our window procedure and creates a
            // hidden top-level window owned by this thread; all strings are constants.
            let (hwnd, icon, taskbar_created) = unsafe {
                let module = GetModuleHandleW(None).unwrap_or_default();
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: module.into(),
                    lpszClassName: class,
                    ..Default::default()
                };
                RegisterClassW(&wc);
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    class,
                    w!("KeySwitch"),
                    WS_OVERLAPPED,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(module.into()),
                    None,
                );
                // The application icon embedded in the executable; the stock application
                // icon if the resource is missing.
                let icon = LoadIconW(Some(module.into()), PCWSTR(APP_ICON_RESOURCE as *const u16))
                    .or_else(|_| LoadIconW(None, IDI_APPLICATION))
                    .unwrap_or_default();
                (hwnd, icon, RegisterWindowMessageW(w!("TaskbarCreated")))
            };
            let Ok(hwnd) = hwnd else {
                let _ = ready_tx.send(0);
                return;
            };
            STATE.set(Some(State {
                model: thread_model,
                on_event: Box::new(on_event),
                taskbar_created,
                icon,
            }));
            notify(hwnd, true);
            let _ = ready_tx.send(hwnd.0 as isize);
            let mut msg = MSG::default();
            // SAFETY: standard message loop for this thread's window; `msg` is a local buffer.
            unsafe {
                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            STATE.set(None);
        })
        .ok()?;
    match ready_rx.recv() {
        Ok(hwnd) if hwnd != 0 => Some(Tray { hwnd, model }),
        _ => None,
    }
}

impl Tray {
    fn post(&self, msg: u32) {
        // SAFETY: posts an integer-only message to our own tray window.
        let _ = unsafe {
            PostMessageW(
                Some(HWND(self.hwnd as *mut core::ffi::c_void)),
                msg,
                WPARAM(0),
                LPARAM(0),
            )
        };
    }

    /// Replaces the tooltip and the menu.
    pub fn set_model(&self, model: Model) {
        *lock(&self.model) = model;
        self.post(WM_TRAY_REFRESH);
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.post(WM_CLOSE);
    }
}
