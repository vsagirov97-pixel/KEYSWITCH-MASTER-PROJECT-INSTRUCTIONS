//! Holder for text read from a field. Deliberately has no `Debug`/`Display`/`Clone`
//! (mirrors the `UserText` rule): the only things that leave it are lengths and
//! equality results.

/// Text read from an accessibility element. Never printed, never logged.
pub struct ProbeText(String);

impl ProbeText {
    pub(crate) fn new(s: String) -> Self {
        Self(s)
    }

    /// Length in UTF-16 code units (the unit AX ranges use).
    pub fn utf16_len(&self) -> usize {
        self.0.encode_utf16().count()
    }

    /// Exact equality with an expected (synthetic) string.
    pub fn matches(&self, expected: &str) -> bool {
        self.0 == expected
    }

    /// True if the text is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
