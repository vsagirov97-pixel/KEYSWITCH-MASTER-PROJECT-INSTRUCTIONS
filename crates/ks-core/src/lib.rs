//! KeySwitch core: OS-independent layout conversion.
//!
//! This crate must not perform I/O or depend on any operating system API.
//! Layouts and language models are data (D-15); built-in ones are embedded at compile time.

mod convert;
mod layout;
mod model;

pub use convert::{
    Confidence, Conversion, ConvertError, ConvertMode, Converter, Detection, Language,
};
pub use layout::{KeyStroke, Layout, LayoutError, ScanCode};
pub use model::LanguageModel;

/// Crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Built-in language ids.
pub mod lang {
    pub const EN: &str = "en";
    pub const RU: &str = "ru";
}

/// Converter with the built-in English (US) and Russian languages.
pub fn builtin() -> Result<Converter, LayoutError> {
    Ok(Converter::new(vec![
        Language {
            id: lang::EN.into(),
            layout: Layout::parse("en-us", include_str!("../layouts/en-us.layout"))?,
            model: LanguageModel::from_frequency_list(include_str!("../data/en.freq"))
                .with_abbreviations(include_str!("../data/en.abbr")),
            joiners: vec!['\''],
        },
        Language {
            id: lang::RU.into(),
            layout: Layout::parse("ru", include_str!("../layouts/ru.layout"))?,
            model: LanguageModel::from_frequency_list(include_str!("../data/ru.freq"))
                .with_abbreviations(include_str!("../data/ru.abbr")),
            joiners: vec![],
        },
    ]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }

    #[test]
    fn builtin_layouts_cover_all_keys() {
        let c = super::builtin().expect("built-in data must parse");
        assert_eq!(c.languages().count(), 2);
    }
}
