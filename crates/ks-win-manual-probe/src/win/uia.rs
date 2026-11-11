//! UI Automation client (MTA, CUIAutomation8, 500/1000 ms timeouts).
//!
//! WS-03: text is reached through exactly one function, [`guarded_text`], which refuses when
//! the element is a password field by *either* `IsPassword` (errors count as password) *or*
//! the `ES_PASSWORD` style of an Edit/RichEdit native window. Survey mode only ever asks it
//! for a **length**; the text buffer is dropped immediately and never leaves this module
//! except in fixture mode, where the content is the tool's own synthetic string.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextPattern2, IUIAutomationTextRange, IUIAutomationValuePattern,
    TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TextUnit_Character,
    UIA_TextEditPatternId, UIA_TextPattern2Id, UIA_TextPatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::ES_PASSWORD;
use windows::core::{BOOL, BSTR, Interface};

use super::sys::{self, Hwnd};
use crate::policy;

/// Number of `GetText` calls (process-wide) — lets fixture mode prove the password guard.
pub static GET_TEXT_CALLS: AtomicU32 = AtomicU32::new(0);

/// Upper bound for any `GetText` (survey reports `>=` this value when reached).
pub const TEXT_CAP: i32 = 200_000;

/// COM apartment guard.
#[derive(Debug)]
pub struct ComGuard;

impl ComGuard {
    /// Joins the MTA.
    pub fn mta() -> Result<Self, String> {
        // SAFETY: standard COM initialisation, balanced in Drop.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self)
        } else if hr == RPC_E_CHANGED_MODE {
            Err("thread already STA".into())
        } else {
            Err(format!("CoInitializeEx: {:#x}", hr.0))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: balances the successful CoInitializeEx on this thread.
        unsafe { CoUninitialize() };
    }
}

/// UIA client; drop before the [`ComGuard`].
#[derive(Debug)]
pub struct Uia {
    automation: IUIAutomation,
}

impl Uia {
    /// Creates CUIAutomation8 with explicit timeouts.
    pub fn new() -> Result<Self, String> {
        // SAFETY: COM is initialised on this thread; documented CLSID.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
                .map_err(|e| format!("CoCreateInstance(CUIAutomation8): {:#x}", e.code().0))?;
        if let Ok(a2) = automation.cast::<IUIAutomation2>() {
            // SAFETY: setters on a live interface.
            unsafe {
                let _ = a2.SetConnectionTimeout(500);
                let _ = a2.SetTransactionTimeout(1000);
            }
        }
        Ok(Self { automation })
    }

    /// System-wide focused element.
    pub fn focused(&self) -> Result<IUIAutomationElement, String> {
        // SAFETY: method call on a live interface.
        unsafe { self.automation.GetFocusedElement() }
            .map_err(|e| format!("GetFocusedElement:{:#x}", e.code().0))
    }
}

fn hr(e: &windows::core::Error) -> String {
    format!("{:#x}", e.code().0)
}

/// Metadata of an element (no text, no Name, no Value).
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// ControlType id.
    pub control_type: i32,
    /// LocalizedControlType (a system label such as "edit").
    pub localized_type: String,
    /// ClassName.
    pub class_name: String,
    /// FrameworkId (Win32, WPF, Chrome, XAML, DirectUI, ...).
    pub framework: String,
    /// IsPassword; `None` = the query failed (treated as password).
    pub is_password: Option<bool>,
    /// `ES_PASSWORD` on an Edit/RichEdit native window.
    pub es_password: bool,
    /// ValuePattern.IsReadOnly; `None` = no ValuePattern / failed.
    pub value_read_only: Option<bool>,
    /// IsEnabled.
    pub enabled: Option<bool>,
    /// HasKeyboardFocus.
    pub has_focus: Option<bool>,
    /// Native window handle reported by UIA.
    pub hwnd: Option<Hwnd>,
    /// Owning process id.
    pub pid: u32,
    /// Supports TextPattern / TextPattern2 / ValuePattern / TextEditPattern.
    pub text: bool,
    /// See `text`.
    pub text2: bool,
    /// See `text`.
    pub value: bool,
    /// See `text`.
    pub text_edit: bool,
}

impl Facts {
    /// Fail-closed password decision (WS-03).
    pub fn treat_as_password(&self) -> bool {
        self.is_password != Some(false) || self.es_password
    }
}

fn has_pattern(
    el: &IUIAutomationElement,
    id: windows::Win32::UI::Accessibility::UIA_PATTERN_ID,
) -> bool {
    // SAFETY: method call on a live interface; the result is an owned IUnknown or error.
    unsafe { el.GetCurrentPattern(id) }.is_ok_and(|p| !p.as_raw().is_null())
}

/// Reads [`Facts`] (property getters only).
pub fn facts(el: &IUIAutomationElement) -> Facts {
    // SAFETY: property getters on a live interface; results are owned values.
    let mut f = unsafe {
        Facts {
            control_type: el.CurrentControlType().map(|c| c.0).unwrap_or(0),
            localized_type: el
                .CurrentLocalizedControlType()
                .map(|b| b.to_string())
                .unwrap_or_default(),
            class_name: el
                .CurrentClassName()
                .map(|b| b.to_string())
                .unwrap_or_default(),
            framework: el
                .CurrentFrameworkId()
                .map(|b| b.to_string())
                .unwrap_or_default(),
            is_password: el.CurrentIsPassword().ok().map(|b| b.as_bool()),
            enabled: el.CurrentIsEnabled().ok().map(|b| b.as_bool()),
            has_focus: el.CurrentHasKeyboardFocus().ok().map(|b| b.as_bool()),
            hwnd: el
                .CurrentNativeWindowHandle()
                .ok()
                .map(Hwnd::from_raw)
                .filter(|h| !h.is_null()),
            pid: el.CurrentProcessId().unwrap_or(0).max(0) as u32,
            ..Facts::default()
        }
    };
    f.text = has_pattern(el, UIA_TextPatternId);
    f.text2 = has_pattern(el, UIA_TextPattern2Id);
    f.value = has_pattern(el, UIA_ValuePatternId);
    f.text_edit = has_pattern(el, UIA_TextEditPatternId);
    if f.value {
        // SAFETY: method calls on live interfaces; only the IsReadOnly flag is read.
        f.value_read_only = unsafe {
            el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                .ok()
                .and_then(|v| v.CurrentIsReadOnly().ok())
                .map(|b| b.as_bool())
        };
    }
    if let Some(h) = f.hwnd {
        let class = sys::class_name(h);
        f.es_password =
            policy::is_edit_family_class(&class) && sys::style(h) & (ES_PASSWORD as u32) != 0;
    }
    f
}

/// TextPattern of an element.
pub fn text_pattern(el: &IUIAutomationElement) -> Option<IUIAutomationTextPattern> {
    // SAFETY: method call on a live interface.
    unsafe { el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }.ok()
}

/// Why [`guarded_text`] returned nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Password (by IsPassword, its failure, or ES_PASSWORD); GetText not called.
    Password,
    /// GetText failed with this HRESULT.
    Failed(String),
}

/// The only `GetText` call site (WS-03). `facts` must be the element's current facts.
fn guarded_text(facts: &Facts, r: &IUIAutomationTextRange, max: i32) -> Result<BSTR, Refused> {
    if facts.treat_as_password() {
        return Err(Refused::Password);
    }
    GET_TEXT_CALLS.fetch_add(1, Ordering::SeqCst);
    // SAFETY: method call on a live interface; the BSTR is owned and freed on drop.
    unsafe { r.GetText(max) }.map_err(|e| Refused::Failed(hr(&e)))
}

/// Length (UTF-16 units) of a range's text; the text is dropped immediately.
pub fn guarded_len(facts: &Facts, r: &IUIAutomationTextRange) -> Result<usize, Refused> {
    guarded_text(facts, r, TEXT_CAP).map(|b| b.len())
}

/// Fixture only: `true` if the range text equals our synthetic `expected` string.
pub fn guarded_equals(
    facts: &Facts,
    r: &IUIAutomationTextRange,
    expected: &str,
) -> Result<bool, Refused> {
    let b = guarded_text(facts, r, TEXT_CAP)?;
    Ok(b.iter().copied().eq(expected.encode_utf16()))
}

/// Text-related probes for the survey (lengths and success flags only).
#[derive(Clone, Debug, Default)]
pub struct TextProbe {
    /// DocumentRange: "ok" or HRESULT.
    pub doc_range: String,
    /// Document length or refusal/error label.
    pub doc_len: String,
    /// GetSelection: "ok" or HRESULT.
    pub selection: String,
    /// Number of selection ranges.
    pub sel_count: String,
    /// Length of the first selection range or refusal/error label.
    pub sel_len: String,
    /// SupportedTextSelection value (0 none, 1 single, 2 multiple).
    pub supported_selection: String,
    /// TextPattern2.GetCaretRange: "ok"/"active"/HRESULT/"n/a".
    pub caret_range: String,
    /// Milliseconds for the text probes.
    pub ms: u128,
}

fn len_label(r: Result<usize, Refused>) -> String {
    match r {
        Ok(n) if n as i64 >= i64::from(TEXT_CAP) => format!(">={TEXT_CAP}"),
        Ok(n) => n.to_string(),
        Err(Refused::Password) => "refused_password".into(),
        Err(Refused::Failed(h)) => format!("error:{h}"),
    }
}

/// Runs the read-only text probes on the focused element.
pub fn text_probe(el: &IUIAutomationElement, f: &Facts) -> TextProbe {
    let t0 = Instant::now();
    let mut p = TextProbe {
        doc_range: "n/a".into(),
        doc_len: "n/a".into(),
        selection: "n/a".into(),
        sel_count: "n/a".into(),
        sel_len: "n/a".into(),
        supported_selection: "n/a".into(),
        caret_range: "n/a".into(),
        ms: 0,
    };
    let Some(tp) = text_pattern(el) else {
        return p;
    };
    if f.treat_as_password() {
        // Do not even open ranges on password fields.
        p.doc_len = "refused_password".into();
        p.sel_len = "refused_password".into();
        p.ms = t0.elapsed().as_millis();
        return p;
    }
    // SAFETY: method calls on live interfaces; returned objects are owned.
    unsafe {
        p.supported_selection = tp
            .SupportedTextSelection()
            .map_or_else(|e| format!("error:{}", hr(&e)), |s| s.0.to_string());
        match tp.DocumentRange() {
            Ok(doc) => {
                p.doc_range = "ok".into();
                p.doc_len = len_label(guarded_len(f, &doc));
            }
            Err(e) => p.doc_range = format!("error:{}", hr(&e)),
        }
        match tp.GetSelection() {
            Ok(arr) => {
                p.selection = "ok".into();
                let n = arr.Length().unwrap_or(0);
                p.sel_count = n.to_string();
                p.sel_len = if n > 0 {
                    match arr.GetElement(0) {
                        Ok(r) => len_label(guarded_len(f, &r)),
                        Err(e) => format!("error:{}", hr(&e)),
                    }
                } else {
                    "0".into()
                };
            }
            Err(e) => p.selection = format!("error:{}", hr(&e)),
        }
        if f.text2 {
            p.caret_range =
                match el.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) {
                    Ok(t2) => {
                        let mut active = BOOL(0);
                        match t2.GetCaretRange(&mut active) {
                            Ok(_) => {
                                if active.as_bool() {
                                    "ok_active".into()
                                } else {
                                    "ok_inactive".into()
                                }
                            }
                            Err(e) => format!("error:{}", hr(&e)),
                        }
                    }
                    Err(e) => format!("error:{}", hr(&e)),
                };
        }
    }
    p.ms = t0.elapsed().as_millis();
    p
}

/// Range over the first `chars` characters of the document.
pub fn prefix_range(
    tp: &IUIAutomationTextPattern,
    chars: i32,
) -> Result<IUIAutomationTextRange, String> {
    // SAFETY: method calls on live interfaces; `r` is our own clone.
    unsafe {
        let doc = tp.DocumentRange().map_err(|e| hr(&e))?;
        let r = doc.Clone().map_err(|e| hr(&e))?;
        r.MoveEndpointByRange(
            TextPatternRangeEndpoint_End,
            &r,
            TextPatternRangeEndpoint_Start,
        )
        .map_err(|e| hr(&e))?;
        r.MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Character, chars)
            .map_err(|e| hr(&e))?;
        Ok(r)
    }
}

/// `Select()` on a range (fixture only).
pub fn select(r: &IUIAutomationTextRange) -> Result<(), String> {
    // SAFETY: method call on a live interface.
    unsafe { r.Select() }.map_err(|e| hr(&e))
}

/// Selection ranges (fixture only).
pub fn selection(tp: &IUIAutomationTextPattern) -> Result<Vec<IUIAutomationTextRange>, String> {
    // SAFETY: method calls on live interfaces.
    unsafe {
        let arr = tp.GetSelection().map_err(|e| hr(&e))?;
        let n = arr.Length().map_err(|e| hr(&e))?;
        (0..n)
            .map(|i| arr.GetElement(i).map_err(|e| hr(&e)))
            .collect()
    }
}
