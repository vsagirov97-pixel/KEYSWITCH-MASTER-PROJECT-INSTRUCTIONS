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

    /// Converts the whole text as typed in `from`'s layout into `to`.
    /// Confidence is `Low` when the original looks more plausible than the result.
    pub fn convert(&self, text: &str, from: &str, to: &str) -> Result<Conversion, ConvertError> {
        let (f, t) = (self.lang(from)?, self.lang(to)?);
        if f == t {
            return Err(ConvertError::SameLanguage);
        }
        let (src, tgt) = (&self.langs[f], &self.langs[t]);
        let mut out = Conversion {
            text: String::with_capacity(text.len()),
            confidence: Confidence::High,
            converted_tokens: 0,
            kept_tokens: 0,
        };
        let mut margin_sum = 0.0;
        let mut any_letters = false;
        let pieces = split_tokens(text);
        let sentence = is_sentence(&pieces);
        for piece in pieces {
            match piece {
                Piece::Space(s) => out.text.push_str(s),
                Piece::Token { text: tok, last } => {
                    if !tok.chars().any(|c| src.model.is_letter(c)) {
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                        continue;
                    }
                    let (best, best_cost) = best_candidate(tok, src, tgt, last, last && sentence);
                    if looks_technical(tok, &best) {
                        // Identifier, version or hex literal (`v1.2.3`, `0xFF`, `x86_64`):
                        // never rewritten, and it does not count as convertible text.
                        out.text.push_str(tok);
                        out.kept_tokens += 1;
                        continue;
                    }
                    any_letters = true;
                    margin_sum += token_cost(tok, src, last) - best_cost;
                    out.text.push_str(&best);
                    out.converted_tokens += 1;
                }
            }
        }
        if !any_letters || margin_sum < 0.0 {
            out.confidence = Confidence::Low;
        }
        Ok(out)
    }

    /// Mixed text: converts tokens not written in `to` into `to`, keeping tokens that are
    /// already `to` or look like genuine words of their own language.
    pub fn convert_mixed(&self, text: &str, to: &str) -> Result<Conversion, ConvertError> {
        let t = self.lang(to)?;
        let tgt = &self.langs[t];
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
