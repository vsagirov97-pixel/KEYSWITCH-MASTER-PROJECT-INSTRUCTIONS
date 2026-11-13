//! BL-QA-B1 item 2: edge cases of the Beta 1 pipeline through the public API of `ks-engine`.
//!
//! All text here is synthetic. The fake platform is an in-memory field of UTF-16 units; in
//! `strict` mode its `write` behaves like the Windows W3 method (refuses control and format
//! characters without changing anything).

use ks_engine::{Config, Failure, Outcome, Platform, Refusal, Stop, TargetInfo, UserText, run};

struct Field {
    text: Vec<u16>,
    sel: (usize, usize),
    exe: &'static str,
    /// W3-like write: control / format characters are refused.
    strict: bool,
    writes: usize,
    selects: usize,
    max_read_request: usize,
    layout: Option<String>,
}

fn forbidden(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00AD}' | '\u{200B}'..='\u{200F}' | '\u{2028}' | '\u{2029}'
            | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

impl Field {
    fn selected(text: &str, a: usize, b: usize) -> Self {
        Self {
            text: text.encode_utf16().collect(),
            sel: (a, b),
            exe: "notepad.exe",
            strict: false,
            writes: 0,
            selects: 0,
            max_read_request: 0,
            layout: None,
        }
    }
    fn all_selected(text: &str) -> Self {
        Self::selected(text, 0, text.encode_utf16().count())
    }
    fn caret_at_end(text: &str) -> Self {
        let n = text.encode_utf16().count();
        Self::selected(text, n, n)
    }
    fn content(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }
    fn slice(&self, a: usize, b: usize) -> UserText {
        UserText::new(String::from_utf16_lossy(&self.text[a..b]))
    }
}

impl Platform for Field {
    fn begin(&mut self) -> Result<TargetInfo, Stop> {
        Ok(TargetInfo {
            exe: self.exe.into(),
        })
    }
    fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        self.max_read_request = self.max_read_request.max(max_units);
        let (a, b) = self.sel;
        if a == b {
            return Ok(None);
        }
        Ok(Some(self.slice(a, b.min(a + max_units))))
    }
    fn read_before_caret(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        self.max_read_request = self.max_read_request.max(max_units);
        let end = self.sel.0;
        Ok(Some(self.slice(end.saturating_sub(max_units), end)))
    }
    fn select_before_caret(&mut self, units: usize) -> Result<(), Stop> {
        self.selects += 1;
        self.sel = (self.sel.0 - units, self.sel.0);
        Ok(())
    }
    fn write(&mut self, text: &UserText) -> Result<(), Stop> {
        if self.strict && text.expose().chars().any(forbidden) {
            return Err(Stop::Refused(Refusal::ForbiddenChars));
        }
        let (a, b) = self.sel;
        let new: Vec<u16> = text.expose().encode_utf16().collect();
        let n = new.len();
        self.text.splice(a..b, new);
        self.sel = (a + n, a + n);
        self.writes += 1;
        Ok(())
    }
    fn verify(&mut self, expected: &UserText) -> bool {
        let end = self.sel.0;
        let n = expected.utf16_len();
        end >= n && self.slice(end - n, end).same_as(expected)
    }
    fn restore_caret(&mut self, selected: &UserText) {
        let (a, b) = self.sel;
        if a < b && self.slice(a, b).same_as(selected) {
            self.sel = (b, b);
        }
    }
    fn switch_layout(&mut self, lang: &str) -> bool {
        self.layout = Some(lang.to_owned());
        true
    }
}

fn go(f: &mut Field) -> Outcome {
    go_with(f, &Config::default())
}

fn go_with(f: &mut Field, cfg: &Config) -> Outcome {
    static CONVERTER: std::sync::OnceLock<ks_core::Converter> = std::sync::OnceLock::new();
    let c = CONVERTER.get_or_init(|| ks_core::builtin().expect("built-in data"));
    run(f, c, cfg)
}

const OK: Outcome = Outcome::Converted {
    verified: true,
    layout_switched: true,
};

/// Asserts that the operation changed nothing and returns the code.
#[track_caller]
fn untouched(mut f: Field) -> &'static str {
    let before = f.content();
    let outcome = go(&mut f);
    assert_eq!(f.writes, 0, "no write expected, outcome {outcome:?}");
    assert!(
        f.content() == before,
        "content changed, outcome {outcome:?}"
    );
    assert_eq!(f.layout, None, "layout must not be switched");
    outcome.code()
}

// ---- empty / whitespace ----

#[test]
fn empty_and_whitespace_only_do_nothing() {
    for text in [
        "", " ", "   ", "\t", "\n", "\r\n", " \t \n ", "\u{00A0}", "\u{3000}",
    ] {
        assert_eq!(untouched(Field::caret_at_end(text)), "NOTHING");
        if !text.is_empty() {
            assert_eq!(untouched(Field::all_selected(text)), "NOTHING");
        }
    }
}

#[test]
fn caret_after_line_break_or_tab_does_nothing() {
    for text in [
        "ghbdtn\n",
        "ghbdtn\r\n",
        "ghbdtn\t",
        "ghbdtn \n",
        "ghbdtn\u{00A0}",
    ] {
        assert_eq!(untouched(Field::caret_at_end(text)), "NOTHING");
    }
}

#[test]
fn caret_word_stops_at_tab_and_newline() {
    for (text, expected) in [
        ("abc\tghbdtn", "abc\tпривет"),
        ("abc\nghbdtn", "abc\nпривет"),
        ("abc\r\nghbdtn  ", "abc\r\nпривет  "),
    ] {
        let mut f = Field::caret_at_end(text);
        assert_eq!(go(&mut f), OK);
        assert!(f.content() == expected);
        assert_eq!(f.writes, 1);
    }
}

// ---- length limit (UTF-16 units) ----

#[test]
fn selection_limit_is_200_units_inclusive() {
    // "ghbdtn " is 7 units; 28 * 7 = 196, plus "vbh " = 200.
    let at_limit = format!("{}vbh ", "ghbdtn ".repeat(28));
    assert_eq!(at_limit.encode_utf16().count(), 200);
    let mut f = Field::all_selected(&at_limit);
    assert_eq!(go(&mut f), OK);
    assert!(f.content().starts_with("привет привет"));
    assert_eq!(f.content().encode_utf16().count(), 200);

    let over = format!("{at_limit}v");
    assert_eq!(untouched(Field::all_selected(&over)), "TOO_LONG");
}

#[test]
fn huge_selection_is_refused_and_never_read_in_full() {
    let huge = "ghbdtn ".repeat(5000);
    let mut f = Field::all_selected(&huge);
    assert_eq!(go(&mut f), Outcome::Refused(Refusal::TooLong));
    assert_eq!(f.writes, 0);
    assert!(
        f.max_read_request <= 201,
        "the engine asked for {} units",
        f.max_read_request
    );
}

#[test]
fn caret_word_filling_the_window_is_refused() {
    assert_eq!(untouched(Field::caret_at_end(&"g".repeat(64))), "TOO_LONG");
    assert_eq!(untouched(Field::caret_at_end(&"g".repeat(500))), "TOO_LONG");
}

// ---- surrogate pairs ----

#[test]
fn surrogate_pair_counts_as_two_units_at_the_limit() {
    // 28 * 7 = 196 units + "vb" + emoji (2 units) = 200.
    let at_limit = format!("{}vb\u{1F600}", "ghbdtn ".repeat(28));
    assert_eq!(at_limit.encode_utf16().count(), 200);
    let mut f = Field::all_selected(&at_limit);
    assert_eq!(go(&mut f), OK);
    assert!(f.content().ends_with('\u{1F600}'));

    // 201 units; the engine reads 201, so the pair is complete.
    let over = format!("{}vbh\u{1F600}", "ghbdtn ".repeat(28));
    assert_eq!(over.encode_utf16().count(), 201);
    assert_eq!(untouched(Field::all_selected(&over)), "TOO_LONG");

    // 202 units; the 201-unit read cuts the pair in half.
    let cut = format!("{}vbhf\u{1F600}", "ghbdtn ".repeat(28));
    assert_eq!(cut.encode_utf16().count(), 202);
    assert_eq!(untouched(Field::all_selected(&cut)), "TOO_LONG");
}

#[test]
fn emoji_inside_the_fragment_is_preserved() {
    let mut f = Field::caret_at_end("\u{1F600}ghbdtn");
    assert_eq!(go(&mut f), OK);
    assert!(f.content() == "\u{1F600}привет");

    let mut f = Field::all_selected("ghbdtn \u{1F468}\u{200D}\u{1F469} vbh");
    assert_eq!(go(&mut f), OK);
    assert!(f.content() == "привет \u{1F468}\u{200D}\u{1F469} мир");
}

#[test]
fn caret_window_cutting_a_surrogate_pair_is_refused() {
    // 65 units: the 64-unit look-back starts at the low surrogate.
    let text = format!("\u{1F600}{}", "g".repeat(63));
    assert_eq!(untouched(Field::caret_at_end(&text)), "TOO_LONG");
}

#[test]
fn text_without_known_letters_does_nothing() {
    for text in [
        "12345",
        "!!! ???",
        "3.14 + 2 = 5.14",
        "\u{1F600}\u{1F600}",
        "αβγ δ",
        "日本語",
        "\u{202E}\u{200B}",
    ] {
        assert_eq!(untouched(Field::all_selected(text)), "NOTHING");
        assert_eq!(untouched(Field::caret_at_end(text)), "NOTHING");
    }
}

// ---- control / bidi / zero-width ----

#[test]
fn strict_write_refuses_forbidden_characters_without_change() {
    for text in [
        "ghbdtn\nvbh",
        "ghbdtn\tvbh",
        "ghbdtn\r\nvbh",
        "ghbdtn\u{202E}vbh",
        "ghbdtn \u{200B}vbh",
        "ghbdtn \u{FEFF}vbh",
        "ghbdtn\u{2028}vbh",
        "gh\u{0001}bdtn vbh",
    ] {
        let mut f = Field::all_selected(text);
        f.strict = true;
        let outcome = go(&mut f);
        assert_eq!(f.writes, 0);
        assert!(f.content() == text);
        assert_eq!(f.layout, None);
        // Either the converter is not confident or the write is refused; never a change.
        assert!(
            matches!(
                outcome,
                Outcome::Refused(Refusal::ForbiddenChars | Refusal::LowConfidence)
            ),
            "{outcome:?}"
        );
    }
}

#[test]
fn permissive_write_keeps_every_unmapped_character() {
    // W1-like target: whatever the outcome, control/format characters are neither dropped
    // nor added, and their order is kept.
    for text in [
        "ghbdtn\nvbh",
        "ghbdtn\r\n\tvbh",
        "ghbdtn\u{202E}vbh\u{202C}",
        "ghbdtn\u{200B} vbh\u{200D}",
        "\u{FEFF}ghbdtn vbh",
    ] {
        let special = |s: &str| -> String {
            s.chars()
                .filter(|c| forbidden(*c) || *c == '\u{200D}')
                .collect()
        };
        let mut f = Field::all_selected(text);
        let outcome = go(&mut f);
        assert!(f.writes <= 1);
        assert!(special(&f.content()) == special(text), "{outcome:?}");
        assert_eq!(
            f.content().encode_utf16().count(),
            text.encode_utf16().count(),
            "{outcome:?}"
        );
    }
}

/// QA-B1-04 (see docs/qa/beta1-qa-report.md): in a caret-only operation the engine selects the
/// word *before* the write method can refuse the text, so a refused/failed write leaves the
/// word selected - the next key the user presses replaces it. This test documents the current
/// behaviour; when the defect is fixed the final assertion must be inverted.
#[test]
fn qa_b1_04_refused_write_after_caret_selection_restores_the_caret() {
    let text = "abc ghbdtn\u{200B}";
    let mut f = Field::caret_at_end(text);
    f.strict = true;
    let outcome = go(&mut f);
    assert_eq!(outcome, Outcome::Refused(Refusal::ForbiddenChars));
    assert_eq!(f.writes, 0);
    assert!(f.content() == text);
    assert_eq!(f.selects, 1);
    let (a, b) = f.sel;
    assert!(a == b, "QA-B1-04 fixed: nothing stays selected");
    assert_eq!(
        b,
        text.encode_utf16().count(),
        "the caret is back at the end"
    );
}

// ---- confidence / mixed ----

#[test]
fn correct_text_is_low_confidence() {
    for text in [
        "hello",
        "hello world",
        "Thanks for your help.",
        "привет",
        "Привет, как дела?",
        "это",
    ] {
        // D-31: a selection is converted key for key, even text that looks correct.
        let mut f = Field::all_selected(text);
        assert!(matches!(go(&mut f), Outcome::Converted { .. }));
        assert!(f.content() != text);
        assert_eq!(untouched(Field::caret_at_end(text)), "LOW_CONFIDENCE");
    }
}

#[test]
fn mixed_languages_are_refused() {
    for text in ["привет world", "ghbdtn мир", "aб"] {
        assert_eq!(untouched(Field::all_selected(text)), "MIXED");
    }
    // Caret mode looks at the last word only.
    assert_eq!(untouched(Field::caret_at_end("ghbdtnмир")), "MIXED");
}

// ---- excluded apps ----

#[test]
fn excluded_list_matches_normalised_names() {
    for raw in ["KeePass.EXE", "  keepass.exe ", "KEEPASS.exe"] {
        let name = ks_engine::normalize_exe(raw).expect("valid name");
        let mut f = Field::caret_at_end("ghbdtn");
        f.exe = "keepass.exe";
        let cfg = Config {
            excluded: vec![name],
            ..Config::default()
        };
        assert_eq!(
            go_with(&mut f, &cfg),
            Outcome::Refused(Refusal::ExcludedApp)
        );
        assert_eq!(f.writes, 0);
        assert_eq!(f.max_read_request, 0, "excluded app: nothing may be read");
    }
}

// ---- result codes ----

#[test]
fn result_codes_are_unique_ascii_and_stable() {
    let refusals = [
        (Refusal::Password, "PASSWORD"),
        (Refusal::Elevated, "ELEVATED"),
        (Refusal::SecureDesktop, "SECURE_DESKTOP"),
        (Refusal::SelfTarget, "SELF_TARGET"),
        (Refusal::Console, "CONSOLE"),
        (Refusal::ExcludedApp, "EXCLUDED_APP"),
        (Refusal::ReadOnly, "READ_ONLY"),
        (Refusal::TooLong, "TOO_LONG"),
        (Refusal::LowConfidence, "LOW_CONFIDENCE"),
        (Refusal::Mixed, "MIXED"),
        (Refusal::Unsupported, "UNSUPPORTED"),
        (Refusal::ForbiddenChars, "FORBIDDEN_CHARS"),
    ];
    let failures = [
        (Failure::TargetChanged, "TARGET_CHANGED"),
        (Failure::ModifiersHeld, "MODIFIERS_HELD"),
        (Failure::Read, "READ_FAILED"),
        (Failure::Write, "WRITE_FAILED"),
    ];
    let mut codes = vec![
        OK.code(),
        Outcome::Converted {
            verified: false,
            layout_switched: false,
        }
        .code(),
        Outcome::Nothing.code(),
    ];
    assert_eq!(codes, ["CONVERTED", "CONVERTED_UNVERIFIED", "NOTHING"]);
    for (r, code) in refusals {
        assert_eq!(Outcome::Refused(r).code(), code);
        codes.push(code);
    }
    for (f, code) in failures {
        assert_eq!(Outcome::Failed(f).code(), code);
        codes.push(code);
    }
    let mut unique = codes.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), codes.len());
    assert!(
        codes
            .iter()
            .all(|c| { c.bytes().all(|b| b.is_ascii_uppercase() || b == b'_') })
    );
}

// ---- invariants on generated input ----

#[test]
fn generated_input_never_changes_text_outside_the_selection() {
    const ALPHABET: [&str; 24] = [
        "g",
        "h",
        "b",
        "d",
        "t",
        "n",
        "v",
        " ",
        " ",
        ",",
        ".",
        ";",
        "'",
        "[",
        "1",
        "п",
        "р",
        "\n",
        "\t",
        "\u{1F600}",
        "\u{200B}",
        "\u{202E}",
        "Q",
        "-",
    ];
    let mut state: u64 = 0x5EED_B1B1;
    let mut next = move |n: usize| -> usize {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % n
    };
    let mut converted = 0;
    for _ in 0..3000 {
        let len = next(40);
        let pieces: Vec<&str> = (0..len).map(|_| ALPHABET[next(ALPHABET.len())]).collect();
        // Selection boundaries on piece boundaries, so no surrogate pair is cut by the test.
        let (i, j) = (next(len + 1), next(len + 1));
        let (i, j) = (i.min(j), i.max(j));
        let units = |p: &[&str]| p.concat().encode_utf16().count();
        let (a, b) = (units(&pieces[..i]), units(&pieces[..j]));
        let text = pieces.concat();
        let prefix_all = pieces[..i].concat();
        let suffix = pieces[j..].concat();

        let mut f = Field::selected(&text, a, b);
        f.strict = next(2) == 0;
        let outcome = go(&mut f);
        let after = f.content();
        assert!(f.writes <= 1);
        assert!(after.ends_with(&suffix), "text after the selection changed");
        if a == b {
            // Caret mode may rewrite only the tail of the prefix (the last word).
            let trimmed = prefix_all.trim_end_matches(' ');
            let word_start = trimmed.rfind(char::is_whitespace).map_or(0, |p| {
                p + trimmed[p..].chars().next().map_or(1, char::len_utf8)
            });
            assert!(
                after.starts_with(&prefix_all[..word_start]),
                "text before the last word changed"
            );
        } else {
            assert!(
                after.starts_with(&prefix_all),
                "text before the selection changed"
            );
        }
        match outcome {
            Outcome::Converted { .. } => {
                converted += 1;
                assert_eq!(f.writes, 1);
            }
            _ => {
                assert_eq!(f.writes, 0);
                assert!(after == text, "non-converted outcome changed the text");
                assert_eq!(f.layout, None);
            }
        }
    }
    assert!(
        converted > 0,
        "the generator never produced a convertible case"
    );
}

// ---- user text cannot be formatted (CLAUDE.md rule 1, T-09) ----

/// Compile-time: `UserText` implements none of `Debug`, `Display`, `Clone`.
/// If it gains one of them, the corresponding line stops compiling (ambiguous impl).
#[test]
fn user_text_has_no_formatting_or_clone_impls() {
    trait NotDebug<A> {
        fn ok() {}
    }
    impl<T: ?Sized> NotDebug<()> for T {}
    impl<T: ?Sized + std::fmt::Debug> NotDebug<u8> for T {}
    trait NotDisplay<A> {
        fn ok() {}
    }
    impl<T: ?Sized> NotDisplay<()> for T {}
    impl<T: ?Sized + std::fmt::Display> NotDisplay<u8> for T {}
    trait NotClone<A> {
        fn ok() {}
    }
    impl<T: ?Sized> NotClone<()> for T {}
    impl<T: Clone> NotClone<u8> for T {}

    <UserText as NotDebug<_>>::ok();
    <UserText as NotDisplay<_>>::ok();
    <UserText as NotClone<_>>::ok();
}

#[test]
fn conversion_debug_output_has_no_text() {
    let c = ks_core::builtin().expect("built-in data");
    let r = c.convert("ghbdtn", "en", "ru").expect("convert");
    let dbg = format!("{r:?}");
    assert!(!dbg.contains("привет") && !dbg.contains("ghbdtn"));
}

/// Source guard: a line that exposes user text must not also format, print, log or panic.
/// (`expose()` is the only way out of `UserText`.) Covers production sources of the engine,
/// the Windows adapter and the Tauri shell.
#[test]
fn exposed_text_is_never_formatted_in_sources() {
    const SINKS: [&str; 14] = [
        "format!",
        "print!",
        "println!",
        "eprint",
        "panic!",
        "expect(",
        "unwrap(",
        "assert",
        "dbg!",
        "log::",
        "tracing::",
        "write!",
        "{:?}",
        "OutputDebugString",
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for dir in [
        "crates/ks-engine/src",
        "crates/ks-platform-win/src",
        "app/src-tauri/src",
    ] {
        let entries = std::fs::read_dir(root.join(dir)).expect("source dir");
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).expect("source file");
            // Unit-test modules use synthetic text and may assert on it.
            let production = src.split("#[cfg(test)]").next().unwrap_or("");
            for (n, line) in production.lines().enumerate() {
                if !line.contains("expose()") {
                    continue;
                }
                checked += 1;
                for sink in SINKS {
                    assert!(
                        !line.contains(sink),
                        "{}:{} exposes user text next to `{sink}`",
                        path.display(),
                        n + 1
                    );
                }
            }
        }
    }
    assert!(
        checked >= 5,
        "guard found only {checked} expose() call sites"
    );
}
