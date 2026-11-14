//! Language model: word unigram costs plus a character trigram model for unknown words.
//!
//! Costs are negative natural log probabilities (lower = more likely).

use std::collections::{HashMap, HashSet};

const BOS: char = '^';
const EOS: char = '$';
/// Extra cost for a word that is not in the dictionary.
const OOV_PENALTY: f32 = 6.0;
/// Add-k smoothing constant for the character model.
const SMOOTH_K: f32 = 0.05;

#[derive(Debug, Clone)]
pub struct LanguageModel {
    words: HashMap<String, f32>,
    alphabet: HashSet<char>,
    tri: HashMap<(char, char, char), f32>,
    bi: HashMap<(char, char), f32>,
    /// Upper-case abbreviations of this language (`PDF`, `ЖКХ`; QA-WF-01/02).
    abbreviations: HashSet<String>,
}

impl LanguageModel {
    /// Builds a model from `word count` lines; `#` starts a comment line.
    pub fn from_frequency_list(src: &str) -> Self {
        let mut counts: Vec<(String, f64)> = Vec::new();
        for line in src.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.split_whitespace();
            let (Some(w), Some(c)) = (it.next(), it.next()) else {
                continue;
            };
            let Ok(c) = c.parse::<f64>() else { continue };
            if c > 0.0 {
                counts.push((w.to_lowercase(), c));
            }
        }
        let total: f64 = counts.iter().map(|(_, c)| c).sum::<f64>().max(1.0);

        let mut words = HashMap::with_capacity(counts.len());
        let mut alphabet = HashSet::new();
        let mut tri: HashMap<(char, char, char), f32> = HashMap::new();
        let mut bi: HashMap<(char, char), f32> = HashMap::new();
        for (w, c) in &counts {
            words.insert(w.clone(), -((c / total).ln()) as f32);
            // Soften frequency so the character model reflects word shapes, not just "и"/"the".
            let weight = (1.0 + c).ln() as f32;
            let chars: Vec<char> = std::iter::once(BOS)
                .chain(std::iter::once(BOS))
                .chain(w.chars())
                .chain(std::iter::once(EOS))
                .collect();
            for ch in w.chars().filter(|c| c.is_alphabetic()) {
                alphabet.insert(ch);
            }
            for win in chars.windows(3) {
                *tri.entry((win[0], win[1], win[2])).or_default() += weight;
                *bi.entry((win[0], win[1])).or_default() += weight;
            }
        }
        Self {
            words,
            alphabet,
            tri,
            bi,
            abbreviations: HashSet::new(),
        }
    }

    /// Adds the abbreviations of `src`: one per line, `#` starts a comment line. They are
    /// stored upper-case; a word that is not all letters of this language is ignored.
    #[must_use]
    pub fn with_abbreviations(mut self, src: &str) -> Self {
        for line in src.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let upper = line.to_uppercase();
            if upper.chars().all(|c| self.is_letter(c)) {
                self.abbreviations.insert(upper);
            }
        }
        self
    }

    /// True if `word` (compared upper-case) is a listed abbreviation of this language.
    pub fn knows_abbreviation(&self, word: &str) -> bool {
        self.abbreviations.contains(&word.to_uppercase())
    }

    /// True if `ch` (any case) is a letter of this language.
    pub fn is_letter(&self, ch: char) -> bool {
        ch.to_lowercase().all(|c| self.alphabet.contains(&c))
    }

    /// Cost of a lowercase word.
    pub fn word_cost(&self, word: &str) -> f32 {
        if let Some(&c) = self.words.get(word) {
            return c;
        }
        let v = (self.alphabet.len() + 1) as f32;
        let mut cost = OOV_PENALTY;
        let mut a = BOS;
        let mut b = BOS;
        for c in word.chars().chain(std::iter::once(EOS)) {
            let n = self.tri.get(&(a, b, c)).copied().unwrap_or(0.0);
            let d = self.bi.get(&(a, b)).copied().unwrap_or(0.0);
            cost -= ((n + SMOOTH_K) / (d + SMOOTH_K * v)).ln();
            a = b;
            b = c;
        }
        cost
    }

    pub fn knows(&self, word: &str) -> bool {
        self.words.contains_key(word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_words_are_cheaper_than_garbage() {
        let m = LanguageModel::from_frequency_list("# c\nhello 100\nworld 50\nhelp 10\n");
        assert!(m.knows("hello"));
        assert!(m.word_cost("hello") < m.word_cost("world"));
        assert!(m.word_cost("hellp") < m.word_cost("xqzv"));
        assert!(m.is_letter('H'));
        assert!(!m.is_letter('я'));
    }

    #[test]
    fn abbreviations_are_upper_case_letters_of_the_language() {
        let m = LanguageModel::from_frequency_list("pdf 1\nhttp 1\n")
            .with_abbreviations("# c\npdf\nHTTP\nЖКХ\n");
        assert!(m.knows_abbreviation("PDF"));
        assert!(m.knows_abbreviation("http"));
        assert!(!m.knows_abbreviation("ЖКХ"), "not letters of this language");
        assert!(!m.knows_abbreviation("PD"));
    }
}
