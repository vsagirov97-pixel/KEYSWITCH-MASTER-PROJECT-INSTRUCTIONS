//! Thin, safe-to-call wrappers over the Accessibility (AXUIElement) C API.
//!
//! Apple docs: <https://developer.apple.com/documentation/applicationservices/axuielement_h>

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{AXError, AXIsProcessTrusted, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFBoolean, CFNumber, CFRange, CFRetained, CFString, CFType};

use crate::text::ProbeText;

/// AX error code (`AXError` raw value; 0 = success).
pub type AxCode = i32;

/// Attribute names (plain strings; the C headers define them as `CFSTR` macros).
pub mod attr {
    pub const FOCUSED_UI_ELEMENT: &str = "AXFocusedUIElement";
    pub const FOCUSED_APPLICATION: &str = "AXFocusedApplication";
    pub const FRONTMOST: &str = "AXFrontmost";
    pub const FOCUSED: &str = "AXFocused";
    pub const ROLE: &str = "AXRole";
    pub const SUBROLE: &str = "AXSubrole";
    pub const VALUE: &str = "AXValue";
    pub const SELECTED_TEXT: &str = "AXSelectedText";
    pub const SELECTED_TEXT_RANGE: &str = "AXSelectedTextRange";
    pub const NUMBER_OF_CHARACTERS: &str = "AXNumberOfCharacters";
}

/// `kAXSecureTextFieldSubrole`.
pub const SECURE_SUBROLE: &str = "AXSecureTextField";

/// Whether this process is a trusted accessibility client. Never prompts.
pub fn is_trusted() -> bool {
    // SAFETY: no arguments; reads TCC state for the current process.
    unsafe { AXIsProcessTrusted() }
}

/// An owned AXUIElement reference.
pub struct Element(CFRetained<AXUIElement>);

fn code(e: AXError) -> AxCode {
    e.0
}

impl Element {
    pub fn system_wide() -> Self {
        // SAFETY: returns a new +1 reference; no preconditions.
        Self(unsafe { AXUIElement::new_system_wide() })
    }

    pub fn application(pid: i32) -> Self {
        // SAFETY: returns a new +1 reference; an invalid pid only yields errors later.
        Self(unsafe { AXUIElement::new_application(pid) })
    }

    /// Sets the per-element messaging timeout so a hung target cannot hang the probe.
    pub fn set_timeout(&self, secs: f32) -> AxCode {
        // SAFETY: `self.0` is a valid element.
        code(unsafe { self.0.set_messaging_timeout(secs) })
    }

    pub fn pid(&self) -> Result<i32, AxCode> {
        let mut pid: i32 = 0;
        // SAFETY: `pid` is a valid out-pointer for the duration of the call.
        let e = unsafe { self.0.pid(NonNull::from(&mut pid)) };
        if e == AXError::Success {
            Ok(pid)
        } else {
            Err(code(e))
        }
    }

    /// Raw attribute copy (+1 reference wrapped in `CFRetained`).
    pub fn attr(&self, name: &str) -> Result<CFRetained<CFType>, AxCode> {
        let name = CFString::from_str(name);
        let mut out: *const CFType = std::ptr::null();
        // SAFETY: `out` is a valid out-pointer; on success it holds a +1 reference.
        let e = unsafe { self.0.copy_attribute_value(&name, NonNull::from(&mut out)) };
        if e != AXError::Success {
            return Err(code(e));
        }
        match NonNull::new(out.cast_mut()) {
            // SAFETY: AXUIElementCopyAttributeValue follows the Copy rule (+1), so we own it.
            Some(p) => Ok(unsafe { CFRetained::from_raw(p) }),
            None => Err(code(AXError::NoValue)),
        }
    }

    pub fn element_attr(&self, name: &str) -> Result<Element, AxCode> {
        let v = self.attr(name)?;
        v.downcast::<AXUIElement>()
            .map(Element)
            .map_err(|_| code(AXError::IllegalArgument))
    }

    /// Metadata string (role, subrole). Must not be used for field contents.
    pub fn meta_attr(&self, name: &str) -> Result<String, AxCode> {
        let v = self.attr(name)?;
        let s = v
            .downcast::<CFString>()
            .map_err(|_| code(AXError::IllegalArgument))?;
        Ok(s.to_string())
    }

    /// Field contents. Wrapped in [`ProbeText`] so it cannot be printed.
    pub fn text_attr(&self, name: &str) -> Result<ProbeText, AxCode> {
        let v = self.attr(name)?;
        let s = v
            .downcast::<CFString>()
            .map_err(|_| code(AXError::IllegalArgument))?;
        Ok(ProbeText::new(s.to_string()))
    }

    pub fn bool_attr(&self, name: &str) -> Result<bool, AxCode> {
        let v = self.attr(name)?;
        let b = v
            .downcast::<CFBoolean>()
            .map_err(|_| code(AXError::IllegalArgument))?;
        Ok(b.as_bool())
    }

    pub fn number_attr(&self, name: &str) -> Result<i64, AxCode> {
        let v = self.attr(name)?;
        let n = v
            .downcast::<CFNumber>()
            .map_err(|_| code(AXError::IllegalArgument))?;
        n.as_i64().ok_or(code(AXError::IllegalArgument))
    }

    pub fn range_attr(&self, name: &str) -> Result<(isize, isize), AxCode> {
        let v = self.attr(name)?;
        let v = v
            .downcast::<AXValue>()
            .map_err(|_| code(AXError::IllegalArgument))?;
        let mut r = CFRange {
            location: 0,
            length: 0,
        };
        // SAFETY: `r` is a CFRange-sized out buffer matching the requested type.
        let ok = unsafe { v.value(AXValueType::CFRange, NonNull::from(&mut r).cast::<c_void>()) };
        if ok {
            Ok((r.location, r.length))
        } else {
            Err(code(AXError::IllegalArgument))
        }
    }

    pub fn settable(&self, name: &str) -> Result<bool, AxCode> {
        let name = CFString::from_str(name);
        let mut out: u8 = 0;
        // SAFETY: `out` is a valid Boolean out-pointer.
        let e = unsafe { self.0.is_attribute_settable(&name, NonNull::from(&mut out)) };
        if e == AXError::Success {
            Ok(out != 0)
        } else {
            Err(code(e))
        }
    }

    fn set_value(&self, name: &str, value: &CFType) -> AxCode {
        let name = CFString::from_str(name);
        // SAFETY: both references are valid CF objects for the duration of the call.
        code(unsafe { self.0.set_attribute_value(&name, value) })
    }

    pub fn set_string(&self, name: &str, value: &str) -> AxCode {
        let v = CFString::from_str(value);
        self.set_value(name, &v)
    }

    pub fn set_bool(&self, name: &str, value: bool) -> AxCode {
        self.set_value(name, CFBoolean::new(value))
    }

    pub fn set_range(&self, name: &str, location: isize, length: isize) -> AxCode {
        let r = CFRange { location, length };
        // SAFETY: `r` is a valid CFRange; AXValueCreate copies it.
        let v = unsafe { AXValue::new(AXValueType::CFRange, NonNull::from(&r).cast::<c_void>()) };
        match v {
            Some(v) => self.set_value(name, &v),
            None => code(AXError::Failure),
        }
    }
}

/// Role/subrole classification used by the probe's refusal policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldClass {
    /// Secure text (password) field: never read, never write.
    Secure,
    /// Ordinary text field / text area.
    Text,
    /// Anything else: refuse.
    Other,
}

pub fn classify(role: &str, subrole: Option<&str>) -> FieldClass {
    if subrole == Some(SECURE_SUBROLE) {
        return FieldClass::Secure;
    }
    match role {
        "AXTextField" | "AXTextArea" | "AXComboBox" | "AXSearchField" => FieldClass::Text,
        _ => FieldClass::Other,
    }
}
