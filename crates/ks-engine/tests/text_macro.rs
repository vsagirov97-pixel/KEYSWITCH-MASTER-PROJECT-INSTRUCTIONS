//! Macro Studio (Beta 2 step 1) through the public API of `ks-engine`: text macros and the
//! code-macro hook. Synthetic text only. The fake field records every platform call so the
//! tests can prove what was (not) touched.

use ks_engine::{
    CodeLanguage, Config, Failure, MACRO_MAX_CHARS, Macro, MacroAction, MacroBody, MacroInput,
    MacroPermissions, MacroRunner, NoCodeRunner, Outcome, Platform, Refusal, Stop, TargetInfo,
    UserText, run_macro,
};

struct Field {
    text: Vec<u16>,
    sel: (usize, usize),
    exe: &'static str,
    refuse: Option<Refusal>,
    write_error: Option<Stop>,
    begins: usize,
    reads: usize,
    writes: usize,
    layout_switches: usize,
}

impl Field {
    fn new(text: &str, sel: (usize, usize)) -> Self {
        Self {
            text: text.encode_utf16().collect(),
            sel,
            exe: "notepad.exe",
            refuse: None,
            write_error: None,
            begins: 0,
            reads: 0,
            writes: 0,
            layout_switches: 0,
        }
    }
    fn caret_at_end(text: &str) -> Self {
        let n = text.encode_utf16().count();
        Self::new(text, (n, n))
    }
    fn content(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }
}

impl Platform for Field {
    fn begin(&mut self) -> Result<TargetInfo, Stop> {
        self.begins += 1;
        match self.refuse {
            Some(r) => Err(Stop::Refused(r)),
            None => Ok(TargetInfo {
                exe: self.exe.into(),
            }),
        }
    }
    fn read_selection(&mut self, _max_units: usize) -> Result<Option<UserText>, Stop> {
        self.reads += 1;
        let (a, b) = self.sel;
        Ok((a < b).then(|| UserText::new(String::from_utf16_lossy(&self.text[a..b]))))
    }
    fn read_before_caret(&mut self, _max_units: usize) -> Result<Option<UserText>, Stop> {
        self.reads += 1;
        Ok(None)
    }
    fn select_before_caret(&mut self, _units: usize) -> Result<(), Stop> {
        Err(Stop::Failed(Failure::Write))
    }
    fn write(&mut self, text: &UserText) -> Result<(), Stop> {
        if let Some(e) = self.write_error {
            return Err(e);
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
        let want: Vec<u16> = expected.expose().encode_utf16().collect();
        end >= want.len() && self.text[end - want.len()..end] == want[..]
    }
    fn switch_layout(&mut self, _lang: &str) -> bool {
        self.layout_switches += 1;
        true
    }
}

fn text_macro(text: &str, allow_enter_tab: bool) -> Macro {
    Macro {
        body: MacroBody::Text(UserText::new(text.into())),
        permissions: MacroPermissions { allow_enter_tab },
    }
}

fn go(f: &mut Field, m: &Macro) -> Outcome {
    run_macro(f, m, &mut NoCodeRunner, &Config::default())
}

const INSERTED: Outcome = Outcome::Inserted { verified: true };

#[test]
fn text_is_inserted_at_the_caret_without_layout_switch() {
    let mut f = Field::caret_at_end("Dear ");
    assert_eq!(go(&mut f, &text_macro("colleagues,", false)), INSERTED);
    assert_eq!(f.content(), "Dear colleagues,");
    assert_eq!((f.writes, f.reads, f.layout_switches), (1, 0, 0));
}

#[test]
fn text_replaces_the_selection() {
    let mut f = Field::new("one two three", (4, 7));
    assert_eq!(go(&mut f, &text_macro("2", false)), INSERTED);
    assert_eq!(f.content(), "one 2 three");
    // A text macro never reads the target.
    assert_eq!(f.reads, 0);
}

#[test]
fn enter_and_tab_need_the_slot_permission() {
    for text in ["line one\nline two", "a\tb"] {
        let mut f = Field::caret_at_end("x");
        assert_eq!(
            go(&mut f, &text_macro(text, false)),
            Outcome::Refused(Refusal::ForbiddenChars)
        );
        // Refused before the target is looked at.
        assert_eq!((f.begins, f.writes), (0, 0));
        assert_eq!(f.content(), "x");
        let mut f = Field::caret_at_end("x");
        assert_eq!(go(&mut f, &text_macro(text, true)), INSERTED);
        assert_eq!(f.content(), format!("x{text}"));
    }
}

#[test]
fn control_characters_are_refused_even_with_the_permission() {
    for text in ["a\rb", "\u{1b}[31m", "a\u{2029}b", "\u{202E}abc", "a\u{0}"] {
        let mut f = Field::caret_at_end("");
        assert_eq!(
            go(&mut f, &text_macro(text, true)),
            Outcome::Refused(Refusal::ForbiddenChars)
        );
        assert_eq!((f.begins, f.writes), (0, 0));
    }
}

#[test]
fn length_limit_is_5000_characters() {
    let mut f = Field::caret_at_end("");
    let max = "й".repeat(MACRO_MAX_CHARS);
    assert_eq!(go(&mut f, &text_macro(&max, false)), INSERTED);
    let mut f = Field::caret_at_end("");
    let over = "й".repeat(MACRO_MAX_CHARS + 1);
    assert_eq!(
        go(&mut f, &text_macro(&over, false)),
        Outcome::Refused(Refusal::TooLong)
    );
    assert_eq!(f.writes, 0);
}

#[test]
fn empty_macro_does_nothing() {
    let mut f = Field::caret_at_end("abc");
    assert_eq!(go(&mut f, &text_macro("", false)), Outcome::Nothing);
    assert_eq!(f.writes, 0);
}

#[test]
fn os_refusals_apply_like_for_conversion() {
    for r in [
        Refusal::Password,
        Refusal::Elevated,
        Refusal::SecureDesktop,
        Refusal::SelfTarget,
        Refusal::Console,
        Refusal::ReadOnly,
        Refusal::Unsupported,
    ] {
        let mut f = Field::caret_at_end("abc");
        f.refuse = Some(r);
        assert_eq!(go(&mut f, &text_macro("hi", false)), Outcome::Refused(r));
        assert_eq!((f.writes, f.reads), (0, 0));
        assert_eq!(f.content(), "abc");
    }
}

#[test]
fn excluded_and_unknown_apps_are_refused() {
    let config = Config {
        excluded: vec!["keepass.exe".into()],
        ..Config::default()
    };
    let mut f = Field::caret_at_end("");
    f.exe = "keepass.exe";
    let m = text_macro("hi", false);
    assert_eq!(
        run_macro(&mut f, &m, &mut NoCodeRunner, &config),
        Outcome::Refused(Refusal::ExcludedApp)
    );
    f.exe = "";
    assert_eq!(
        run_macro(&mut f, &m, &mut NoCodeRunner, &config),
        Outcome::Refused(Refusal::Unsupported)
    );
    assert_eq!(f.writes, 0);
}

#[test]
fn write_refusal_of_the_method_is_passed_through() {
    // E.g. W3 refuses `\n` even when the slot allows it; nothing is changed.
    let mut f = Field::caret_at_end("x");
    f.write_error = Some(Stop::Refused(Refusal::ForbiddenChars));
    assert_eq!(
        go(&mut f, &text_macro("a\nb", true)),
        Outcome::Refused(Refusal::ForbiddenChars)
    );
    assert_eq!(f.content(), "x");
}

fn code_macro() -> Macro {
    Macro {
        body: MacroBody::Code {
            language: CodeLanguage::JavaScript,
            source: UserText::new("ks.insert('x')".into()),
        },
        permissions: MacroPermissions::default(),
    }
}

#[test]
fn code_macros_are_not_available_yet() {
    let mut f = Field::caret_at_end("abc");
    assert_eq!(
        go(&mut f, &code_macro()),
        Outcome::Refused(Refusal::NotAvailable)
    );
    assert_eq!((f.writes, f.reads), (0, 0));
    assert_eq!(
        Outcome::Refused(Refusal::NotAvailable).code(),
        "NOT_AVAILABLE"
    );
}

/// A runner as the sandbox will plug in: its result goes through the content policy.
struct Echo {
    result: &'static str,
    wants_selection: bool,
    saw_selection: Option<bool>,
}

impl MacroRunner for Echo {
    fn wants_selection(&self) -> bool {
        self.wants_selection
    }
    fn run(
        &mut self,
        _language: CodeLanguage,
        _source: &UserText,
        input: &MacroInput<'_>,
    ) -> Result<MacroAction, Refusal> {
        self.saw_selection = Some(input.selection.is_some());
        Ok(MacroAction::Insert(UserText::new(self.result.into())))
    }
}

#[test]
fn runner_result_is_policed_and_selection_is_read_only_on_request() {
    let mut f = Field::new("abc", (0, 3));
    let mut r = Echo {
        result: "ok",
        wants_selection: false,
        saw_selection: None,
    };
    let c = Config::default();
    assert_eq!(run_macro(&mut f, &code_macro(), &mut r, &c), INSERTED);
    assert_eq!((f.reads, r.saw_selection), (0, Some(false)));
    assert_eq!(f.content(), "ok");

    let mut f = Field::new("abc", (0, 3));
    r.wants_selection = true;
    assert_eq!(run_macro(&mut f, &code_macro(), &mut r, &c), INSERTED);
    assert_eq!((f.reads, r.saw_selection), (1, Some(true)));

    // A runner cannot smuggle in an Enter the slot does not permit.
    let mut f = Field::caret_at_end("");
    r.result = "send\n";
    assert_eq!(
        run_macro(&mut f, &code_macro(), &mut r, &c),
        Outcome::Refused(Refusal::ForbiddenChars)
    );
    assert_eq!(f.writes, 0);
}
