//! Regressions for QA-WF-01 (mixed selection destroyed a correct all-caps abbreviation) and
//! QA-WF-02 (caret mode converted a correct all-caps abbreviation; the second press did not
//! undo a wrong conversion). Cases from `docs/qa/windows-product-qa-report.md`; invented text.

use ks_engine::{Config, History, Outcome, Platform, Stop, TargetInfo, UserText, run_with_history};

/// A single-line field with the caret at the end and an optional selection at its end.
struct Field {
    text: String,
    selected: Option<usize>,
    key: Option<u64>,
}

impl Field {
    fn tail(&self, units: usize) -> String {
        let u: Vec<u16> = self.text.encode_utf16().collect();
        String::from_utf16_lossy(&u[u.len().saturating_sub(units)..])
    }
}

impl Platform for Field {
    fn begin(&mut self) -> Result<TargetInfo, Stop> {
        Ok(TargetInfo {
            exe: "notepad.exe".into(),
        })
    }
    fn read_selection(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        Ok(self.selected.map(|n| {
            let s = &self.text[self.text.len() - n..];
            let units: Vec<u16> = s.encode_utf16().take(max_units).collect();
            UserText::new(String::from_utf16_lossy(&units))
        }))
    }
    fn read_before_caret(&mut self, max_units: usize) -> Result<Option<UserText>, Stop> {
        Ok(Some(UserText::new(self.tail(max_units))))
    }
    fn select_before_caret(&mut self, units: usize) -> Result<(), Stop> {
        self.selected = Some(self.tail(units).len());
        Ok(())
    }
    fn restore_caret(&mut self, _selected: &UserText) {
        self.selected = None;
    }
    fn write(&mut self, text: &UserText) -> Result<(), Stop> {
        let n = self.selected.take().unwrap_or(0);
        self.text.truncate(self.text.len() - n);
        self.text.push_str(text.expose());
        Ok(())
    }
    fn verify(&mut self, expected: &UserText) -> bool {
        self.text.ends_with(expected.expose())
    }
    fn switch_layout(&mut self, _lang: &str) -> bool {
        true
    }
    fn field_key(&self) -> Option<u64> {
        self.key
    }
}

fn converter() -> &'static ks_core::Converter {
    static C: std::sync::OnceLock<ks_core::Converter> = std::sync::OnceLock::new();
    C.get_or_init(|| ks_core::builtin().expect("built-in data"))
}

fn press(f: &mut Field, h: &mut History) -> Outcome {
    run_with_history(f, converter(), &Config::default(), h)
}

fn field(text: &str, select_all: bool) -> Field {
    Field {
        text: text.to_owned(),
        selected: select_all.then_some(text.len()),
        key: Some(7),
    }
}

fn once(text: &str, select_all: bool) -> String {
    let mut f = field(text, select_all);
    let _ = press(&mut f, &mut History::default());
    f.text
}

#[test]
fn qa_wf_01_mixed_selection_keeps_correct_abbreviations() {
    for (input, expected) in [
        ("Send it to ЖКХ ещвфн", "Send it to ЖКХ today"),
        ("Send it to ЖКХ", "Send it to ЖКХ"),
        ("Call ЖЭК ещвфн", "Call ЖЭК today"),
        ("Ask ЖЭК", "Ask ЖЭК"),
        ("ФСБ and ЖКХ", "ФСБ and ЖКХ"),
        ("My ЖЖ blog", "My ЖЖ blog"),
        ("Отправь PDF yf почту", "Отправь PDF на почту"),
        ("Pay the НДС ещьщккщц", "Pay the НДС tomorrow"),
        ("Report for ФНС is ready", "Report for ФНС is ready"),
    ] {
        assert_eq!(once(input, true), expected, "selection {input:?}");
    }
}

#[test]
fn qa_wf_02_caret_keeps_correct_abbreviations() {
    for input in [
        "Скачай PDF",
        "Это HTTP",
        "Это HTTPS",
        "Paid via ФНС",
        "Call ЖКХ",
        "Это ПДФ",
        "Отправь USB",
        "Нужен API",
        "Звони в IT",
        "Код SQL",
        "Сделано по ГОСТ",
        "Зарегистрировал ИП",
        "Paid via СБП",
    ] {
        let mut f = field(input, false);
        let o = press(&mut f, &mut History::default());
        assert_eq!(f.text, input, "caret {input:?}");
        assert_eq!(o, Outcome::Nothing, "no language question for {input:?}");
    }
}

#[test]
fn qa_wf_02_caret_converts_a_mistyped_abbreviation() {
    assert_eq!(once("Скачай ЗВА", false), "Скачай PDF");
    assert_eq!(once("Ссылка: гкд", false), "Ссылка: url");
    assert_eq!(once("Paid via AYC", false), "Paid via ФНС");
}

#[test]
fn qa_wf_02_second_press_undoes_the_conversion() {
    let mut h = History::default();
    let mut f = field("Я сейчас ljvf", false);
    assert!(matches!(press(&mut f, &mut h), Outcome::Converted { .. }));
    assert_eq!(f.text, "Я сейчас дома");
    assert!(matches!(press(&mut f, &mut h), Outcome::Converted { .. }));
    assert_eq!(
        f.text, "Я сейчас ljvf",
        "second press restores the typed text"
    );
    assert!(matches!(press(&mut f, &mut h), Outcome::Converted { .. }));
    assert_eq!(f.text, "Я сейчас дома", "third press toggles again");
}

#[test]
fn toggle_needs_the_same_field_and_the_exact_text() {
    // Another field: the history does not apply (normal decision instead).
    let mut h = History::default();
    let mut f = field("ghbdtn", false);
    let _ = press(&mut f, &mut h);
    assert_eq!(f.text, "привет");
    let mut other = field("привет", false);
    other.key = Some(8);
    let _ = press(&mut other, &mut h);
    assert_eq!(other.text, "привет", "no undo in another field");
    assert!(!h.is_held(), "every press consumes the entry");

    // Text typed after the conversion: no undo.
    let mut h = History::default();
    let mut f = field("ghbdtn", false);
    let _ = press(&mut f, &mut h);
    f.text.push_str(" vbh");
    let _ = press(&mut f, &mut h);
    assert_eq!(f.text, "привет мир");

    // A platform without a field identity never toggles.
    let mut h = History::default();
    let mut f = field("ghbdtn", false);
    f.key = None;
    let _ = press(&mut f, &mut h);
    assert!(!h.is_held());
}

#[test]
fn history_debug_shows_no_text() {
    let mut h = History::default();
    let mut f = field("ghbdtn", false);
    let _ = press(&mut f, &mut h);
    let d = format!("{h:?}");
    assert!(!d.contains("ghbdtn") && !d.contains("привет"), "{d}");
}
