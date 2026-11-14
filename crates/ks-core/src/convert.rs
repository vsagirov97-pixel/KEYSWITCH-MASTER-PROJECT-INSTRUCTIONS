//! Layout conversion with language-model disambiguation.
//!
//! Every character is re-mapped through its physical key. Non-letter characters that
//! map to something different (e.g. `,` -> `б`, `?` -> `,`) are *ambiguous*: the user may
//! have meant the mapped character or the literal one. All combinations are scored with
//! the target language model plus punctuation priors, and the cheapest wins.

use crate::layout::Layout;
use crate::model::LanguageModel;

/// At most this many ambiguous positions per token are enumerated (2^N candidates).
const MAX_AMBIGUOUS: usize = 10;
/// Tokens longer than this are not natural words: no enumeration, physical mapping only.
const MAX_ENUMERATED_TOKEN: usize = 48;
/// Mixed text: a token is converted only if conversion is better by more than this margin
/// (nats), kept if worse by more than it, otherwise the result is ambiguous.
const MIXED_MARGIN: f32 = 2.0;
/// Extra cost of keeping word-final punctuation when its key's letter would complete a
/// dictionary word (`'nf;` -> `этаж`, not `эта;`; QA-B1-03). Small enough that a much more
/// frequent word before the punctuation still keeps it.
const LETTER_WORD_PENALTY: f32 = 1.5;

/// A language: its keyboard layout and its language model.
#[derive(Debug, Clone)]
pub struct Language {
    pub id: String,
    pub layout: Layout,
    pub model: LanguageModel,
    /// Characters allowed between letters inside a word at low cost (e.g. `'` in English).
    pub joiners: Vec<char>,
}

/// Which languages the letters of a text belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// No letters of any known language.
    NoLetters,
    /// All letters belong to one language.
    Single(String),
    /// Letters of several languages: the user must choose the target.
    Mixed(Vec<String>),
}

/// How a text is re-typed (D-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConvertMode {
    /// Caret mode (last word): language-model choice for ambiguous punctuation, technical
    /// tokens kept, number separators only next to converted words, `Low` confidence when
    /// the original looks more plausible.
    #[default]
    Smart,
    /// Selection mode: every character key-for-key through the layouts, no refusals. The
    /// only exception: `.` `,` `:` between two digits stay (`v1.2.3` -> `м1.2.3`), because
    /// a number cannot contain a letter.
    Literal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    /// The engine must not replace text without the user's confirmation.
    Low,
}

/// Result of a conversion. `text` is user data: never log it.
#[derive(Clone, PartialEq)]
pub struct Conversion {
    pub text: String,
    pub confidence: Confidence,
    pub converted_tokens: usize,
    pub kept_tokens: usize,
}

impl std::fmt::Debug for Conversion {
    // Deliberately omits `text`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conversion")
            .field("text_len", &self.text.chars().count())
            .field("confidence", &self.confidence)
            .field("converted_tokens", &self.converted_tokens)
            .field("kept_tokens", &self.kept_tokens)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    UnknownLanguage,
    SameLanguage,
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownLanguage => "unknown language",
            Self::SameLanguage => "source and target language are the same",
        })
    }
}

impl std::error::Error for ConvertError {}

/// Converter over a set of languages.
#[derive(Debug, Clone)]
pub struct Converter {
    langs: Vec<Language>,
}

impl Converter {
    pub fn new(langs: Vec<Language>) -> Self {
        Self { langs }
    }

    pub fn languages(&self) -> impl Iterator<Item = &str> {
        self.langs.iter().map(|l| l.id.as_str())
    }

    fn lang(&self, id: &str) -> Result<usize, ConvertError> {
        self.langs
            .iter()
            .position(|l| l.id == id)
            .ok_or(ConvertError::UnknownLanguage)
    }

    /// Detects which languages' letters are present.
    pub fn detect(&self, text: &str) -> Detection {
        let mut present: Vec<usize> = Vec::new();
        for ch in text.chars().filter(|c| c.is_alphabetic()) {
            for (i, l) in self.langs.iter().enumerate() {
                if l.model.is_letter(ch) && !present.contains(&i) {
                    present.push(i);
                }
            }
        }
        present.sort_unstable();
        match present.as_slice() {
            [] => Detection::NoLetters,
            [one] => Detection::Single(self.langs[*one].id.clone()),
            many => Detection::Mixed(many.iter().map(|&i| self.langs[i].id.clone()).collect()),
        }
    }

    /// Converts the whole text as typed in `from`'s layout into `to` ([`ConvertMode::Smart`]).
    /// Confidence is `Low` when the original looks more plausible than the result.
    pub fn convert(&self, text: &str, from: &str, to: &str) -> Result<Conversion, ConvertError> {
        self.convert_with(text, from, to, ConvertMode::Smart)
    }

    /// As [`Converter::convert`] in the given mode. [`ConvertMode::Literal`] is `High` as
    /// soon as the text has a letter of `from`.
    pub fn convert_with(
        &self,
        text: &str,
        from: &str,
        to: &str,
        mode: ConvertMode,
    ) -> Result<Conversion, ConvertError> {
        let (f, t) = (self.lang(from)?, self.lang(to)?);
        if f == t {
            return Err(ConvertError::SameLanguage);
        }
        let (src, tgt) = (&self.langs[f], &self.langs[t]);
        if mode == ConvertMode::Literal {
            // Every token, letterless ones too (`3/14` -> `3.14` next to Russian words).
            let mut out = literal(text, |tok| Some(literal_token(tok, src, tgt)));
            if !text.chars().any(|c| src.model.is_letter(c)) {
                out.confidence = Confidence::Low;
            }
            return Ok(out);
        }
        let mut out = Conversion {
            text: String::with_capacity(text.len()),
            confidence: Confidence::High,
            converted_tokens: 0,
            kept_tokens: 0,
        };
        let mut margin_sum = 0.0;
        let mut any_letters = false;
        let (mut kept_own, mut all_confident) = (false, true);
        let mut kept_abbreviation = false;
        let pieces = split_tokens(text);
        let sentence = is_sentence(&pieces);
        // Number tokens whose separators the target layout reads differently (`3/14`):
        // byte offset in `out.text` and the re-typed token, applied only if words convert.
        let mut numbers: Vec<(usize, String)> = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Space(s) => out.text.push_str(s),
                Piece::Token { text: tok, last } => {
                    if !tok.chars().any(|c| src.model.is_letter(c)) {
                        if let Some(n) = number_separators(tok, src, tgt, last) {
                            numbers.push((out.text.len(), n));
                        }
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                        continue;
                    }
                    match caps_rule(tok, src, tgt) {
                        Some(CapsRule::Keep) => {
                            // A correct all-caps abbreviation (`PDF`, `ЖКХ`; QA-WF-02).
                            out.text.push_str(tok);
                            out.kept_tokens += 1;
                            kept_abbreviation = true;
                            continue;
                        }
                        Some(CapsRule::Abbreviation(a)) => {
                            // `ЗВА` -> `PDF`: a listed abbreviation of the target.
                            out.text.push_str(&a);
                            out.converted_tokens += 1;
                            any_letters = true;
                            continue;
                        }
                        Some(CapsRule::Word) | None => {}
                    }
                    let (best, best_cost) = best_candidate(tok, src, tgt, last, last && sentence);
                    if looks_technical(tok, &best) {
                        // Identifier, version or hex literal (`v1.2.3`, `0xFF`, `x86_64`):
                        // never rewritten, and it does not count as convertible text.
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                        continue;
                    }
                    let margin = token_cost(tok, src, last) - best_cost;
                    if sentence && keeps_own_language(tok, &best, src, tgt, margin) {
                        // A genuine word of the typed language among wrong-layout words
                        // (`cgfcb,j, John` -> `спасибо, John`): left as typed.
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                        kept_own = true;
                        continue;
                    }
                    if !(margin > MIXED_MARGIN && words_known(&best, tgt) && !words_known(tok, src))
                    {
                        all_confident = false;
                    }
                    any_letters = true;
                    margin_sum += margin;
                    out.text.push_str(&best);
                    out.converted_tokens += 1;
                }
            }
        }
        // Keeping some words is a per-token decision: every converted word must then be
        // confidently in the wrong layout on its own (`npm install` stays).
        // Only correct abbreviations (and letterless tokens): the text is right as it is,
        // `High` with the text unchanged (nothing to do, no language question).
        let only_abbreviations = !any_letters && kept_abbreviation;
        if (!any_letters && !only_abbreviations) || margin_sum < 0.0 || (kept_own && !all_confident)
        {
            out.confidence = Confidence::Low;
        }
        if out.converted_tokens > 0 {
            for (at, n) in numbers {
                // Same length in bytes: only ASCII separators are swapped for ASCII ones.
                out.text.replace_range(at..at + n.len(), &n);
                out.converted_tokens += 1;
                out.kept_tokens -= 1;
            }
        }
        Ok(out)
    }

    /// Mixed text: converts tokens not written in `to` into `to`, keeping tokens that are
    /// already `to` or look like genuine words of their own language ([`ConvertMode::Smart`]).
    pub fn convert_mixed(&self, text: &str, to: &str) -> Result<Conversion, ConvertError> {
        self.convert_mixed_with(text, to, ConvertMode::Smart)
    }

    /// As [`Converter::convert_mixed`] in the given mode. [`ConvertMode::Literal`] re-types
    /// every token whose letters are mostly of another language, key for key.
    pub fn convert_mixed_with(
        &self,
        text: &str,
        to: &str,
        mode: ConvertMode,
    ) -> Result<Conversion, ConvertError> {
        let t = self.lang(to)?;
        let tgt = &self.langs[t];
        if mode == ConvertMode::Literal {
            return Ok(literal(text, |tok| {
                let s = self.dominant_foreign(tok, t)?;
                let src = &self.langs[s];
                match caps_rule(tok, src, tgt) {
                    // Mixed text is decided per token: a correct abbreviation stays.
                    Some(CapsRule::Keep) => None,
                    Some(CapsRule::Abbreviation(a)) => Some(a),
                    Some(CapsRule::Word) | None => Some(literal_token(tok, src, tgt)),
                }
            }));
        }
        let mut out = Conversion {
            text: String::with_capacity(text.len()),
            confidence: Confidence::High,
            converted_tokens: 0,
            kept_tokens: 0,
        };
        let pieces = split_tokens(text);
        let sentence = is_sentence(&pieces);
        for piece in pieces {
            match piece {
                Piece::Space(s) => out.text.push_str(s),
                Piece::Token { text: tok, last } => {
                    let Some(s) = self.dominant_foreign(tok, t) else {
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                        continue;
                    };
                    let src = &self.langs[s];
                    match caps_rule(tok, src, tgt) {
                        Some(CapsRule::Keep) => {
                            out.text.push_str(tok);
                            out.kept_tokens += 1;
                            continue;
                        }
                        Some(CapsRule::Abbreviation(a)) => {
                            out.text.push_str(&a);
                            out.converted_tokens += 1;
                            continue;
                        }
                        Some(CapsRule::Word) | None => {}
                    }
                    let (best, best_cost) = best_candidate(tok, src, tgt, last, last && sentence);
                    let margin = token_cost(tok, src, last) - best_cost;
                    if looks_technical(tok, &best) {
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                    } else if margin > MIXED_MARGIN {
                        out.text.push_str(&best);
                        out.converted_tokens += 1;
                    } else {
                        if margin >= -MIXED_MARGIN {
                            out.confidence = Confidence::Low;
                        }
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                    }
                }
            }
        }
        Ok(out)
    }

    /// Mixed text without a chosen target: each token is decided on its own. A token is
    /// converted only when it is confidently typed in the wrong layout (its re-typing is made
    /// of dictionary words, the original is not, and the re-typing wins by more than
    /// [`MIXED_MARGIN`]); tokens that are correct, technical (URL, e-mail, identifier,
    /// version) or letterless stay. `High` only if at least one token was converted and no
    /// token was ambiguous; otherwise `Low` with the text unchanged, and the caller asks the
    /// user for the language.
    ///
    /// [`ConvertMode::Literal`] (selection, D-31) decides the same way but accepts a token
    /// only if the language-model choice equals its key-for-key re-typing; any difference
    /// makes the text ambiguous.
    pub fn convert_mixed_auto(&self, text: &str, mode: ConvertMode) -> Conversion {
        let mut out = Conversion {
            text: String::with_capacity(text.len()),
            confidence: Confidence::High,
            converted_tokens: 0,
            kept_tokens: 0,
        };
        let mut ambiguous = false;
        let pieces = split_tokens(text);
        let sentence = is_sentence(&pieces);
        for piece in pieces {
            match piece {
                Piece::Space(s) => out.text.push_str(s),
                Piece::Token { text: tok, last } => {
                    match self.token_decision(tok, last, last && sentence, mode) {
                        TokenDecision::Keep => {
                            out.text.push_str(tok);
                            out.kept_tokens += 1;
                        }
                        TokenDecision::Convert(t) => {
                            out.text.push_str(&t);
                            out.converted_tokens += 1;
                        }
                        TokenDecision::Ambiguous => {
                            ambiguous = true;
                            out.text.push_str(tok);
                            out.kept_tokens += 1;
                        }
                    }
                }
            }
        }
        if ambiguous || out.converted_tokens == 0 {
            out.confidence = Confidence::Low;
            out.text = text.to_owned();
            out.kept_tokens += out.converted_tokens;
            out.converted_tokens = 0;
        }
        out
    }

    /// Per-token decision of [`Converter::convert_mixed_auto`].
    fn token_decision(
        &self,
        tok: &str,
        last: bool,
        sentence_end: bool,
        mode: ConvertMode,
    ) -> TokenDecision {
        let present: Vec<usize> = (0..self.langs.len())
            .filter(|&i| tok.chars().any(|c| self.langs[i].model.is_letter(c)))
            .collect();
        let s = match present.as_slice() {
            [] => return TokenDecision::Keep,
            [one] => *one,
            // Letters of two languages in one token: only the user knows.
            _ => return TokenDecision::Ambiguous,
        };
        if looks_like_address_or_identifier(tok) {
            return TokenDecision::Keep;
        }
        let src = &self.langs[s];
        let keep_cost = token_cost(tok, src, last);
        let orig_known = words_known(tok, src);
        let mut decision = TokenDecision::Keep;
        for (t, tgt) in self.langs.iter().enumerate() {
            if t == s {
                continue;
            }
            let caps = match caps_rule(tok, src, tgt) {
                // QA-WF-01: `Send it to ЖКХ ещвфн` keeps `ЖКХ`.
                Some(CapsRule::Keep) => Some(TokenDecision::Keep),
                Some(CapsRule::Abbreviation(a)) => Some(TokenDecision::Convert(a)),
                Some(CapsRule::Word) | None => None,
            };
            if let Some(this) = caps {
                decision = match (decision, this) {
                    (TokenDecision::Keep, d) => d,
                    (d, TokenDecision::Keep) => d,
                    _ => TokenDecision::Ambiguous,
                };
                continue;
            }
            let (best, best_cost) = best_candidate(tok, src, tgt, last, sentence_end);
            if looks_technical(tok, &best) {
                return TokenDecision::Keep;
            }
            let margin = keep_cost - best_cost;
            let conv_known = words_known(&best, tgt);
            let this = if conv_known && !orig_known && margin > MIXED_MARGIN {
                if mode == ConvertMode::Literal && best != literal_token(tok, src, tgt) {
                    TokenDecision::Ambiguous
                } else {
                    TokenDecision::Convert(best)
                }
            } else if margin < -MIXED_MARGIN || (orig_known && !conv_known) {
                TokenDecision::Keep
            } else {
                TokenDecision::Ambiguous
            };
            decision = match (decision, this) {
                (TokenDecision::Keep, d) => d,
                (d, TokenDecision::Keep) => d,
                // Two targets both plausible (more than two languages).
                _ => TokenDecision::Ambiguous,
            };
        }
        decision
    }

    /// Language (other than `target`) with the most letters in `tok`, if any.
    fn dominant_foreign(&self, tok: &str, target: usize) -> Option<usize> {
        let mut best: Option<(usize, usize)> = None;
        for (i, l) in self.langs.iter().enumerate() {
            if i == target {
                continue;
            }
            let n = tok.chars().filter(|&c| l.model.is_letter(c)).count();
            if n > 0 && best.is_none_or(|(_, bn)| n > bn) {
                best = Some((i, n));
            }
        }
        best.map(|(i, _)| i)
    }
}

/// Per-token outcome of the automatic mixed-text decision.
enum TokenDecision {
    Keep,
    Convert(String),
    Ambiguous,
}

/// Shortest and longest all-caps token the abbreviation rule applies to (letters).
const ABBREVIATION_LETTERS: std::ops::RangeInclusive<usize> = 2..=5;

/// Outcome of the all-caps abbreviation rule.
enum CapsRule {
    /// A correct abbreviation (or no evidence of a wrong layout): leave the token as typed.
    Keep,
    /// The token is a listed abbreviation of the target typed in the wrong layout.
    Abbreviation(String),
    /// The re-typing is a dictionary word: the usual decision applies.
    Word,
}

/// All-caps tokens of 2-5 letters (QA-WF-01/02). Such a token is an abbreviation far more
/// often than a word typed with Caps Lock, and the character model cannot tell a correct
/// `PDF` from a mistyped one. It is converted only if its key-for-key re-typing is a listed
/// abbreviation (`ЗВА` -> `PDF`) or a dictionary word of the target; a listed abbreviation
/// of its own language always stays. `None`: not an all-caps token, the rule does not apply.
/// Leading `(`/quotes and trailing sentence punctuation are allowed around the letters.
fn caps_rule(tok: &str, src: &Language, tgt: &Language) -> Option<CapsRule> {
    let core = tok
        .trim_start_matches(['(', '"', '«', '\''])
        .trim_end_matches(['.', ',', '!', '?', ':', ';', ')', '"', '»', '\'']);
    let n = core.chars().count();
    if !ABBREVIATION_LETTERS.contains(&n)
        || !core
            .chars()
            .all(|c| c.is_uppercase() && src.model.is_letter(c))
    {
        return None;
    }
    if src.model.knows_abbreviation(core) {
        return Some(CapsRule::Keep);
    }
    let converted = literal_token(core, src, tgt);
    let letters = converted.chars().all(|c| tgt.model.is_letter(c));
    if letters && tgt.model.knows_abbreviation(&converted) {
        // With punctuation around it the usual decision re-types the token as a whole.
        return Some(if core.len() == tok.len() {
            CapsRule::Abbreviation(converted)
        } else {
            CapsRule::Word
        });
    }
    if letters && tgt.model.knows(&converted.to_lowercase()) {
        return Some(CapsRule::Word);
    }
    Some(CapsRule::Keep)
}

/// `true` if every letter run of `s` is a dictionary word of `lang` (and there is one).
fn words_known(s: &str, lang: &Language) -> bool {
    let mut any = false;
    let mut word = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_alphabetic() {
            word.extend(c.to_lowercase());
        } else if !word.is_empty() {
            if !lang.model.knows(&word) {
                return false;
            }
            any = true;
            word.clear();
        }
    }
    any
}

/// A token of a several-token text that is a dictionary word as typed, while its re-typing
/// is not and is clearly worse: the word was meant in the typed language.
fn keeps_own_language(tok: &str, best: &str, src: &Language, tgt: &Language, margin: f32) -> bool {
    margin < -MIXED_MARGIN && words_known(tok, src) && !words_known(best, tgt)
}

/// URL, e-mail, path or programming identifier: never rewritten by the automatic mixed-text
/// decision (`https://x`, `user@host`, `www.x`, `snake_case`, `std::fmt`, `camelCase`).
fn looks_like_address_or_identifier(tok: &str) -> bool {
    if tok.contains("://") || tok.contains("::") || tok.starts_with("www.") {
        return true;
    }
    let chars: Vec<char> = tok.chars().collect();
    let inner = |i: usize| {
        i > 0
            && chars[i - 1].is_alphanumeric()
            && chars.get(i + 1).is_some_and(|c| c.is_alphanumeric())
    };
    chars.iter().enumerate().any(|(i, &c)| {
        ((c == '@' || c == '_') && inner(i))
            || (i > 0 && chars[i - 1].is_lowercase() && c.is_uppercase())
    })
}

/// Literal conversion of `text`: `convert(token)` gives the re-typed token or `None` to keep
/// it. `High` if any token was converted.
fn literal(text: &str, convert: impl Fn(&str) -> Option<String>) -> Conversion {
    let mut out = Conversion {
        text: String::with_capacity(text.len()),
        confidence: Confidence::Low,
        converted_tokens: 0,
        kept_tokens: 0,
    };
    for piece in split_tokens(text) {
        match piece {
            Piece::Space(s) => out.text.push_str(s),
            Piece::Token { text: tok, .. } => match convert(tok) {
                Some(t) => {
                    out.text.push_str(&t);
                    out.converted_tokens += 1;
                    out.confidence = Confidence::High;
                }
                None => {
                    out.text.push_str(tok);
                    out.kept_tokens += 1;
                }
            },
        }
    }
    out
}

/// `tok` re-typed key for key from `src` to `tgt`; `.` `,` `:` between digits stay.
fn literal_token(tok: &str, src: &Language, tgt: &Language) -> String {
    let chars: Vec<char> = tok.chars().collect();
    let digit = |i: Option<usize>| {
        i.and_then(|i| chars.get(i))
            .is_some_and(char::is_ascii_digit)
    };
    chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            if NUMBER_SEPARATORS.contains(&c) && digit(i.checked_sub(1)) && digit(Some(i + 1)) {
                return c;
            }
            src.layout
                .key_of(c)
                .and_then(|k| tgt.layout.char_of(k))
                .unwrap_or(c)
        })
        .collect()
}

enum Piece<'a> {
    Space(&'a str),
    Token { text: &'a str, last: bool },
}

/// Splits into whitespace runs and non-whitespace tokens; marks the last token.
fn split_tokens(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut in_space: Option<bool> = None;
    for (i, ch) in text.char_indices() {
        let sp = ch.is_whitespace();
        if in_space.is_some_and(|s| s != sp) {
            pieces.push(make_piece(&text[start..i], !sp));
            start = i;
        }
        in_space = Some(sp);
    }
    if start < text.len() {
        pieces.push(make_piece(&text[start..], in_space.unwrap_or(false)));
    }
    if let Some(Piece::Token { last, .. }) = pieces
        .iter_mut()
        .rev()
        .find(|p| matches!(p, Piece::Token { .. }))
    {
        *last = true;
    }
    pieces
}

fn make_piece(s: &str, space: bool) -> Piece<'_> {
    if space {
        Piece::Space(s)
    } else {
        Piece::Token {
            text: s,
            last: false,
        }
    }
}

/// Separators a number may contain after conversion (decimal point, thousands, time).
const NUMBER_SEPARATORS: [char; 3] = ['.', ',', ':'];

/// A letterless token re-typed from `src` to `tgt`, if a key between digits gives a number
/// separator in `tgt` (`3/14` -> `3.14`, `15^30` -> `15:30`; QA-B1-02). A `,` may also follow
/// the last digit of a token inside the text (`3?` -> `3,`). `None` if nothing changes.
fn number_separators(tok: &str, src: &Language, tgt: &Language, last: bool) -> Option<String> {
    let chars: Vec<char> = tok.chars().collect();
    let digit = |i: Option<usize>| {
        i.and_then(|i| chars.get(i))
            .is_some_and(|c| c.is_ascii_digit())
    };
    let mut out = chars.clone();
    let mut changed = false;
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_ascii_punctuation() || NUMBER_SEPARATORS.contains(&c) {
            continue;
        }
        let Some(m) = src.layout.key_of(c).and_then(|k| tgt.layout.char_of(k)) else {
            continue;
        };
        if !NUMBER_SEPARATORS.contains(&m) {
            continue;
        }
        let between = digit(i.checked_sub(1)) && digit(Some(i + 1));
        let trailing_comma = m == ',' && !last && i + 1 == chars.len() && digit(i.checked_sub(1));
        if between || trailing_comma {
            out[i] = m;
            changed = true;
        }
    }
    changed.then(|| out.into_iter().collect())
}

/// `true` if a letter touches a digit somewhere in `s` (`v1`, `0x`, `x86`, `a1b2`).
fn letter_digit_contact(s: &str) -> bool {
    let mut prev: Option<char> = None;
    s.chars().any(|c| {
        let touch = prev.is_some_and(|p| {
            (p.is_alphabetic() && c.is_numeric()) || (p.is_numeric() && c.is_alphabetic())
        });
        prev = Some(c);
        touch
    })
}

/// A token that looks like an identifier, a version or a hex literal both as typed and as
/// converted: letters glued to digits on both sides of the conversion (QA-B1-01). A
/// mistyped Russian `2ю0` is not technical: converted it is `2.0`, without letters.
fn looks_technical(tok: &str, converted: &str) -> bool {
    letter_digit_contact(tok) && letter_digit_contact(converted)
}

/// Options for each character of `tok` when re-typed from `src` to `tgt`.
/// The first option is the physically mapped character (preferred on ties).
fn options(tok: &str, src: &Language, tgt: &Language) -> Vec<Vec<char>> {
    tok.chars()
        .map(|c| {
            let mapped = src.layout.key_of(c).and_then(|k| tgt.layout.char_of(k));
            match mapped {
                None => vec![c],
                Some(m) if c.is_alphabetic() || m == c => vec![m],
                Some(m) => vec![m, c],
            }
        })
        .collect()
}

/// `true` if the text has several tokens: its final punctuation then ends a sentence.
fn is_sentence(pieces: &[Piece<'_>]) -> bool {
    pieces
        .iter()
        .filter(|p| matches!(p, Piece::Token { .. }))
        .nth(1)
        .is_some()
}

/// Cheapest re-typing of `tok`. `sentence_end`: the token ends a text of several words, so
/// its final punctuation is a sentence end and is not pushed towards a letter.
fn best_candidate(
    tok: &str,
    src: &Language,
    tgt: &Language,
    last: bool,
    sentence_end: bool,
) -> (String, f32) {
    let opts = options(tok, src, tgt);
    if opts.len() > MAX_ENUMERATED_TOKEN {
        let chars: Vec<char> = opts.iter().map(|o| o[0]).collect();
        let cost = score(&chars, tgt, last, &[]);
        return (chars.into_iter().collect(), cost);
    }
    let ambiguous: Vec<usize> = opts
        .iter()
        .enumerate()
        .filter(|(_, o)| o.len() > 1)
        .map(|(i, _)| i)
        .take(MAX_AMBIGUOUS)
        .collect();
    let mut buf: Vec<char> = opts.iter().map(|o| o[0]).collect();
    // For each position: the letter its key gives, if the other option is punctuation.
    let mut letters: Vec<Option<char>> = opts
        .iter()
        .map(|o| {
            o.iter()
                .copied()
                .find(|c| c.is_alphabetic())
                .filter(|_| o.len() > 1)
        })
        .collect();
    if sentence_end && let Some(l) = letters.last_mut() {
        // `Cgfcb,j pf gjvjom.` -> `Спасибо за помощь.`, not `помощью`.
        *l = None;
    }
    let mut best: Option<(Vec<char>, f32)> = None;
    for mask in 0u32..(1u32 << ambiguous.len()) {
        for (bit, &pos) in ambiguous.iter().enumerate() {
            buf[pos] = opts[pos][((mask >> bit) & 1) as usize];
        }
        let cost = score(&buf, tgt, last, &letters);
        if best.as_ref().is_none_or(|(_, bc)| cost < *bc) {
            best = Some((buf.clone(), cost));
        }
    }
    let (chars, cost) = best.unwrap_or((buf, f32::INFINITY));
    (chars.into_iter().collect(), cost)
}

fn token_cost(tok: &str, lang: &Language, last: bool) -> f32 {
    let chars: Vec<char> = tok.chars().collect();
    score(&chars, lang, last, &[])
}

/// Cost of a token under `lang`: word costs plus punctuation priors. `letters[i]`, when
/// given, is the letter the key at position `i` produces in `lang` (ambiguous punctuation).
fn score(chars: &[char], lang: &Language, last: bool, letters: &[Option<char>]) -> f32 {
    let mut cost = 0.0;
    let mut word = String::new();
    let flush = |word: &mut String, cost: &mut f32| {
        if !word.is_empty() {
            *cost += lang.model.word_cost(word);
            word.clear();
        }
    };
    // O(n) precomputation: is there a letter before / after position i?
    let first_letter = chars.iter().position(|c| c.is_alphabetic());
    let last_letter = chars.iter().rposition(|c| c.is_alphabetic());
    let letter_before = |i: usize| first_letter.is_some_and(|f| f < i);
    let letter_after = |i: usize| last_letter.is_some_and(|l| l > i);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_alphabetic() {
            word.extend(c.to_lowercase());
            continue;
        }
        // Word-final punctuation whose key would have completed a dictionary word.
        let completes_word = !word.is_empty()
            && !letter_after(i)
            && letters.get(i).copied().flatten().is_some_and(|l| {
                let mut w = word.clone();
                w.extend(l.to_lowercase());
                lang.model.knows(&w)
            });
        if completes_word {
            cost += LETTER_WORD_PENALTY;
        }
        flush(&mut word, &mut cost);
        if c.is_numeric() {
            continue;
        }
        let adjacent_letters = i > 0
            && chars[i - 1].is_alphabetic()
            && chars.get(i + 1).is_some_and(|n| n.is_alphabetic());
        let is_final = last && i + 1 == chars.len();
        cost += if adjacent_letters {
            if c == '-' || lang.joiners.contains(&c) {
                0.5
            } else {
                12.0
            }
        } else if letter_before(i) && !letter_after(i) {
            match c {
                '.' | '?' | '!' if is_final => 0.3,
                ',' | ';' | ':' if is_final => 4.0,
                ',' | '.' => 1.0,
                '?' | '!' | ')' | '"' | '»' => 1.5,
                ':' | ';' => 2.5,
                _ => 4.0,
            }
        } else if !letter_before(i) && letter_after(i) {
            match c {
                '"' | '(' | '«' => 1.5,
                _ => 4.0,
            }
        } else {
            4.0
        };
    }
    flush(&mut word, &mut cost);
    cost
}
