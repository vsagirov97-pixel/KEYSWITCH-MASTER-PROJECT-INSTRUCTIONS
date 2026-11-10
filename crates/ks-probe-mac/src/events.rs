//! Keyboard event *synthesis* (method M3) with `CGEventKeyboardSetUnicodeString`.
//!
//! This posts events; it never observes them. No event-tap creation, no monitors (D-27).
//! `CGEventTapLocation::HIDEventTap` below is only the *injection point* constant of
//! `CGEventPost`, not an event tap.
//!
//! Apple docs:
//! <https://developer.apple.com/documentation/coregraphics/cgevent/keyboardsetunicodestring(stringlength:unicodestring:)>
//! <https://developer.apple.com/documentation/coregraphics/cgevent/posttopid(_:)>

use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGPreflightListenEventAccess, CGPreflightPostEventAccess,
};

/// Marker stored in `kCGEventSourceUserData` of every event we post, so our own
/// synthetic events are identifiable (future loop-guard; value is arbitrary).
pub const KS_EVENT_TAG: i64 = 0x4B53_5052_4F42; // "KSPROB"

/// Where to deliver synthesized events.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Delivery {
    /// `CGEventPostToPid`: straight to the target process.
    Pid(i32),
    /// `CGEventPost(kCGHIDEventTap, …)`: system input stream, goes to the key window.
    Hid,
}

impl Delivery {
    pub fn name(self) -> &'static str {
        match self {
            Delivery::Pid(_) => "post_to_pid",
            Delivery::Hid => "post_hid",
        }
    }
}

/// `CGPreflightPostEventAccess` (diagnostic only, never requests).
pub fn can_post() -> bool {
    CGPreflightPostEventAccess()
}

/// `CGPreflightListenEventAccess` — Input Monitoring status, recorded for diagnostics only.
/// The probe never requests it and never needs it (D-27).
pub fn listen_access_preflight() -> bool {
    CGPreflightListenEventAccess()
}

fn source() -> Option<CFRetained<CGEventSource>> {
    // Private state so the physical modifier state of the session does not leak in.
    CGEventSource::new(CGEventSourceStateID::Private)
}

fn post(delivery: Delivery, ev: &CGEvent) {
    CGEvent::set_integer_value_field(Some(ev), CGEventField::EventSourceUserData, KS_EVENT_TAG);
    match delivery {
        Delivery::Pid(pid) => CGEvent::post_to_pid(pid, Some(ev)),
        Delivery::Hid => CGEvent::post(CGEventTapLocation::HIDEventTap, Some(ev)),
    }
}

/// Splits UTF-16 into chunks of at most `max` units without splitting surrogate pairs.
pub fn chunk_utf16(units: &[u16], max: usize) -> Vec<&[u16]> {
    let max = max.max(1);
    let mut out = Vec::new();
    let mut i = 0;
    while i < units.len() {
        let mut end = (i + max).min(units.len());
        // Do not end a chunk on a high surrogate: shrink, or grow if the pair is alone.
        if end < units.len() && (0xD800..0xDC00).contains(&units[end - 1]) {
            if end - 1 > i { end -= 1 } else { end += 1 }
        }
        out.push(&units[i..end]);
        i = end;
    }
    out
}

/// Types `text` as Unicode keyboard events, `chunk` UTF-16 units per key-down/up pair.
/// Returns the number of key-down events posted, or `None` if an event could not be created.
pub fn type_unicode(delivery: Delivery, text: &str, chunk: usize, pause_us: u64) -> Option<usize> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let src = source();
    let mut n = 0;
    for part in chunk_utf16(&units, chunk) {
        for down in [true, false] {
            let ev = CGEvent::new_keyboard_event(src.as_deref(), 0, down)?;
            CGEvent::set_flags(Some(&ev), CGEventFlags::empty());
            // SAFETY: `part` is a live UTF-16 slice; length matches the pointer's extent.
            unsafe {
                CGEvent::keyboard_set_unicode_string(Some(&ev), part.len() as _, part.as_ptr());
            }
            post(delivery, &ev);
        }
        n += 1;
        if pause_us > 0 {
            std::thread::sleep(std::time::Duration::from_micros(pause_us));
        }
    }
    Some(n)
}

/// Posts one key chord (e.g. Cmd+Z) as key-down/key-up with the given flags.
pub fn key_chord(delivery: Delivery, keycode: u16, flags: CGEventFlags) -> bool {
    let src = source();
    for down in [true, false] {
        let Some(ev) = CGEvent::new_keyboard_event(src.as_deref(), keycode, down) else {
            return false;
        };
        CGEvent::set_flags(Some(&ev), flags);
        post(delivery, &ev);
    }
    true
}

/// macOS virtual key codes (`Events.h`, `kVK_*`) used by the probe.
pub mod vk {
    pub const ANSI_Z: u16 = 0x06;
    pub const ANSI_K: u16 = 0x28;
    pub const ANSI_J: u16 = 0x26;
    pub const ANSI_1: u16 = 0x12;
}

#[cfg(test)]
mod tests {
    use super::chunk_utf16;

    #[test]
    fn chunks_keep_surrogate_pairs() {
        let s: Vec<u16> = "ab\u{1F600}cd".encode_utf16().collect();
        for max in 1..6 {
            for c in chunk_utf16(&s, max) {
                assert!(
                    String::from_utf16(c).is_ok(),
                    "chunk split a surrogate pair"
                );
            }
        }
    }
}
