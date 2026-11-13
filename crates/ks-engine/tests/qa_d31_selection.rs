//! BL-QA-B1 final re-test: independent spot-check of owner decision D-31 (selection =
//! literal key-for-key conversion; `.` `,` `:` between two digits stay) through the real
//! pipeline `ks_engine::run`, not through `ks-core` directly. Expected values are written by
//! hand from the Windows layouts US / Russian (KLID 00000419). Synthetic text only.

use ks_engine::{Config, Outcome, Platform, Stop, TargetInfo, UserText, run};

/// Whole content selected; records the write.
struct Selected {
    text: String,
    writes: usize,
    caret_reads: usize,
}

impl Platform for Selected {
    fn begin(&mut self) -> Result<TargetInfo, Stop> {
        Ok(TargetInfo {
            exe: "notepad.exe".into(),
        })
    }
    fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        let units: Vec<u16> = self.text.encode_utf16().take(max_units).collect();
        Ok(Some(UserText::new(String::from_utf16_lossy(&units))))
    }
    fn read_before_caret(&mut self, _max_units: usize) -> Result<Option<UserText>, Stop> {
        self.caret_reads += 1;
        Ok(None)
    }
    fn select_before_caret(&mut self, _units: usize) -> Result<(), Stop> {
        unreachable!("selection mode never selects by itself")
    }
    fn write(&mut self, text: &UserText) -> Result<(), Stop> {
        text.expose().clone_into(&mut self.text);
        self.writes += 1;
        Ok(())
    }
    fn verify(&mut self, expected: &UserText) -> bool {
        self.text == expected.expose()
    }
    fn switch_layout(&mut self, _lang: &str) -> bool {
        true
    }
}

/// Field content after one hotkey press on the selected `input`, plus the outcome code.
fn press(input: &str) -> (String, &'static str) {
    static CONVERTER: std::sync::OnceLock<ks_core::Converter> = std::sync::OnceLock::new();
    let c = CONVERTER.get_or_init(|| ks_core::builtin().expect("built-in data"));
    let mut f = Selected {
        text: input.to_owned(),
        writes: 0,
        caret_reads: 0,
    };
    let outcome = run(&mut f, c, &Config::default());
    assert_eq!(
        f.caret_reads, 0,
        "a selection must not fall back to caret mode"
    );
    match outcome {
        Outcome::Converted { .. } => assert_eq!(f.writes, 1),
        _ => assert_eq!(f.writes, 0),
    }
    (f.text, outcome.code())
}

/// (selected input, expected field content)
const CASES: [(&str, &str); 36] = [
    // D-31 examples from docs/00-decisions.md
    ("м1.2.3", "v1.2.3"),
    ("v1.2.3", "м1.2.3"),
    ("ltkf.", "делаю"),
    ("hello", "руддщ"),
    // Correct-looking text is re-typed too (no smart refusal in selection mode)
    ("привет", "ghbdtn"),
    ("0xFF", "0чАА"),
    ("x86_64", "ч86_64"),
    // Number separators: only . , : between two digits stay
    ("xbckj 1,5", "число 1,5"),
    ("d 10:30", "в 10:30"),
    ("d 15^30", "в 15:30"),
    ("xbckj 3/14", "число 3.14"),
    ("ntcn 1.", "тест 1ю"),
    ("v1,b", "м1би"),
    ("f 1;2", "а 1ж2"),
    ("руддщ 1,5", "hello 1,5"),
    ("руддщ 1:5", "hello 1:5"),
    ("руддщ.", "hello/"),
    ("руддщ,", "hello?"),
    // Shifted digit row
    ("f !@#$%^&*()", "а !\"№;%:?*()"),
    ("Ghbdtn? rfr ltkf&", "Привет, как дела?"),
    // Case: Shift and CapsLock-style inversions are kept per character
    ("gHBDTN", "пРИВЕТ"),
    ("ПРИВЕТ", "GHBDTN"),
    ("Ghbdtn", "Привет"),
    ("рЕЛЛО", "hTKKJ"),
    // Letters on punctuation keys
    ("`krf ~", "ёлка Ё"),
    ("[jhjij]", "хорошоъ"),
    ("ш вщтэе", "i don't"),
    ("'nj", "это"),
    // Surrogate pairs and unmapped characters are kept in place
    ("ghbdtn\u{1F600}", "привет\u{1F600}"),
    ("\u{1F600}руддщ", "\u{1F600}hello"),
    ("a\u{1F600}b", "ф\u{1F600}и"),
    ("ghbdtn — «vbh»", "привет — «мир»"),
    // Whitespace runs and line breaks survive
    ("ghbdtn\r\n\tvbh  ", "привет\r\n\tмир  "),
    // Nothing to convert: no letters of a known language
    ("1.2.3", "1.2.3"),
    ("12:30 3,5", "12:30 3,5"),
    ("\u{1F600}\u{1F600}", "\u{1F600}\u{1F600}"),
];

#[test]
fn d31_selection_spot_check() {
    let mut failures = Vec::new();
    for (input, expected) in CASES {
        let (actual, code) = press(input);
        if actual != expected {
            failures.push(format!(
                "in={input:?} expected={expected:?} actual={actual:?} code={code}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "D-31 failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn d31_selection_is_reversible_for_letter_text() {
    // Key-for-key both ways returns the original when every character is on a mapped key
    // and no separator sits between digits.
    for s in [
        "ghbdtn vbh",
        "Ghbdtn? rfr ltkf&",
        "gHBDTN",
        "[jhjij]",
        "`krf",
    ] {
        let (once, _) = press(s);
        let (twice, _) = press(&once);
        assert!(
            twice == s,
            "round trip failed for {s:?}: {once:?} -> {twice:?}"
        );
    }
}

#[test]
fn d31_letterless_selection_is_nothing() {
    for s in ["1.2.3", "12:30 3,5", "!!!", "\u{1F600}"] {
        assert_eq!(press(s).1, "NOTHING", "{s:?}");
    }
}
