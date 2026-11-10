//! Hand-written FFI to Carbon/HIToolbox: `RegisterEventHotKey`, Text Input Sources (TIS)
//! and `IsSecureEventInputEnabled`. `objc2-carbon` has no bindings for these.
//!
//! Apple docs (headers `CarbonEvents.h`, `TextInputSources.h`):
//! <https://developer.apple.com/documentation/carbon/1428891-registereventhotkey> [reported URL]
//! <https://developer.apple.com/documentation/carbon/text_input_source_services> [reported URL]
//!
//! Hotkeys are *registered* combinations delivered by the system; nothing here observes
//! the keyboard stream (D-27).

use std::ffi::c_void;
use std::ptr::{self, NonNull};

use objc2_core_foundation::{CFArray, CFBoolean, CFRetained, CFString, CFType};

type OSStatus = i32;
type EventTargetRef = *mut c_void;
type EventHotKeyRef = *mut c_void;
type EventHandlerRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventRef = *mut c_void;
type EventHandlerUPP = extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;

#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> EventTargetRef;
    fn RegisterEventHotKey(
        code: u32,
        modifiers: u32,
        id: EventHotKeyID,
        target: EventTargetRef,
        options: u32,
        out: *mut EventHotKeyRef,
    ) -> OSStatus;
    fn UnregisterEventHotKey(hotkey: EventHotKeyRef) -> OSStatus;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: EventHandlerUPP,
        num_types: usize, // ItemCount = unsigned long
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        out: *mut EventHandlerRef,
    ) -> OSStatus;

    fn IsSecureEventInputEnabled() -> u8;

    fn TISCopyCurrentKeyboardInputSource() -> Option<NonNull<CFType>>;
    fn TISCreateInputSourceList(
        properties: *const c_void,
        include_all_installed: u8,
    ) -> Option<NonNull<CFArray<CFType>>>;
    fn TISGetInputSourceProperty(source: &CFType, key: &CFString) -> *const CFType;
    fn TISSelectInputSource(source: &CFType) -> OSStatus;
    fn TISEnableInputSource(source: &CFType) -> OSStatus;
    fn TISDisableInputSource(source: &CFType) -> OSStatus;

    static kTISPropertyInputSourceID: &'static CFString;
    static kTISPropertyInputSourceCategory: &'static CFString;
    static kTISPropertyInputSourceIsSelectCapable: &'static CFString;
    static kTISPropertyInputSourceIsEnabled: &'static CFString;
    static kTISCategoryKeyboardInputSource: &'static CFString;
}

/// Carbon modifier masks (`Events.h`).
pub mod mods {
    pub const CMD: u32 = 1 << 8;
    pub const SHIFT: u32 = 1 << 9;
    pub const OPTION: u32 = 1 << 11;
    pub const CONTROL: u32 = 1 << 12;
}

/// `eventHotKeyExistsErr`.
pub const HOTKEY_EXISTS_ERR: OSStatus = -9878;

const SIGNATURE: u32 = u32::from_be_bytes(*b"KSPB");
const K_EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const K_EVENT_HOT_KEY_PRESSED: u32 = 5;

/// `IsSecureEventInputEnabled` (some process has Secure Event Input on).
pub fn secure_event_input_enabled() -> bool {
    // SAFETY: no arguments, read-only query.
    unsafe { IsSecureEventInputEnabled() != 0 }
}

/// A registered hotkey; unregistered on drop.
pub struct HotKey(EventHotKeyRef);

impl Drop for HotKey {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from a successful RegisterEventHotKey and is dropped once.
        unsafe {
            UnregisterEventHotKey(self.0);
        }
    }
}

/// Registers `keycode`+`modifiers` on the application event target.
pub fn register_hotkey(keycode: u32, modifiers: u32, id: u32) -> Result<HotKey, OSStatus> {
    let mut out: EventHotKeyRef = ptr::null_mut();
    // SAFETY: valid out-pointer; the application target lives for the process lifetime.
    let st = unsafe {
        RegisterEventHotKey(
            keycode,
            modifiers,
            EventHotKeyID {
                signature: SIGNATURE,
                id,
            },
            GetApplicationEventTarget(),
            0,
            &mut out,
        )
    };
    if st == 0 && !out.is_null() {
        Ok(HotKey(out))
    } else {
        Err(st)
    }
}

/// Installs a hotkey-pressed handler on the application target. `on_press` runs on the
/// main thread when any of our hotkeys fires (requires a running event loop).
pub fn install_hotkey_handler(on_press: extern "C" fn()) -> OSStatus {
    extern "C" fn handler(_: EventHandlerCallRef, _: EventRef, user: *mut c_void) -> OSStatus {
        // SAFETY: `user` is the `extern "C" fn()` pointer passed at installation.
        let f: extern "C" fn() =
            unsafe { std::mem::transmute::<*mut c_void, extern "C" fn()>(user) };
        f();
        0
    }
    let spec = EventTypeSpec {
        event_class: K_EVENT_CLASS_KEYBOARD,
        event_kind: K_EVENT_HOT_KEY_PRESSED,
    };
    let mut out: EventHandlerRef = ptr::null_mut();
    // SAFETY: `spec` outlives the call (copied by Carbon); `user` is a plain fn pointer.
    unsafe {
        InstallEventHandler(
            GetApplicationEventTarget(),
            handler,
            1,
            &spec,
            on_press as *mut c_void,
            &mut out,
        )
    }
}

/// A Text Input Source (owned reference).
pub struct InputSource(CFRetained<CFType>);

fn string_prop(src: &CFType, key: &CFString) -> Option<String> {
    // SAFETY: TISGetInputSourceProperty follows the Get rule; the value lives as long as `src`.
    let p = unsafe { TISGetInputSourceProperty(src, key) };
    // SAFETY: non-null pointer to a CF object owned by `src`.
    let v = unsafe { p.as_ref() }?;
    v.downcast_ref::<CFString>().map(|s| s.to_string())
}

fn bool_prop(src: &CFType, key: &CFString) -> Option<bool> {
    // SAFETY: as in `string_prop`.
    let p = unsafe { TISGetInputSourceProperty(src, key) };
    // SAFETY: as in `string_prop`.
    let v = unsafe { p.as_ref() }?;
    v.downcast_ref::<CFBoolean>().map(|b| b.as_bool())
}

impl InputSource {
    /// Input source ID, e.g. `com.apple.keylayout.US` (metadata, not user text).
    pub fn id(&self) -> String {
        // SAFETY: reading an immutable framework constant.
        string_prop(&self.0, unsafe { kTISPropertyInputSourceID }).unwrap_or_default()
    }

    pub fn is_keyboard(&self) -> bool {
        // SAFETY: reading immutable framework constants.
        let (k, c) = unsafe {
            (
                kTISPropertyInputSourceCategory,
                kTISCategoryKeyboardInputSource,
            )
        };
        string_prop(&self.0, k).is_some_and(|v| v == c.to_string())
    }

    pub fn is_select_capable(&self) -> bool {
        // SAFETY: reading an immutable framework constant.
        bool_prop(&self.0, unsafe { kTISPropertyInputSourceIsSelectCapable }).unwrap_or(false)
    }

    pub fn is_enabled(&self) -> bool {
        // SAFETY: reading an immutable framework constant.
        bool_prop(&self.0, unsafe { kTISPropertyInputSourceIsEnabled }).unwrap_or(false)
    }

    pub fn select(&self) -> OSStatus {
        // SAFETY: valid input source reference.
        unsafe { TISSelectInputSource(&self.0) }
    }

    pub fn enable(&self) -> OSStatus {
        // SAFETY: valid input source reference.
        unsafe { TISEnableInputSource(&self.0) }
    }

    pub fn disable(&self) -> OSStatus {
        // SAFETY: valid input source reference.
        unsafe { TISDisableInputSource(&self.0) }
    }
}

/// Current keyboard input source. Must be called on the main thread.
pub fn current_keyboard_source() -> Option<InputSource> {
    // SAFETY: Copy rule: we own the returned +1 reference.
    let p = unsafe { TISCopyCurrentKeyboardInputSource() }?;
    // SAFETY: `p` is a +1 CF reference.
    Some(InputSource(unsafe { CFRetained::from_raw(p) }))
}

/// Enabled (or all installed) input sources. Must be called on the main thread.
pub fn input_sources(include_all_installed: bool) -> Vec<InputSource> {
    // SAFETY: NULL filter = no filtering; Create rule: we own the array.
    let Some(p) = (unsafe { TISCreateInputSourceList(ptr::null(), include_all_installed as u8) })
    else {
        return Vec::new();
    };
    // SAFETY: `p` is a +1 CFArray of TISInputSourceRef (CF objects).
    let arr = unsafe { CFRetained::from_raw(p) };
    (0..arr.len())
        .filter_map(|i| arr.get(i))
        .map(InputSource)
        .collect()
}
