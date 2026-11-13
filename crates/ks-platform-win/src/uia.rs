//! UI Automation client wrappers (MTA, CUIAutomation8, explicit timeouts).
//!
//! WS-03: `GetText` is reachable only through [`Uia::read_text`] and [`Uia::read_selection`],
//! both of which pass [`password_guard`] first: UIA `IsPassword` (errors = password, fail
//! closed) **and** `ES_PASSWORD` in `GWL_STYLE` of the element's native window (edit
//! family). Every call is counted in [`GET_TEXT_CALLS`] so tests can prove that a password
//! field was never read.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Ole::{
    SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextRange, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_TextPatternId,
};
use windows::core::{BSTR, Interface};

use crate::Hwnd;

/// Number of `GetText` calls made through this module (process-wide).
pub static GET_TEXT_CALLS: AtomicU32 = AtomicU32::new(0);

/// COM apartment guard for the current thread.
#[derive(Debug)]
pub struct ComGuard {
    uninit: bool,
}

impl ComGuard {
    /// Joins the MTA (as KeySwitch's T2 thread will).
    pub fn mta() -> Result<Self, String> {
        // SAFETY: standard COM initialisation for the calling thread; balanced in Drop.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self { uninit: true })
        } else if hr == RPC_E_CHANGED_MODE {
            Err("thread already initialised as STA".into())
        } else {
            Err(format!("CoInitializeEx: {hr:?}"))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninit {
            // SAFETY: balances the successful CoInitializeEx of this guard on this thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// UIA client. Must be dropped before the thread's [`ComGuard`].
#[derive(Debug)]
pub struct Uia {
    /// The automation object.
    pub automation: IUIAutomation,
}

/// Result of a guarded read.
#[derive(Debug)]
pub enum Read {
    /// Text of the document range (synthetic content in tests).
    Text(String),
    /// The element is a password field; `GetText` was not called.
    RefusedPassword,
    /// The element has no TextPattern.
    NoTextPattern,
}

impl Uia {
    /// Creates `CUIAutomation8` with 500 ms connection / 1000 ms transaction timeouts.
    pub fn new() -> Result<Self, String> {
        // SAFETY: COM is initialised on this thread (caller holds a ComGuard); the CLSID is
        // the documented UIA class.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
                .map_err(|e| format!("CoCreateInstance(CUIAutomation8): {e}"))?;
        if let Ok(a2) = automation.cast::<IUIAutomation2>() {
            // SAFETY: plain setters on a live interface.
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
            .map_err(|e| format!("GetFocusedElement: {e}"))
    }

    /// Element for a window handle.
    pub fn from_hwnd(&self, h: Hwnd) -> Result<IUIAutomationElement, String> {
        // SAFETY: method call on a live interface; an invalid handle only yields an error.
        unsafe { self.automation.ElementFromHandle(h.raw()) }
            .map_err(|e| format!("ElementFromHandle: {e}"))
    }

    /// Guarded read of the whole document range, at most `max` UTF-16 units.
    pub fn read_text(&self, el: &IUIAutomationElement, max: i32) -> Result<Read, String> {
        if password_guard(el).is_some() {
            return Ok(Read::RefusedPassword);
        }
        let Some(tp) = text_pattern(el) else {
            return Ok(Read::NoTextPattern);
        };
        // SAFETY: method calls on live interfaces.
        let doc = unsafe { tp.DocumentRange() }.map_err(|e| format!("DocumentRange: {e}"))?;
        Ok(Read::Text(range_text(&doc, max)?))
    }

    /// Guarded read of the (single) current selection of `el`, at most `max` units.
    /// `Ok(None)` if there is not exactly one selection range.
    pub fn read_selection(
        &self,
        el: &IUIAutomationElement,
        max: i32,
    ) -> Result<Option<Read>, String> {
        if password_guard(el).is_some() {
            return Ok(Some(Read::RefusedPassword));
        }
        let Some(tp) = text_pattern(el) else {
            return Ok(Some(Read::NoTextPattern));
        };
        let sel = selection(&tp)?;
        if sel.len() != 1 {
            return Ok(None);
        }
        Ok(Some(Read::Text(range_text(&sel[0], max)?)))
    }

    /// Guarded read of at most `units` characters immediately before the caret (the start of
    /// the single selection). `Ok(None)` if there is not exactly one selection range.
    /// Never reads the whole document.
    pub fn read_before_caret(
        &self,
        el: &IUIAutomationElement,
        units: i32,
    ) -> Result<Option<Read>, String> {
        if password_guard(el).is_some() {
            return Ok(Some(Read::RefusedPassword));
        }
        let Some(tp) = text_pattern(el) else {
            return Ok(Some(Read::NoTextPattern));
        };
        let sel = selection(&tp)?;
        if sel.len() != 1 {
            return Ok(None);
        }
        let r = before_caret_range(&sel[0], units)?;
        Ok(Some(Read::Text(range_text(&r, units)?)))
    }

    /// Selects the last `units` characters before the caret. `Ok(false)` if there is not
    /// exactly one selection range or no TextPattern.
    pub fn select_before_caret(
        &self,
        el: &IUIAutomationElement,
        units: i32,
    ) -> Result<bool, String> {
        let Some(tp) = text_pattern(el) else {
            return Ok(false);
        };
        let sel = selection(&tp)?;
        if sel.len() != 1 {
            return Ok(false);
        }
        select(&before_caret_range(&sel[0], units)?)?;
        Ok(true)
    }

    /// Collapses the single selection of `el` to its end, but only if that selection is
    /// still exactly `expected` (the fragment the operation selected). `Ok(true)` if the
    /// selection was collapsed. Used to undo a caret-mode selection after a refused write.
    pub fn collapse_selection_if(
        &self,
        el: &IUIAutomationElement,
        expected: &str,
    ) -> Result<bool, String> {
        if password_guard(el).is_some() {
            return Ok(false);
        }
        let Some(tp) = text_pattern(el) else {
            return Ok(false);
        };
        let sel = selection(&tp)?;
        if sel.len() != 1 {
            return Ok(false);
        }
        // QA-B1-09: compared in place, no `String` copy; both buffers are wiped.
        let mut want: Vec<u16> = expected.encode_utf16().collect();
        let max = i32::try_from(want.len().saturating_add(1)).unwrap_or(i32::MAX);
        let same = range_equals(&sel[0], max, &want);
        want.fill(0);
        std::hint::black_box(&want);
        if !same? {
            return Ok(false);
        }
        // SAFETY: method calls on live interfaces; `r` is a fresh clone we own.
        let r = unsafe {
            let r = sel[0].Clone().map_err(|e| format!("Clone: {e}"))?;
            r.MoveEndpointByRange(
                TextPatternRangeEndpoint_Start,
                &r,
                TextPatternRangeEndpoint_End,
            )
            .map_err(|e| format!("MoveEndpointByRange: {e}"))?;
            r
        };
        select(&r)?;
        Ok(true)
    }
}

/// Lengths around the caret of a text element, for diagnostics only (no text kept).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaretFacts {
    /// Number of selection ranges.
    pub selections: usize,
    /// UTF-16 units of the (first) selection; 0 = caret only.
    pub selection_units: usize,
    /// UTF-16 units from the document start to the selection start (as UIA reports it).
    pub units_before_caret: usize,
    /// UTF-16 units of the whole document (capped at `max`).
    pub document_units: usize,
    /// `\r\n` pairs and lone `\r` / `\n` in the document as UIA returns it.
    pub document_crlf: usize,
    pub document_lone_cr: usize,
    pub document_lone_lf: usize,
}

/// Line-break shape of `s`: (`\r\n` pairs, lone `\r`, lone `\n`).
pub fn line_breaks(s: &str) -> (usize, usize, usize) {
    let crlf = s.matches("\r\n").count();
    let cr = s.matches('\r').count() - crlf;
    let lf = s.matches('\n').count() - crlf;
    (crlf, cr, lf)
}

/// Diagnostics of the caret position of `el`, reading at most `max` units. Test use only:
/// it reads the document from its start (the text itself is dropped right away).
pub fn caret_facts(el: &IUIAutomationElement, max: i32) -> Result<CaretFacts, String> {
    if password_guard(el).is_some() {
        return Err("password".into());
    }
    let tp = text_pattern(el).ok_or("no TextPattern")?;
    let sel = selection(&tp)?;
    let mut facts = CaretFacts {
        selections: sel.len(),
        ..CaretFacts::default()
    };
    // SAFETY: method calls on live interfaces; ranges are fresh clones we own.
    let doc = unsafe { tp.DocumentRange() }.map_err(|e| format!("DocumentRange: {e}"))?;
    let all = ks_engine::UserText::new(range_text(&doc, max)?);
    facts.document_units = all.utf16_len();
    (
        facts.document_crlf,
        facts.document_lone_cr,
        facts.document_lone_lf,
    ) = line_breaks(all.expose());
    if let Some(first) = sel.first() {
        facts.selection_units = ks_engine::UserText::new(range_text(first, max)?).utf16_len();
        // SAFETY: as above.
        let head = unsafe {
            let r = doc.Clone().map_err(|e| format!("Clone: {e}"))?;
            r.MoveEndpointByRange(
                TextPatternRangeEndpoint_End,
                first,
                TextPatternRangeEndpoint_Start,
            )
            .map_err(|e| format!("MoveEndpointByRange: {e}"))?;
            r
        };
        facts.units_before_caret = ks_engine::UserText::new(range_text(&head, max)?).utf16_len();
    }
    Ok(facts)
}

/// Range of at most `units` characters that ends at the start of `sel`.
pub fn before_caret_range(
    sel: &IUIAutomationTextRange,
    units: i32,
) -> Result<IUIAutomationTextRange, String> {
    // SAFETY: method calls on live interfaces; `r` is a fresh clone we own.
    unsafe {
        let r = sel.Clone().map_err(|e| format!("Clone: {e}"))?;
        r.MoveEndpointByRange(
            TextPatternRangeEndpoint_End,
            &r,
            TextPatternRangeEndpoint_Start,
        )
        .map_err(|e| format!("MoveEndpointByRange: {e}"))?;
        r.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -units)
            .map_err(|e| format!("MoveEndpointByUnit: {e}"))?;
        Ok(r)
    }
}

/// Which check marked an element as a password (WS-03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordBy {
    /// UIA `CurrentIsPassword` was true or failed (fail closed).
    UiaIsPassword,
    /// `GWL_STYLE` of the native window has `ES_PASSWORD` (edit family).
    EsPasswordStyle,
}

/// WS-03 single gate before any text read: `Some(reason)` = do not read.
pub fn password_guard(el: &IUIAutomationElement) -> Option<PasswordBy> {
    // SAFETY: property getter on a live interface.
    let uia_pw = unsafe { el.CurrentIsPassword() }.map_or(true, |b| b.as_bool());
    if uia_pw {
        return Some(PasswordBy::UiaIsPassword);
    }
    // SAFETY: property getter on a live interface.
    if let Ok(h) = unsafe { el.CurrentNativeWindowHandle() }
        && !h.0.is_null()
        && crate::gate::hwnd_is_password(Hwnd::from_raw(h))
    {
        return Some(PasswordBy::EsPasswordStyle);
    }
    None
}

/// Metadata of an element (no text).
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// `UIA_CONTROLTYPE_ID` value (50004 = Edit, 50030 = Document).
    pub control_type: i32,
    /// `CurrentIsPassword`. Errors are treated as `true` (fail closed).
    pub is_password: bool,
    /// Native window handle reported by UIA.
    pub hwnd: Option<Hwnd>,
    /// `CurrentClassName` — a control class name, not user text.
    pub class_name: String,
    /// Owning process id.
    pub pid: i32,
    /// RuntimeId as returned by `GetRuntimeId`.
    pub runtime_id: Vec<i32>,
    /// Element supports TextPattern.
    pub has_text_pattern: bool,
}

/// Reads [`Facts`] for an element.
pub fn facts(el: &IUIAutomationElement) -> Facts {
    // SAFETY: property getters on a live interface; results are owned values.
    unsafe {
        Facts {
            control_type: el.CurrentControlType().map(|c| c.0).unwrap_or(0),
            is_password: el.CurrentIsPassword().map(|b| b.as_bool()).unwrap_or(true),
            hwnd: el.CurrentNativeWindowHandle().ok().map(Hwnd::from_raw),
            class_name: el
                .CurrentClassName()
                .map(|b| b.to_string())
                .unwrap_or_default(),
            pid: el.CurrentProcessId().unwrap_or(0),
            runtime_id: runtime_id(el).unwrap_or_default(),
            has_text_pattern: el
                .GetCurrentPattern(UIA_TextPatternId)
                .is_ok_and(|p| !p.as_raw().is_null()),
        }
    }
}

/// `GetRuntimeId` → vector of i32, the SAFEARRAY is destroyed afterwards.
pub fn runtime_id(el: &IUIAutomationElement) -> Result<Vec<i32>, String> {
    // SAFETY: GetRuntimeId returns a caller-owned 1-D SAFEARRAY of VT_I4 (or NULL). We read
    // it element by element into a local i32 and destroy it exactly once.
    unsafe {
        let sa = el
            .GetRuntimeId()
            .map_err(|e| format!("GetRuntimeId: {e}"))?;
        if sa.is_null() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let lb = SafeArrayGetLBound(sa, 1);
        let ub = SafeArrayGetUBound(sa, 1);
        if let (Ok(lb), Ok(ub)) = (lb, ub) {
            for i in lb..=ub {
                let mut v = 0i32;
                if SafeArrayGetElement(sa, &i, &mut v as *mut i32 as *mut c_void).is_ok() {
                    out.push(v);
                }
            }
        }
        let _ = SafeArrayDestroy(sa);
        Ok(out)
    }
}

/// TextPattern of an element, if supported.
pub fn text_pattern(el: &IUIAutomationElement) -> Option<IUIAutomationTextPattern> {
    // SAFETY: method call on a live interface; returns an owned interface or an error.
    unsafe { el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }.ok()
}

/// `GetText(max)` on a range (counted). Private: callers go through the guarded readers.
/// The returned BSTR is wiped before it is freed; the `String` is the caller's to protect.
fn range_text(r: &IUIAutomationTextRange, max: i32) -> Result<String, String> {
    let b = range_bstr(r, max)?;
    // Capacity up front (one UTF-16 unit is at most 3 UTF-8 bytes): no reallocation leaves
    // an unwiped copy behind.
    let mut s = String::with_capacity(b.len().saturating_mul(3));
    s.extend(char::decode_utf16(b.iter().copied()).map(|c| c.unwrap_or('\u{FFFD}')));
    wipe_bstr(b);
    Ok(s)
}

/// `true` if `GetText(max)` of `r` equals `want` (UTF-16). No copy of the text is made;
/// the BSTR is wiped before it is freed.
fn range_equals(r: &IUIAutomationTextRange, max: i32, want: &[u16]) -> Result<bool, String> {
    let b = range_bstr(r, max)?;
    let same = *b == *want;
    wipe_bstr(b);
    Ok(same)
}

fn range_bstr(r: &IUIAutomationTextRange, max: i32) -> Result<BSTR, String> {
    GET_TEXT_CALLS.fetch_add(1, Ordering::SeqCst);
    // SAFETY: method call on a live interface; the BSTR is owned by us.
    unsafe { r.GetText(max) }.map_err(|e| format!("GetText: {e}"))
}

/// Overwrites the characters of a BSTR that held user text, then frees it.
fn wipe_bstr(b: BSTR) {
    let units = b.len();
    let raw = b.into_raw().cast_mut();
    if !raw.is_null() && units > 0 {
        // SAFETY: `raw` is the allocation of the BSTR we owned (returned to us by GetText);
        // `units` (`SysStringLen`) characters lie inside it; nothing else references it.
        unsafe { std::ptr::write_bytes(raw, 0, units) };
    }
    // SAFETY: reassembles the BSTR taken apart above, so it is freed by `SysFreeString`.
    drop(unsafe { BSTR::from_raw(raw) });
}

/// Current selection ranges.
pub fn selection(tp: &IUIAutomationTextPattern) -> Result<Vec<IUIAutomationTextRange>, String> {
    // SAFETY: method calls on live interfaces; returned ranges are owned.
    unsafe {
        let arr = tp
            .GetSelection()
            .map_err(|e| format!("GetSelection: {e}"))?;
        let n = arr.Length().map_err(|e| format!("Length: {e}"))?;
        (0..n)
            .map(|i| arr.GetElement(i).map_err(|e| format!("GetElement: {e}")))
            .collect()
    }
}

/// Range covering the first `chars` characters of the document.
pub fn prefix_range(
    tp: &IUIAutomationTextPattern,
    chars: i32,
) -> Result<IUIAutomationTextRange, String> {
    // SAFETY: method calls on live interfaces; `r` is a fresh clone we own.
    unsafe {
        let doc = tp
            .DocumentRange()
            .map_err(|e| format!("DocumentRange: {e}"))?;
        let r = doc.Clone().map_err(|e| format!("Clone: {e}"))?;
        r.MoveEndpointByRange(
            TextPatternRangeEndpoint_End,
            &r,
            TextPatternRangeEndpoint_Start,
        )
        .map_err(|e| format!("MoveEndpointByRange: {e}"))?;
        r.MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Character, chars)
            .map_err(|e| format!("MoveEndpointByUnit: {e}"))?;
        Ok(r)
    }
}

/// `Select()` on a range.
pub fn select(r: &IUIAutomationTextRange) -> Result<(), String> {
    // SAFETY: method call on a live interface.
    unsafe { r.Select() }.map_err(|e| format!("Select: {e}"))
}

/// `Compare` of two ranges (same endpoints).
pub fn same_range(a: &IUIAutomationTextRange, b: &IUIAutomationTextRange) -> bool {
    // SAFETY: method call on live interfaces.
    unsafe { a.Compare(b) }.is_ok_and(|v| v.as_bool())
}
