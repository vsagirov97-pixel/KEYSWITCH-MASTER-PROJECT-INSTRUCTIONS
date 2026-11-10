//! Minimal JSON-lines reporting without extra dependencies.
//!
//! Values are restricted to numbers, booleans and short ASCII metadata (AX roles, input
//! source IDs, OS versions, error codes). Field text never reaches this module.

use std::fmt::Write as _;

/// Outcome of a scenario.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Pass criterion met.
    Pass,
    /// Pass criterion not met (or a safety invariant was violated).
    Fail,
    /// Not executed; the reason is mandatory.
    Skip,
    /// Observation recorded; the scenario has no binary criterion.
    Fact,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
            Status::Fact => "FACT",
        }
    }
}

/// One scenario result: id, status and a flat set of key/value details.
pub struct Outcome {
    id: &'static str,
    status: Status,
    fields: Vec<(&'static str, String)>,
}

impl Outcome {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            status: Status::Pass,
            fields: Vec::new(),
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn set_status(&mut self, status: Status) -> &mut Self {
        self.status = status;
        self
    }

    /// Marks the outcome failed (never downgrades back to pass).
    pub fn fail(&mut self, reason: &str) -> &mut Self {
        self.status = Status::Fail;
        self.meta("fail_reason", reason)
    }

    /// Marks the outcome skipped with a reason, unless it already failed.
    pub fn skip(&mut self, reason: &str) -> &mut Self {
        if self.status != Status::Fail {
            self.status = Status::Skip;
        }
        self.meta("skip_reason", reason)
    }

    pub fn num<N: std::fmt::Display>(&mut self, key: &'static str, v: N) -> &mut Self {
        self.fields.push((key, v.to_string()));
        self
    }

    pub fn flag(&mut self, key: &'static str, v: bool) -> &mut Self {
        self.fields.push((key, v.to_string()));
        self
    }

    /// Short metadata string (role, source ID, version). Never field text.
    pub fn meta(&mut self, key: &'static str, v: &str) -> &mut Self {
        self.fields.push((key, json_string(v)));
        self
    }

    /// Serialises to one JSON object.
    pub fn to_json(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "{{\"id\":\"{}\",\"status\":\"{}\"",
            self.id,
            self.status.as_str()
        );
        for (k, v) in &self.fields {
            let _ = write!(s, ",\"{k}\":{v}");
        }
        s.push('}');
        s
    }
}

fn json_string(v: &str) -> String {
    let mut s = String::with_capacity(v.len() + 2);
    s.push('"');
    for c in v.chars().take(120) {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            c if c.is_ascii() && !c.is_ascii_control() => s.push(c),
            _ => s.push('?'),
        }
    }
    s.push('"');
    s
}
