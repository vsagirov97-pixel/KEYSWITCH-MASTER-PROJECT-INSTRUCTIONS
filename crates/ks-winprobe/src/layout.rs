//! Keyboard layout query and switch via `WM_INPUTLANGCHANGEREQUEST`.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ACTIVATE_KEYBOARD_LAYOUT_FLAGS, GetKeyboardLayout, GetKeyboardLayoutList, HKL,
    LoadKeyboardLayoutW, UnloadKeyboardLayout,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_INPUTLANGCHANGEREQUEST};
use windows::core::PCWSTR;

use crate::fixture::Hwnd;

/// HKL as an integer (thread-safe to pass around).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hkl(pub isize);

impl Hkl {
    /// Low word = language id (0x0409 en-US, 0x0419 ru-RU).
    pub fn lang_id(self) -> u16 {
        (self.0 as usize & 0xFFFF) as u16
    }
    fn raw(self) -> HKL {
        HKL(self.0 as *mut core::ffi::c_void)
    }
}

/// Active layout of a thread.
pub fn thread_layout(thread_id: u32) -> Hkl {
    // SAFETY: trivial query; an unknown thread id yields 0.
    Hkl(unsafe { GetKeyboardLayout(thread_id) }.0 as isize)
}

/// Layouts loaded in this session.
pub fn loaded_layouts() -> Vec<Hkl> {
    // SAFETY: first call with None returns the count; second call fills a buffer of that size.
    unsafe {
        let n = GetKeyboardLayoutList(None).max(0) as usize;
        let mut v = vec![HKL::default(); n];
        let got = GetKeyboardLayoutList(Some(&mut v)).max(0) as usize;
        v.truncate(got);
        v.into_iter().map(|h| Hkl(h.0 as isize)).collect()
    }
}

/// `LoadKeyboardLayoutW(klid)` without activating it. `klid` like `"00000419"`.
pub fn load(klid: &str) -> Option<Hkl> {
    let wide: Vec<u16> = klid.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is NUL-terminated and outlives the call; flags 0 = load, do not activate.
    unsafe { LoadKeyboardLayoutW(PCWSTR(wide.as_ptr()), ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0)) }
        .ok()
        .map(|h| Hkl(h.0 as isize))
}

/// Unloads a layout we loaded ourselves.
pub fn unload(h: Hkl) -> bool {
    // SAFETY: `h` is a layout handle returned by LoadKeyboardLayoutW in this test.
    unsafe { UnloadKeyboardLayout(h.raw()) }.is_ok()
}

/// Posts `WM_INPUTLANGCHANGEREQUEST(0, hkl)` to `target` (as KeySwitch will, to the target's
/// foreground/focus window).
pub fn request_switch(target: Hwnd, h: Hkl) -> bool {
    // SAFETY: posting a documented message with integer parameters to our own window.
    unsafe {
        PostMessageW(
            Some(target.raw()),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(h.0),
        )
    }
    .is_ok()
}
