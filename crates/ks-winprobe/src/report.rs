//! One-line JSON reports, prefixed with `KS-PROBE `, printed to stdout.
//!
//! Only metadata may be recorded: lengths, counts, ids, timings, booleans and fixed
//! labels chosen by the probe. Never pass text read from a field or the clipboard.

use std::fmt::Write as _;

/// Outcome of one probe scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Behaviour matched the expectation.
    Pass,
    /// Behaviour contradicted the expectation (the test also panics).
    Fail,
    /// Could not be evaluated on this machine; `reason` says why. Not a pass.
    Skip,
    /// A fact was recorded; there is no pass/fail expectation (e.g. XR-03 RTF behaviour).
    Observed,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
            Status::Observed => "OBSERVED",
        }
    }
}

/// Builder for one report line.
#[derive(Debug)]
pub struct Report {
    id: &'static str,
    fields: Vec<(&'static str, String)>,
}

impl Report {
    /// Starts a report for scenario `id` (e.g. `"a.uia.focused"`).
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            fields: Vec::new(),
        }
    }

    /// Adds a metadata field. `value` must not be user or field text.
    pub fn field(&mut self, key: &'static str, value: impl ToString) -> &mut Self {
        self.fields.push((key, value.to_string()));
        self
    }

    /// Prints the line and returns the status (so callers can assert on it).
    pub fn emit(&self, status: Status, reason: &str) -> Status {
        let mut line = String::new();
        let _ = write!(
            line,
            "{{\"test\":\"{}\",\"status\":\"{}\"",
            escape(self.id),
            status.label()
        );
        if !reason.is_empty() {
            let _ = write!(line, ",\"reason\":\"{}\"", escape(reason));
        }
        for (k, v) in &self.fields {
            let _ = write!(line, ",\"{}\":\"{}\"", escape(k), escape(v));
        }
        line.push('}');
        print_line(&line);
        status
    }
}

/// Reports SKIP because the probe was not enabled via `KS_WINPROBE=1`.
pub fn skip_disabled(id: &'static str) {
    Report::new(id).emit(
        Status::Skip,
        "probe disabled: set KS_WINPROBE=1 and run with --test-threads=1",
    );
}

#[allow(clippy::print_stdout)] // The probe's deliverable is this metadata-only stdout line.
fn print_line(line: &str) {
    println!("KS-PROBE {line}");
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escapes_json_specials() {
        assert_eq!(escape("a\"b\\c\n"), "a\\\"b\\\\c\\u000a");
    }
}
