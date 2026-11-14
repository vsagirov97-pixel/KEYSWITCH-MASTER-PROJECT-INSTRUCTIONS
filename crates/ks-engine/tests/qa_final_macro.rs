//! QA final (Windows product): independent checks of the text macro slot (D-18) against
//! the conversion pipeline. Synthetic text only.
//!
//! * every refusal raised by the target gate is reported identically by `run` (conversion)
//!   and `run_macro` (macro), and neither touches the field;
//! * a text macro never reads the field, never selects by itself and never switches the
//!   keyboard layout;
//! * control and invisible line-break characters are refused even with the Enter/Tab
//!   permission; `\n` / `\t` only with it.

use ks_engine::{
    Config, Macro, MacroBody, MacroPermissions, NoCodeRunner, Outcome, Platform, Refusal, Stop,
    TargetInfo, UserText, run, run_macro,
};

#[derive(Default)]
struct Spy {
    refuse: Option<Refusal>,
    text: String,
    reads: usize,
    selects: usize,
    switches: usize,
    writes: usize,
}

impl Platform for Spy {
    fn begin(&mut self) -> Result<TargetInfo, Stop> {
        match self.refuse {
            Some(r) => Err(Stop::Refused(r)),
            None => Ok(TargetInfo {
                exe: "notepad.exe".into(),
            }),
        }
    }
    fn read_selection(&mut self, _max_units: usize) -> Result<Option<UserText>, Stop> {
        self.reads += 1;
        Ok(None)
    }
    fn read_before_caret(&mut self, _max_units: usize) -> Result<Option<UserText>, Stop> {
        self.reads += 1;
        Ok(Some(UserText::new(self.text.clone())))
    }
    fn select_before_caret(&mut self, _units: usize) -> Result<(), Stop> {
        self.selects += 1;
        Ok(())
    }
    fn write(&mut self, text: &UserText) -> Result<(), Stop> {
        self.writes += 1;
        self.text.push_str(text.expose());
        Ok(())
    }
    fn verify(&mut self, expected: &UserText) -> bool {
        self.text.ends_with(expected.expose())
    }
    fn switch_layout(&mut self, _lang: &str) -> bool {
        self.switches += 1;
        true
    }
}

fn text_macro(text: &str, allow_enter_tab: bool) -> Macro {
    Macro {
        body: MacroBody::Text(UserText::new(text.to_owned())),
        permissions: MacroPermissions { allow_enter_tab },
    }
}

const GATE_REFUSALS: [Refusal; 8] = [
    Refusal::Password,
    Refusal::Elevated,
    Refusal::SecureDesktop,
    Refusal::SelfTarget,
    Refusal::Console,
    Refusal::ExcludedApp,
    Refusal::ReadOnly,
    Refusal::Unsupported,
];

#[test]
fn gate_refusals_are_identical_for_conversion_and_macro() {
    let c = ks_core::builtin().expect("built-in data");
    for r in GATE_REFUSALS {
        let mut conv = Spy {
            refuse: Some(r),
            text: "ghbdtn".into(),
            ..Spy::default()
        };
        let a = run(&mut conv, &c, &Config::default());
        let mut mac = Spy {
            refuse: Some(r),
            text: "ghbdtn".into(),
            ..Spy::default()
        };
        let b = run_macro(
            &mut mac,
            &text_macro("hi", false),
            &mut NoCodeRunner,
            &Config::default(),
        );
        assert_eq!(a, b, "{r:?}");
        assert_eq!(a, Outcome::Refused(r));
        for s in [&conv, &mac] {
            assert_eq!((s.reads, s.selects, s.writes, s.switches), (0, 0, 0, 0));
            assert_eq!(s.text, "ghbdtn");
        }
    }
}

#[test]
fn text_macro_never_reads_selects_or_switches_layout() {
    for (text, allow) in [
        ("ghbdtn", false),
        ("Добрый день,", false),
        ("a\nb\tc", true),
    ] {
        let mut s = Spy {
            text: "before ".into(),
            ..Spy::default()
        };
        let out = run_macro(
            &mut s,
            &text_macro(text, allow),
            &mut NoCodeRunner,
            &Config::default(),
        );
        assert_eq!(out, Outcome::Inserted { verified: true });
        // Inserted as is: no conversion of a macro that looks like a wrong layout.
        assert_eq!(s.text, format!("before {text}"));
        assert_eq!((s.reads, s.selects, s.switches, s.writes), (0, 0, 0, 1));
    }
}

#[test]
fn control_and_hidden_line_breaks_are_refused_even_with_permission() {
    for bad in [
        "\r\n",
        "a\u{0B}b",
        "a\u{0C}b",
        "a\u{1F}b",
        "a\u{7F}b",
        "a\u{85}b",
        "a\u{2028}b",
        "\u{200E}x",
        "\u{2067}x",
        "\u{061C}x",
    ] {
        for allow in [false, true] {
            let mut s = Spy::default();
            let out = run_macro(
                &mut s,
                &text_macro(bad, allow),
                &mut NoCodeRunner,
                &Config::default(),
            );
            assert_eq!(out, Outcome::Refused(Refusal::ForbiddenChars), "{bad:?}");
            assert_eq!(s.writes, 0);
        }
    }
    for ok in ["\n", "\t", "x\n\ny", "\u{200D}", "\u{FEFF}"] {
        let mut s = Spy::default();
        let out = run_macro(
            &mut s,
            &text_macro(ok, false),
            &mut NoCodeRunner,
            &Config::default(),
        );
        let expect_refused = ok.contains(['\n', '\t']);
        assert_eq!(
            out == Outcome::Refused(Refusal::ForbiddenChars),
            expect_refused,
            "{ok:?}"
        );
    }
}
