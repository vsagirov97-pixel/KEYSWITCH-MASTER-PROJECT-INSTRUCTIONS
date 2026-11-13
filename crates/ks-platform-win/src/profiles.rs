//! Per-application profiles that enable the clipboard methods (W2 paste, Ctrl+C read).
//!
//! The clipboard is never used for an application without a profile. The table is data: a
//! new application is a new row, not new code. Every default row is **UNVERIFIED on the real
//! application** (`verified_on_real_app = false`): the mechanism is proven against the test
//! fixture only, the rows await owner manual acceptance.

/// How the text to convert is read from a profiled application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadMode {
    /// UI Automation TextPattern only.
    Uia,
    /// TextPattern; if the focused element has none, the **selection** via Ctrl+C.
    UiaThenCopy,
    /// Always the selection via Ctrl+C (TextPattern exists but is not trusted).
    Copy,
}

/// One application profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Lower-cased executable name; one `*` matches any run of characters.
    pub exe: &'static str,
    /// Write by clipboard paste (W2) instead of W3.
    pub paste: bool,
    /// How to read.
    pub read: ReadMode,
    /// Focus window classes in which the profile applies (empty = any). Outside them the
    /// application is refused (Excel outside cell editing).
    pub focus_classes: &'static [&'static str],
    /// The paste signal (`WM_RENDERFORMAT` from the target's pid) was confirmed for this
    /// application, so the clipboard may be restored on the signal alone when the result
    /// cannot be read back (CISO Q-1, "Unknown" post-check).
    pub signal_confirmed: bool,
    /// The row was accepted on the real application.
    pub verified_on_real_app: bool,
}

const fn unverified(
    exe: &'static str,
    read: ReadMode,
    focus_classes: &'static [&'static str],
) -> Profile {
    Profile {
        exe,
        paste: true,
        read,
        focus_classes,
        signal_confirmed: false,
        verified_on_real_app: false,
    }
}

/// Conservative defaults. All rows UNVERIFIED on the real applications.
pub const DEFAULTS: &[Profile] = &[
    unverified("chrome.exe", ReadMode::UiaThenCopy, &[]),
    unverified("msedge.exe", ReadMode::UiaThenCopy, &[]),
    unverified("telegram.exe", ReadMode::UiaThenCopy, &[]),
    unverified("whatsapp*.exe", ReadMode::UiaThenCopy, &[]),
    unverified("winword.exe", ReadMode::UiaThenCopy, &[]),
    // Excel only while a cell is being edited: in-cell editor and formula bar. On the grid
    // Ctrl+C would copy the whole cell (SYSTEM_ARCHITECTURE §4.2: refuse).
    unverified("excel.exe", ReadMode::UiaThenCopy, &["EXCEL6", "EXCEL<"]),
];

fn matches(pattern: &str, exe: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == exe,
        Some((head, tail)) => {
            exe.len() >= head.len() + tail.len() && exe.starts_with(head) && exe.ends_with(tail)
        }
    }
}

/// The profile for lower-cased executable name `exe`, if any.
pub fn find<'a>(table: &'a [Profile], exe: &str) -> Option<&'a Profile> {
    table.iter().find(|p| matches(p.exe, exe))
}

impl Profile {
    /// `true` if the profile applies to a focus window of class `class`.
    pub fn allows_class(&self, class: &str) -> bool {
        self.focus_classes.is_empty()
            || self
                .focus_classes
                .iter()
                .any(|c| c.eq_ignore_ascii_case(class))
    }

    /// `true` if the selection must be read via Ctrl+C given TextPattern availability.
    pub fn reads_by_copy(&self, has_text_pattern: bool) -> bool {
        match self.read {
            ReadMode::Uia => false,
            ReadMode::UiaThenCopy => !has_text_pattern,
            ReadMode::Copy => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_the_agreed_apps_and_are_all_unverified() {
        for exe in [
            "chrome.exe",
            "msedge.exe",
            "telegram.exe",
            "whatsapp.exe",
            "whatsapp.root.exe",
            "winword.exe",
            "excel.exe",
        ] {
            assert!(find(DEFAULTS, exe).is_some(), "{exe}");
        }
        assert!(DEFAULTS.iter().all(|p| !p.verified_on_real_app));
        assert!(DEFAULTS.iter().all(|p| !p.signal_confirmed));
        assert!(DEFAULTS.iter().all(|p| p.exe == p.exe.to_lowercase()));
    }

    #[test]
    fn unlisted_apps_have_no_profile() {
        for exe in [
            "notepad.exe",
            "",
            "chrome.exe.evil.exe",
            "xchrome.exe",
            "whatsapp",
        ] {
            assert!(find(DEFAULTS, exe).is_none(), "{exe}");
        }
    }

    #[test]
    fn excel_applies_only_while_editing_a_cell() {
        let p = find(DEFAULTS, "excel.exe").expect("row");
        assert!(p.allows_class("EXCEL6"));
        assert!(p.allows_class("EXCEL<"));
        assert!(!p.allows_class("EXCEL7"));
        assert!(find(DEFAULTS, "chrome.exe").expect("row").allows_class("x"));
    }

    #[test]
    fn copy_read_only_without_text_pattern_by_default() {
        let p = find(DEFAULTS, "telegram.exe").expect("row");
        assert!(!p.reads_by_copy(true));
        assert!(p.reads_by_copy(false));
    }
}
