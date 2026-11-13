//! Conversion corpus: PRD FR-CONV-3 acceptance cases and edge cases.

use ks_core::{Confidence, ConvertMode, Converter, Detection, builtin, lang};
use std::sync::OnceLock;

fn conv() -> &'static Converter {
    static C: OnceLock<Converter> = OnceLock::new();
    C.get_or_init(|| builtin().expect("builtin data"))
}

#[track_caller]
fn check(input: &str, from: &str, to: &str, expected: &str) {
    let r = conv().convert(input, from, to).expect("convert");
    assert_eq!(r.text, expected, "input: {input:?}");
}

// ---- PRD FR-CONV-3 (mandatory) ----

#[test]
fn prd_privet_kak_dela() {
    check("Ghbdtn, rfr ltkf?", lang::EN, lang::RU, "Привет, как дела?");
}

#[test]
fn prd_lyublyu() {
    check("k.,k.", lang::EN, lang::RU, "люблю");
}

#[test]
fn prd_obychno() {
    check("j,sxyj", lang::EN, lang::RU, "обычно");
}

#[test]
fn prd_hello() {
    check("Руддщ", lang::RU, lang::EN, "Hello");
}

#[test]
fn prd_test_digits() {
    check("Ntcn 123", lang::EN, lang::RU, "Тест 123");
}

// ---- EN -> RU ----

#[test]
fn physically_typed_punctuation() {
    // What a Russian typist actually produces in the EN layout for "Привет, как дела?".
    check("Ghbdtn? rfr ltkf&", lang::EN, lang::RU, "Привет, как дела?");
}

#[test]
fn word_final_period_kept() {
    check("ytn.", lang::EN, lang::RU, "нет.");
}

#[test]
fn apostrophe_maps_to_e() {
    check("'nj ghjcnj", lang::EN, lang::RU, "это просто");
}

#[test]
fn brackets_map_to_letters() {
    check("[jhjij", lang::EN, lang::RU, "хорошо");
}

#[test]
fn yo_key() {
    check("`krf", lang::EN, lang::RU, "ёлка");
}

#[test]
fn case_preserved() {
    check("GHBDTN vbh", lang::EN, lang::RU, "ПРИВЕТ мир");
}

#[test]
fn whitespace_and_newlines_preserved() {
    check("ghbdtn\n\tvbh  ", lang::EN, lang::RU, "привет\n\tмир  ");
}

#[test]
fn unknown_characters_preserved() {
    check("ghbdtn 😀 «vbh»", lang::EN, lang::RU, "привет 😀 «мир»");
}

#[test]
fn decimal_number_kept() {
    check("xbckj 3.14", lang::EN, lang::RU, "число 3.14");
}

#[test]
fn technical_tokens_with_digits_are_kept() {
    // QA-B1-01: identifiers, versions, hex literals are not words.
    for tok in ["v1.2.3", "0xFF", "x86_64", "a1b2c3"] {
        let c = conv().convert(tok, lang::EN, lang::RU).unwrap();
        assert_eq!(c.text, tok);
        assert_eq!(c.confidence, Confidence::Low);
    }
    // Plain words around numbers still convert, the technical token stays.
    check("d 15", lang::EN, lang::RU, "в 15");
    check("ghbdtn v1.2.3", lang::EN, lang::RU, "привет v1.2.3");
    check("мукышщт 2ю0", lang::RU, lang::EN, "version 2.0");
}

#[test]
fn word_final_key_prefers_the_letter_of_a_dictionary_word() {
    // QA-B1-03: `;` / `.` / `,` at a word end that complete a word are its letter.
    check("'nf;", lang::EN, lang::RU, "этаж");
    check("'nf; 3, jabc 12", lang::EN, lang::RU, "этаж 3, офис 12");
    check("hf,jnf.", lang::EN, lang::RU, "работаю");
    // A sentence end after a complete word keeps its punctuation.
    check(
        "Cgfcb,j pf gjvjom.",
        lang::EN,
        lang::RU,
        "Спасибо за помощь.",
    );
    check("ytn.", lang::EN, lang::RU, "нет.");
}

#[test]
fn number_separators_follow_converted_words() {
    // QA-B1-02: the key between digits is the target layout's separator.
    check("xbckj 3/14", lang::EN, lang::RU, "число 3.14");
    check("dthcbz 2/0", lang::EN, lang::RU, "версия 2.0");
    check("d 15^30", lang::EN, lang::RU, "в 15:30");
    check("'nf; 3? jabc 12", lang::EN, lang::RU, "этаж 3, офис 12");
    // Without converted words the number is left as typed.
    let c = conv().convert("3/14 15^30", lang::EN, lang::RU).unwrap();
    assert_eq!(c.text, "3/14 15^30");
    // Literal separators and technical tokens are not touched.
    check("xbckj 3.14", lang::EN, lang::RU, "число 3.14");
    check("ghbdtn 1/2/ 3", lang::EN, lang::RU, "привет 1.2/ 3");
    check("Руддщ 3.14", lang::RU, lang::EN, "Hello 3.14");
}

fn literal(input: &str, from: &str, to: &str, expected: &str) {
    let c = conv()
        .convert_with(input, from, to, ConvertMode::Literal)
        .unwrap();
    assert_eq!(c.text, expected, "input: {input:?}");
    assert_eq!(c.confidence, Confidence::High, "input: {input:?}");
}

#[test]
fn literal_mode_converts_key_for_key_without_refusals() {
    // D-31: selection mode.
    literal("м1.2.3", lang::RU, lang::EN, "v1.2.3");
    literal("v1.2.3", lang::EN, lang::RU, "м1.2.3");
    literal("ltkf.", lang::EN, lang::RU, "делаю");
    literal("0xFF", lang::EN, lang::RU, "0чАА");
    literal("x86_64", lang::EN, lang::RU, "ч86_64");
    literal("hello", lang::EN, lang::RU, "руддщ");
    literal("xbckj 3/14", lang::EN, lang::RU, "число 3.14");
    literal("d 15^30", lang::EN, lang::RU, "в 15:30");
    literal("Ghbdtn? rfr ltkf&", lang::EN, lang::RU, "Привет, как дела?");
    literal(
        "Cgfcb,j pf gjvjom.",
        lang::EN,
        lang::RU,
        "Спасибо за помощью",
    );
    literal("Руддщб цщкдв!", lang::RU, lang::EN, "Hello, world!");
    // Without letters of the source language there is nothing to be confident about.
    let c = conv()
        .convert_with("3/14", lang::EN, lang::RU, ConvertMode::Literal)
        .unwrap();
    assert_eq!(c.confidence, Confidence::Low);
    // Mixed selection (language picked in the popup): foreign tokens key for key.
    let m = conv()
        .convert_mixed_with("привет vbh v1.2.3", lang::RU, ConvertMode::Literal)
        .unwrap();
    assert_eq!(m.text, "привет мир м1.2.3");
}

#[test]
fn smart_mode_keeps_its_protections() {
    // D-31: caret mode is unchanged.
    for tok in ["v1.2.3", "м1.2.3"] {
        let from = if tok.starts_with('v') {
            lang::EN
        } else {
            lang::RU
        };
        let to = if from == lang::EN { lang::RU } else { lang::EN };
        let c = conv()
            .convert_with(tok, from, to, ConvertMode::Smart)
            .unwrap();
        assert_eq!(c.text, tok);
        assert_eq!(c.confidence, Confidence::Low);
    }
}

// ---- RU -> EN ----

#[test]
fn ru_to_en_comma_from_letter() {
    check("Руддщб цщкдв!", lang::RU, lang::EN, "Hello, world!");
}

#[test]
fn ru_to_en_apostrophe() {
    check("ш вщтэе лтщц", lang::RU, lang::EN, "i don't know");
}

#[test]
fn ru_to_en_period_from_letter() {
    check("ыуу нщгю", lang::RU, lang::EN, "see you.");
}

// ---- detection, confidence, mixed text ----

#[test]
fn detection() {
    assert_eq!(conv().detect("Ghbdtn"), Detection::Single(lang::EN.into()));
    assert_eq!(conv().detect("Привет"), Detection::Single(lang::RU.into()));
    assert_eq!(conv().detect("123 ?!"), Detection::NoLetters);
    assert_eq!(
        conv().detect("Привет, rfr ltkf?"),
        Detection::Mixed(vec![lang::EN.into(), lang::RU.into()])
    );
}

#[test]
fn wrong_layout_text_is_high_confidence() {
    let r = conv()
        .convert("Ghbdtn, rfr ltkf?", lang::EN, lang::RU)
        .unwrap();
    assert_eq!(r.confidence, Confidence::High);
}

#[test]
fn correct_text_is_low_confidence() {
    // Real English typed correctly: converting it would be wrong.
    let r = conv()
        .convert("Hello, how are you?", lang::EN, lang::RU)
        .unwrap();
    assert_eq!(r.confidence, Confidence::Low);
}

#[test]
fn mixed_to_russian() {
    let r = conv().convert_mixed("Привет, rfr ltkf?", lang::RU).unwrap();
    assert_eq!(r.text, "Привет, как дела?");
    assert_eq!(r.confidence, Confidence::High);
}

#[test]
fn mixed_to_english() {
    let r = conv().convert_mixed("Hello, руку ш фь", lang::EN).unwrap();
    assert_eq!(r.text, "Hello, here i am");
}

#[test]
fn mixed_keeps_genuine_foreign_word() {
    let r = conv()
        .convert_mixed("Открой google, gj;fkeqcnf", lang::RU)
        .unwrap();
    assert_eq!(r.text, "Открой google, пожалуйста");
}

#[test]
fn mixed_auto_converts_only_confident_tokens() {
    let r = conv().convert_mixed_auto("hello цщкдв", ConvertMode::Smart);
    assert_eq!(
        (r.text.as_str(), r.confidence),
        ("hello world", Confidence::High)
    );
    assert_eq!((r.converted_tokens, r.kept_tokens), (1, 1));
    // Technical tokens and digits stay, wrong words around them are fixed.
    let r = conv().convert_mixed_auto("версия v1.2.3 ujnjdf", ConvertMode::Smart);
    assert_eq!(r.text, "версия v1.2.3 готова");
    let r = conv().convert_mixed_auto("на user@example.com ctujlyz в 10", ConvertMode::Smart);
    assert_eq!(r.text, "на user@example.com сегодня в 10");
}

#[test]
fn mixed_auto_is_low_when_nothing_is_clear() {
    for text in ["привет world", "aб", "ghbdtnмир"] {
        let r = conv().convert_mixed_auto(text, ConvertMode::Smart);
        assert_eq!((r.text.as_str(), r.confidence), (text, Confidence::Low));
    }
    // Selection: the guess must equal the key-for-key re-typing (`,` would become `б`).
    let r = conv().convert_mixed_auto("ghbdtn, как дела?", ConvertMode::Literal);
    assert_eq!(r.confidence, Confidence::Low);
    let r = conv().convert_mixed_auto("ghbdtn, как дела?", ConvertMode::Smart);
    assert_eq!(r.text, "привет, как дела?");
}

#[test]
fn phrase_keeps_genuine_words_of_the_typed_language() {
    let r = conv().convert("cgfcb,j, John", lang::EN, lang::RU).unwrap();
    assert_eq!(
        (r.text.as_str(), r.confidence),
        ("спасибо, John", Confidence::High)
    );
    // A kept word demands that every converted word is confidently wrong on its own.
    let r = conv().convert("npm install", lang::EN, lang::RU).unwrap();
    assert_eq!(r.confidence, Confidence::Low);
}

#[test]
fn errors() {
    assert!(conv().convert("x", lang::EN, lang::EN).is_err());
    assert!(conv().convert("x", "de", lang::RU).is_err());
}

#[test]
fn debug_does_not_leak_text() {
    let r = conv().convert("ctrhtn", lang::EN, lang::RU).unwrap();
    let dbg = format!("{r:?}");
    assert!(!dbg.contains("секрет") && !dbg.contains("ctrhtn"), "{dbg}");
}

// ---- invariants on pseudo-random input (deterministic, no extra dependencies) ----

#[test]
fn invariants_on_random_input() {
    let alphabet: Vec<char> = "abcxyzABC,.;'[]`?&@#$^/|{}<>~ \n\t123йцукЁё!😀«»-_"
        .chars()
        .collect();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..300 {
        let len = (next() % 80) as usize;
        let input: String = (0..len)
            .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
            .collect();
        for (from, to) in [(lang::EN, lang::RU), (lang::RU, lang::EN)] {
            let out = conv().convert(&input, from, to).unwrap().text;
            assert_eq!(out.chars().count(), input.chars().count());
            let ws = |s: &str| -> Vec<(usize, char)> {
                s.chars()
                    .enumerate()
                    .filter(|(_, c)| c.is_whitespace())
                    .collect()
            };
            assert_eq!(ws(&out), ws(&input), "whitespace must be preserved");
            let digits = |s: &str| -> Vec<(usize, char)> {
                s.chars()
                    .enumerate()
                    .filter(|(_, c)| c.is_ascii_digit())
                    .collect()
            };
            assert_eq!(digits(&out), digits(&input), "digits must be preserved");
        }
        let m = conv().convert_mixed(&input, lang::RU).unwrap().text;
        assert_eq!(m.chars().count(), input.chars().count());
        for mode in [ConvertMode::Smart, ConvertMode::Literal] {
            let a = conv().convert_mixed_auto(&input, mode);
            assert_eq!(a.text.chars().count(), input.chars().count());
            if a.confidence == Confidence::Low {
                assert_eq!(
                    a.text, input,
                    "an unclear decision must not change the text"
                );
            }
        }
    }
}
