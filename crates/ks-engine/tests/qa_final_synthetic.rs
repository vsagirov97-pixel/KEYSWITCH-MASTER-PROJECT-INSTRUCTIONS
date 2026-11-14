//! QA final (Windows product, feat/win-beta2): independent synthetic cases written by QA
//! and NOT shown to the developer before the run. They go through the real pipeline
//! `ks_engine::run` with two fake fields: caret mode (no selection, the engine selects the
//! last word itself) and selection mode (D-31). Expected values are written by hand from
//! the Windows layouts US / Russian (KLID 00000419). Invented text only.
//!
//! Every case is classified:
//! * `PASS`    - the field holds the expected text;
//! * `REFUSED` - the field is unchanged although a fix was expected (harmless: nothing was
//!   corrupted; the language popup is "unavailable" in this fake, so a popup also lands here);
//! * `WRONG`   - the field was changed to something other than the expected text;
//! * `FALSE`   - text that had to stay unchanged was changed.
//!
//! WRONG and FALSE corrupt what the user typed: they are defects and fail the test unless
//! listed in `KNOWN` with a defect ID from `docs/qa/windows-product-qa-report.md`.
//! REFUSED is reported (run with `--nocapture`) and capped by `MAX_REFUSED`.

#![allow(
    clippy::print_stdout,
    reason = "the result table is the purpose of this test; the cases are synthetic"
)]

use ks_engine::{Config, Outcome, Platform, Stop, TargetInfo, UserText, run};

/// A single-line field with the caret at the end and an optional selection at its end.
struct Field {
    text: String,
    /// Length in bytes of the selected tail (`None` = caret only).
    selected: Option<usize>,
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
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Caret at the end of the field, nothing selected.
    Caret,
    /// The whole field is selected.
    Select,
}

fn press(mode: Mode, input: &str) -> (String, &'static str) {
    static CONVERTER: std::sync::OnceLock<ks_core::Converter> = std::sync::OnceLock::new();
    let c = CONVERTER.get_or_init(|| ks_core::builtin().expect("built-in data"));
    let mut f = Field {
        text: input.to_owned(),
        selected: (mode == Mode::Select).then_some(input.len()),
    };
    let outcome = run(&mut f, c, &Config::default());
    if !matches!(outcome, Outcome::Converted { .. }) {
        assert_eq!(f.text, input, "field changed without a Converted outcome");
    }
    (f.text, outcome.code())
}

use Mode::{Caret, Select};

/// (category, mode, field content before the hotkey, expected field content after it)
const CASES: &[(&str, Mode, &str, &str)] = &[
    // --- short words as the last word, after correct context (caret) ---
    ("short_ctx", Caret, "Я сейчас ljvf", "Я сейчас дома"),
    ("short_ctx", Caret, "Ты где? z", "Ты где? я"),
    ("short_ctx", Caret, "Это vs", "Это мы"),
    ("short_ctx", Caret, "кто jy", "кто он"),
    ("short_ctx", Caret, "Ну yt", "Ну не"),
    ("short_ctx", Caret, "Скажи lf", "Скажи да"),
    ("short_ctx", Caret, "Я pf", "Я за"),
    ("short_ctx", Caret, "Привет, rfr", "Привет, как"),
    ("short_ctx", Caret, "where шы", "where is"),
    ("short_ctx", Caret, "let us go ещ", "let us go to"),
    ("short_ctx", Caret, "I love ьн", "I love my"),
    ("short_keep", Caret, "go to", "go to"),
    ("short_keep", Caret, "I am at", "I am at"),
    ("short_keep", Caret, "where is it", "where is it"),
    ("short_keep", Caret, "Ну и", "Ну и"),
    ("short_keep", Caret, "он не", "он не"),
    ("short_keep", Caret, "мы в", "мы в"),
    ("short_keep", Caret, "Ок ", "Ок "),
    // --- punctuation at the end of a sentence (caret) ---
    ("punct_end", Caret, "Всё gjyznyj.", "Всё понятно."),
    ("punct_end", Caret, "Очень rhfcbdj.", "Очень красиво."),
    ("punct_end", Caret, "Ты ujnjd?", "Ты готов?"),
    ("punct_end", Caret, "Это ghfdlf!", "Это правда!"),
    ("punct_end", Caret, "J,zpfntkmyj!", "Обязательно!"),
    ("punct_end", Caret, "Rjulf?", "Когда?"),
    ("punct_end", Caret, "Ну ctujlyz.", "Ну сегодня."),
    ("punct_end", Caret, "Ghbdtn)", "Привет)"),
    ("punct_end", Caret, "Как ltkf&", "Как дела?"),
    ("punct_end", Caret, "Cgfcb,j...", "Спасибо..."),
    ("punct_end", Caret, "I am ыгкую", "I am sure."),
    ("punct_end", Caret, "Where are нщг,", "Where are you?"),
    ("punct_end", Caret, "I said нуыю", "I said yes."),
    ("punct_end", Caret, "Ok, ерфтлы!", "Ok, thanks!"),
    // --- names and brands (caret) ---
    ("names", Caret, "Vjcrdf", "Москва"),
    ("names", Caret, "Я из Gtnth,ehuf", "Я из Петербурга"),
    ("names", Caret, "Bdfy", "Иван"),
    ("names", Caret, "Ntktuhfv", "Телеграм"),
    ("names", Caret, "Пщщпду", "Google"),
    ("names", Caret, "открой Цштвщцы", "открой Windows"),
    ("names", Caret, "Ьшскщыщае", "Microsoft"),
    ("names", Caret, "Ыфьыгтп", "Samsung"),
    ("names_keep", Caret, "GitHub", "GitHub"),
    ("names_keep", Caret, "iPhone", "iPhone"),
    ("names_keep", Caret, "WhatsApp", "WhatsApp"),
    ("names_keep", Caret, "Excel", "Excel"),
    ("names_keep", Caret, "Ozon", "Ozon"),
    ("names_keep", Caret, "Wildberries", "Wildberries"),
    ("names_keep", Caret, "Яндекс", "Яндекс"),
    ("names_keep", Caret, "Сбер", "Сбер"),
    // --- Latin abbreviations inside Russian text ---
    (
        "lat_in_ru",
        Select,
        "Отправь PDF на почту",
        "Отправь PDF на почту",
    ),
    ("lat_in_ru", Select, "Нужен API ключ", "Нужен API ключ"),
    (
        "lat_in_ru",
        Select,
        "Звонил в IT отдел",
        "Звонил в IT отдел",
    ),
    (
        "lat_in_ru",
        Select,
        "Код ошибки HTTP 404",
        "Код ошибки HTTP 404",
    ),
    (
        "lat_in_ru",
        Select,
        "Открой Excel и Word",
        "Открой Excel и Word",
    ),
    (
        "lat_in_ru",
        Select,
        "Отправь PDF yf почту",
        "Отправь PDF на почту",
    ),
    (
        "lat_in_ru",
        Select,
        "Подключи USB rf,tkm",
        "Подключи USB кабель",
    ),
    (
        "lat_in_ru",
        Select,
        "Сделай git pull gjnjv",
        "Сделай git pull потом",
    ),
    (
        "lat_in_ru",
        Select,
        "Версия Windows 11 ujnjdf",
        "Версия Windows 11 готова",
    ),
    (
        "lat_in_ru",
        Select,
        "Купи iPhone xthtp ytltk.",
        "Купи iPhone через неделю",
    ),
    (
        "lat_in_ru",
        Select,
        "Скинь ссылку d Telegram",
        "Скинь ссылку в Telegram",
    ),
    ("lat_in_ru", Caret, "Скачай PDF", "Скачай PDF"),
    ("lat_in_ru", Caret, "Скачай ЗВА", "Скачай PDF"),
    // --- Cyrillic abbreviations inside English text ---
    (
        "cyr_in_en",
        Select,
        "Paid via СБП today",
        "Paid via СБП today",
    ),
    (
        "cyr_in_en",
        Select,
        "Contract with ООО Ромашка signed",
        "Contract with ООО Ромашка signed",
    ),
    (
        "cyr_in_en",
        Select,
        "He studied at МГУ last year",
        "He studied at МГУ last year",
    ),
    (
        "cyr_in_en",
        Select,
        "Report for ФНС is ready",
        "Report for ФНС is ready",
    ),
    ("cyr_in_en", Select, "The МГУ ыегвуте", "The МГУ student"),
    (
        "cyr_in_en",
        Select,
        "Send it to ЖКХ ещвфн",
        "Send it to ЖКХ today",
    ),
    (
        "cyr_in_en",
        Select,
        "Pay the НДС ещьщккщц",
        "Pay the НДС tomorrow",
    ),
    ("cyr_in_en", Caret, "Paid via СБП", "Paid via СБП"),
    // --- mixed-language sentences (selection) ---
    (
        "mixed",
        Select,
        "Привет, how are you?",
        "Привет, how are you?",
    ),
    ("mixed", Select, "Я купил new laptop", "Я купил new laptop"),
    (
        "mixed",
        Select,
        "Это deadline на завтра",
        "Это deadline на завтра",
    ),
    ("mixed", Select, "Let's go в кино", "Let's go в кино"),
    (
        "mixed",
        Select,
        "Мой email: test@mail.ru",
        "Мой email: test@mail.ru",
    ),
    ("mixed", Select, "hello ьн акшутв", "hello my friend"),
    ("mixed", Select, "Привет, rfr ltkf", "Привет, как дела"),
    (
        "mixed",
        Select,
        "Встреча в 10:30 pfdnhf",
        "Встреча в 10:30 завтра",
    ),
    ("mixed", Caret, "Ok, gjyznyj", "Ok, понятно"),
    ("mixed", Select, "Спасибо, see нщг", "Спасибо, see you"),
    ("abbr_caps", Select, "Send it to ЖКХ", "Send it to ЖКХ"),
    ("abbr_caps", Select, "Call ЖЭК ещвфн", "Call ЖЭК today"),
    ("abbr_caps", Select, "Ask ЖЭК", "Ask ЖЭК"),
    ("abbr_caps", Select, "My ЖЖ blog", "My ЖЖ blog"),
    ("abbr_caps", Select, "ФСБ and ЖКХ", "ФСБ and ЖКХ"),
    ("abbr_caps", Caret, "Отправь USB", "Отправь USB"),
    ("abbr_caps", Caret, "Нужен API", "Нужен API"),
    ("abbr_caps", Caret, "Звони в IT", "Звони в IT"),
    ("abbr_caps", Caret, "Это HTTP", "Это HTTP"),
    ("abbr_caps", Caret, "Код SQL", "Код SQL"),
    ("abbr_caps", Caret, "Paid via ФНС", "Paid via ФНС"),
    ("abbr_caps", Caret, "Call ЖКХ", "Call ЖКХ"),
    ("abbr_caps", Caret, "Это ПДФ", "Это ПДФ"),
    (
        "mixed",
        Select,
        "Отлично, ctujlyz в 5",
        "Отлично, сегодня в 5",
    ),
    ("mixed", Select, "Скажи ему: ltkfq", "Скажи ему: делай"),
    ("mixed", Select, "Я ljvf, а ты?", "Я дома, а ты?"),
];

/// WRONG/FALSE results that are known and tracked: (field content, defect ID).
const KNOWN: &[(&str, &str)] = &[
    // Mixed selection: an all-caps Cyrillic abbreviation in English text is re-typed into
    // punctuation/Latin garbage by the per-token decision.
    ("Send it to ЖКХ ещвфн", "QA-WF-01"),
    ("Send it to ЖКХ", "QA-WF-01"),
    ("Call ЖЭК ещвфн", "QA-WF-01"),
    ("Ask ЖЭК", "QA-WF-01"),
    ("ФСБ and ЖКХ", "QA-WF-01"),
    // Caret: a correct all-caps abbreviation outside the technical list is converted.
    ("Скачай PDF", "QA-WF-02"),
    ("Это HTTP", "QA-WF-02"),
    ("Paid via ФНС", "QA-WF-02"),
    ("Call ЖКХ", "QA-WF-02"),
    ("Это ПДФ", "QA-WF-02"),
];

/// Upper bound on harmless refusals (measured 5 on 6db8fe9, regression guard only).
const MAX_REFUSED: usize = 5;

#[test]
fn qa_final_synthetic_cases() {
    assert!(CASES.len() >= 40, "at least 40 QA cases");
    let (mut pass, mut refused) = (0, 0);
    let mut defects: Vec<String> = Vec::new();
    let mut report: Vec<String> = Vec::new();
    for &(cat, mode, input, expected) in CASES {
        let (actual, code) = press(mode, input);
        let kind = if actual == expected {
            pass += 1;
            "PASS"
        } else if actual == input {
            refused += 1;
            "REFUSED"
        } else if expected == input {
            "FALSE"
        } else {
            "WRONG"
        };
        if kind != "PASS" {
            let line = format!(
                "{kind:7} {cat:10} {m} in={input:?} expected={expected:?} actual={actual:?} code={code}",
                m = if mode == Caret { "caret " } else { "select" }
            );
            if matches!(kind, "WRONG" | "FALSE") && !KNOWN.iter().any(|(i, _)| *i == input) {
                defects.push(line.clone());
            }
            report.push(line);
        }
    }
    println!(
        "\nQA final synthetic: {} cases, {pass} pass, {refused} refused, {} wrong/false",
        CASES.len(),
        CASES.len() - pass - refused
    );
    for r in &report {
        println!("  {r}");
    }
    assert!(
        defects.is_empty(),
        "untracked WRONG/FALSE:\n{}",
        defects.join("\n")
    );
    assert!(refused <= MAX_REFUSED, "refusals grew: {refused}");
}
