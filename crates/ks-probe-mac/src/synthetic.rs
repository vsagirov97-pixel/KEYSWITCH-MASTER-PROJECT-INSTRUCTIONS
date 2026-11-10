//! Synthetic strings shared by the probe and the fixture. No real user data anywhere.

/// Initial value of the single-line `NSTextField` ("Привет мир" typed in the EN layout).
pub const FIELD_TEXT: &str = "Ghbdtn vbh";
/// Initial value of the `NSTextView` (multi-line, the last line is the "fragment").
pub const VIEW_TEXT: &str = "Synthetic line one\nGhbdtn, rfr ltkf?";
/// Value of the `NSSecureTextField`. The probe must never read it back.
pub const SECURE_TEXT: &str = "synthetic-not-a-password";
/// Replacement written through `kAXSelectedTextAttribute` (method M1).
pub const M1_REPLACEMENT: &str = "Привет";
/// Text typed through `CGEventKeyboardSetUnicodeString` (method M3): Cyrillic, Latin,
/// punctuation and one astral character (surrogate pair).
pub const M3_TEXT: &str = "Привет, мир! Hello, world. Ёжик 123 \u{1F600} тест; конец.";
/// How many times [`M3_TEXT`] is typed per run (gives a longer sample for chars/sec).
pub const M3_REPEAT: usize = 3;
/// Policy limit for whole-field reads (M-P02: 5 000 UTF-16 units).
pub const READ_LIMIT: usize = 5_000;
/// Length of the "too long" text view used to check the limit is enforced before reading.
pub const LONG_LEN: usize = 6_000;

/// Builds the over-limit synthetic text for the `--long` fixture mode.
pub fn long_text() -> String {
    "abcdefghij".repeat(LONG_LEN / 10)
}

/// Builds the full M3 payload.
pub fn m3_payload() -> String {
    M3_TEXT.repeat(M3_REPEAT)
}
