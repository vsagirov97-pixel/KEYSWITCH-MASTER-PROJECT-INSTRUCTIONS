//! `UserText`: the only carrier of user text in the pipeline (CLAUDE.md rule 1, T-09).

/// Text read from or written to the user's application.
///
/// Deliberately implements neither `Debug` nor `Display` nor `Clone`, so it cannot end up in
/// logs, panic messages or test snapshots by accident. The buffer is overwritten with zeroes
/// on drop (best effort: copies made by the OS or by a reallocation are out of our reach).
pub struct UserText(String);

impl UserText {
    /// Takes ownership of `text`.
    pub fn new(text: String) -> Self {
        Self(text)
    }

    /// The text itself. Callers must not log, store or format it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Length in UTF-16 code units (the unit of Win32 and UI Automation).
    pub fn utf16_len(&self) -> usize {
        self.0.encode_utf16().count()
    }

    /// `true` if there is no text.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Equality of the contents (no text leaves the function).
    pub fn same_as(&self, other: &UserText) -> bool {
        self.0 == other.0
    }
}

impl Drop for UserText {
    fn drop(&mut self) {
        wipe(std::mem::take(&mut self.0));
    }
}

/// Overwrites a string's buffer with zeroes before freeing it.
pub fn wipe(text: String) {
    let mut bytes = text.into_bytes();
    bytes.fill(0);
    // Keeps the optimiser from removing the "dead" store above.
    std::hint::black_box(&bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_are_utf16_units() {
        assert_eq!(UserText::new("привет".into()).utf16_len(), 6);
        assert_eq!(UserText::new("a\u{1F600}".into()).utf16_len(), 3);
        assert!(UserText::new(String::new()).is_empty());
    }

    #[test]
    fn equality_helper() {
        let a = UserText::new("abc".into());
        assert!(a.same_as(&UserText::new("abc".into())));
        assert!(!a.same_as(&UserText::new("abd".into())));
    }
}
