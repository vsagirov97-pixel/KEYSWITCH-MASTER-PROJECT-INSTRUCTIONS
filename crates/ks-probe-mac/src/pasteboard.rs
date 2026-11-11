//! NSPasteboard snapshot / write / restore-only-if-unchanged (method M2 building blocks).
//!
//! Apple docs: <https://developer.apple.com/documentation/appkit/nspasteboard>
//! Privacy markers: <http://nspasteboard.org> (`org.nspasteboard.TransientType`,
//! `org.nspasteboard.ConcealedType`) [reported: community convention, not Apple API].

use objc2::rc::Retained;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSData, NSString};

pub const T_UTF8: &str = "public.utf8-plain-text";
pub const T_HTML: &str = "public.html";
pub const T_RTF: &str = "public.rtf";
pub const T_TRANSIENT: &str = "org.nspasteboard.TransientType";
pub const T_CONCEALED: &str = "org.nspasteboard.ConcealedType";

/// Types we are willing to snapshot and restore. Anything else => refuse M2.
const WHITELIST: [&str; 3] = [T_UTF8, T_HTML, T_RTF];

/// Legacy (pre-UTI) aliases that `-[NSPasteboard types]` may report on its own for the
/// whitelisted UTIs [hypothesis: CI run 42830218515 refused a utf8+html-only seed with
/// UnsupportedType; the legacy names exist as deprecated constants in `NSPasteboard.h`;
/// the probe now prints the offending type so the next run confirms or refutes this].
/// If the system derives them from the UTI, they reappear on restore, so they are
/// neither snapshotted nor treated as foreign.
const DERIVED_ALIASES: [&str; 3] = [
    "NSStringPboardType",
    "Apple HTML pasteboard type",
    "NeXT Rich Text Format v1.0 pasteboard type",
];

fn is_derived(t: &str) -> bool {
    DERIVED_ALIASES.contains(&t)
}

pub struct Board(Retained<NSPasteboard>);

/// Why a snapshot was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// More than one pasteboard item (spike limitation; M-2b).
    MultipleItems(usize),
    /// A type outside the whitelist is present (files, images, private types).
    /// Carries the type identifier (metadata, never content) for diagnostics.
    UnsupportedType(String),
}

/// Copy of whitelisted pasteboard data. Bytes are never printed.
pub struct Snapshot {
    entries: Vec<(String, Vec<u8>)>,
}

impl Snapshot {
    pub fn type_count(&self) -> usize {
        self.entries.len()
    }
    pub fn byte_len(&self) -> usize {
        self.entries.iter().map(|(_, d)| d.len()).sum()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RestoreOutcome {
    Restored,
    /// `changeCount` moved since our write: someone else owns the pasteboard now.
    ChangedByOther,
}

impl Board {
    pub fn general() -> Self {
        Self(NSPasteboard::generalPasteboard())
    }

    pub fn change_count(&self) -> isize {
        self.0.changeCount()
    }

    pub fn clear(&self) -> isize {
        self.0.clearContents()
    }

    pub fn set_string(&self, ty: &str, s: &str) -> bool {
        self.0
            .setString_forType(&NSString::from_str(s), &NSString::from_str(ty))
    }

    pub fn set_data(&self, ty: &str, bytes: &[u8]) -> bool {
        let d = NSData::with_bytes(bytes);
        self.0.setData_forType(Some(&d), &NSString::from_str(ty))
    }

    pub fn data(&self, ty: &str) -> Option<Vec<u8>> {
        self.0
            .dataForType(&NSString::from_str(ty))
            .map(|d| d.to_vec())
    }

    pub fn types(&self) -> Vec<String> {
        self.0
            .types()
            .map(|a| a.iter().map(|t| t.to_string()).collect())
            .unwrap_or_default()
    }

    pub fn item_count(&self) -> usize {
        self.0.pasteboardItems().map(|a| a.len()).unwrap_or(0)
    }

    /// Snapshot of whitelisted types; refuses on anything it cannot restore faithfully.
    pub fn snapshot(&self) -> Result<Snapshot, SnapshotError> {
        let items = self.item_count();
        if items > 1 {
            return Err(SnapshotError::MultipleItems(items));
        }
        let mut entries = Vec::new();
        for t in self.types() {
            if is_derived(&t) {
                continue;
            }
            if !WHITELIST.contains(&t.as_str()) {
                return Err(SnapshotError::UnsupportedType(t));
            }
            if let Some(d) = self.data(&t) {
                entries.push((t, d));
            }
        }
        Ok(Snapshot { entries })
    }

    /// Writes our text with privacy markers; returns the resulting `changeCount`.
    pub fn write_ours(&self, text: &str) -> isize {
        self.clear();
        self.set_string(T_UTF8, text);
        self.set_data(T_TRANSIENT, &[]);
        self.set_data(T_CONCEALED, &[]);
        self.change_count()
    }

    /// Restores `snap` only if nobody touched the pasteboard since `our_count`.
    pub fn restore_if_unchanged(&self, our_count: isize, snap: &Snapshot) -> RestoreOutcome {
        if self.change_count() != our_count {
            return RestoreOutcome::ChangedByOther;
        }
        self.clear();
        for (t, d) in &snap.entries {
            self.set_data(t, d);
        }
        RestoreOutcome::Restored
    }

    /// True if current contents equal the snapshot byte-for-byte (same type set,
    /// ignoring system-derived legacy aliases).
    pub fn equals(&self, snap: &Snapshot) -> bool {
        let mut now: Vec<String> = self
            .types()
            .into_iter()
            .filter(|t| !is_derived(t))
            .collect();
        now.sort();
        let mut then: Vec<&str> = snap.entries.iter().map(|(t, _)| t.as_str()).collect();
        then.sort_unstable();
        now == then
            && snap
                .entries
                .iter()
                .all(|(t, d)| self.data(t).as_deref() == Some(d))
    }
}
