//! The languages and the words of a RavenRS business application, in one place.
//!
//! Every text the framework shows has a key, as `ui.save` or `report.show`. The texts that come
//! with it are in English and in Brazilian Portuguese; a language without a key falls back to
//! English, and a key nobody has shows as itself. An application or a module adds or replaces
//! texts with a flat JSON of keys, without code of its own for the screens.
//!
//! ```
//! use ravenrs_app_i18n::{Catalog, Language, Texts};
//!
//! let texts = Texts::new(Language::BrazilianPortuguese);
//! assert_eq!(texts.get("ui.save"), "Salvar");
//! assert_eq!(texts.format("rules.required", &["Nome"]), "Nome é obrigatório");
//!
//! let mut catalog = Catalog::builtin();
//! catalog.merge_json("pt-BR", r#"{ "ui.save": "Gravar", "shop.checkout": "Fechar pedido" }"#)?;
//! let texts = catalog.texts(Language::BrazilianPortuguese);
//! assert_eq!(texts.get("ui.save"), "Gravar");
//! assert_eq!(texts.get("shop.checkout"), "Fechar pedido");
//! # Ok::<(), ravenrs_app_i18n::TextError>(())
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

pub mod model;

/// The language everything falls back to.
pub const FALLBACK: &str = "en";

/// The texts that come with the framework, by language and by part, as flat JSON.
const BUILTIN: &[(&str, &str)] = &[
    ("en", include_str!("../texts/en/ai.json")),
    ("en", include_str!("../texts/en/format.json")),
    ("en", include_str!("../texts/en/import.json")),
    ("en", include_str!("../texts/en/movie.json")),
    ("en", include_str!("../texts/en/pane.json")),
    ("en", include_str!("../texts/en/report.json")),
    ("en", include_str!("../texts/en/rules.json")),
    ("en", include_str!("../texts/en/sqlite.json")),
    ("en", include_str!("../texts/en/spreadsheet.json")),
    ("en", include_str!("../texts/en/ui.json")),
    ("en", include_str!("../texts/en/viewer.json")),
    ("en", include_str!("../texts/en/widgets.json")),
    ("pt-BR", include_str!("../texts/pt-BR/ai.json")),
    ("pt-BR", include_str!("../texts/pt-BR/format.json")),
    ("pt-BR", include_str!("../texts/pt-BR/import.json")),
    ("pt-BR", include_str!("../texts/pt-BR/movie.json")),
    ("pt-BR", include_str!("../texts/pt-BR/pane.json")),
    ("pt-BR", include_str!("../texts/pt-BR/report.json")),
    ("pt-BR", include_str!("../texts/pt-BR/rules.json")),
    ("pt-BR", include_str!("../texts/pt-BR/sqlite.json")),
    ("pt-BR", include_str!("../texts/pt-BR/spreadsheet.json")),
    ("pt-BR", include_str!("../texts/pt-BR/ui.json")),
    ("pt-BR", include_str!("../texts/pt-BR/viewer.json")),
    ("pt-BR", include_str!("../texts/pt-BR/widgets.json")),
];

/// A language the screens can be in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Language {
    /// English.
    #[default]
    English,
    /// Brazilian Portuguese.
    BrazilianPortuguese,
}

impl Language {
    /// Every language the framework comes with texts for.
    pub const ALL: &'static [Language] = &[Self::English, Self::BrazilianPortuguese];

    /// What texts and translations of the model are known by, as `en` or `pt-BR`.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::BrazilianPortuguese => "pt-BR",
        }
    }

    /// The language of a code, without minding case or `_` for `-`. A bare `pt` is Brazilian
    /// Portuguese, and English is any `en`.
    pub fn from_code(code: &str) -> Option<Self> {
        let code = code.trim().replace('_', "-").to_ascii_lowercase();
        match code.as_str() {
            "pt" | "pt-br" => Some(Self::BrazilianPortuguese),
            "en" => Some(Self::English),
            _ if code.starts_with("en-") => Some(Self::English),
            _ => None,
        }
    }

    /// Its name, in itself.
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::BrazilianPortuguese => "Português (Brasil)",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What is wrong with texts given as JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextError(pub String);

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TextError {}

/// Texts by language and by key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    languages: BTreeMap<String, BTreeMap<String, String>>,
}

impl Catalog {
    /// With nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// With the texts that come with the framework.
    pub fn builtin() -> Self {
        static BUILT: OnceLock<Catalog> = OnceLock::new();
        BUILT
            .get_or_init(|| {
                let mut catalog = Catalog::new();
                for (language, json) in BUILTIN {
                    // The tests of this crate read each of them, so this does not fail.
                    let read = catalog.merge_json(language, json);
                    debug_assert!(read.is_ok(), "{read:?}");
                }
                catalog
            })
            .clone()
    }

    /// Puts this text under this key, in this language, over what was there.
    pub fn set(&mut self, language: &str, key: impl Into<String>, text: impl Into<String>) {
        self.languages.entry(language.to_owned()).or_default().insert(key.into(), text.into());
    }

    /// The same, as a builder.
    pub fn with(mut self, language: &str, key: impl Into<String>, text: impl Into<String>) -> Self {
        self.set(language, key, text);
        self
    }

    /// Puts the texts of a flat JSON object of keys, as `{ "ui.save": "Gravar" }`, over what was
    /// there. Nothing is put when something in it is not text.
    pub fn merge_json(&mut self, language: &str, json: &str) -> Result<(), TextError> {
        let wrong = |detail: String| TextError(format!("the texts of {language}: {detail}"));
        let value: serde_json::Value = serde_json::from_str(json).map_err(|e| wrong(e.to_string()))?;
        let serde_json::Value::Object(entries) = value else {
            return Err(wrong("they are not an object of keys".into()));
        };
        let mut read = Vec::with_capacity(entries.len());
        for (key, text) in entries {
            match text {
                serde_json::Value::String(text) => read.push((key, text)),
                _ => return Err(wrong(format!("`{key}` is not text"))),
            }
        }
        let texts = self.languages.entry(language.to_owned()).or_default();
        texts.extend(read);
        Ok(())
    }

    /// Puts every text of the other over what is here.
    pub fn merge(&mut self, other: &Catalog) {
        for (language, texts) in &other.languages {
            let mine = self.languages.entry(language.clone()).or_default();
            mine.extend(texts.iter().map(|(key, text)| (key.clone(), text.clone())));
        }
    }

    /// The text of a key in a language, or in English when the language does not have it.
    pub fn text(&self, language: &str, key: &str) -> Option<&str> {
        self.own_text(language, key).or_else(|| self.own_text(FALLBACK, key))
    }

    /// The text of a key in a language, and only in it.
    pub fn own_text(&self, language: &str, key: &str) -> Option<&str> {
        self.languages.get(language)?.get(key).map(String::as_str)
    }

    /// The languages it has texts in.
    pub fn languages(&self) -> impl Iterator<Item = &str> {
        self.languages.keys().map(String::as_str)
    }

    /// The keys it has in a language.
    pub fn keys(&self, language: &str) -> impl Iterator<Item = &str> {
        self.languages.get(language).into_iter().flat_map(|texts| texts.keys().map(String::as_str))
    }

    /// The keys English has and this language does not, for whoever translates.
    pub fn missing(&self, language: &str) -> Vec<String> {
        self.keys(FALLBACK).filter(|key| self.own_text(language, key).is_none()).map(str::to_owned).collect()
    }

    /// Its texts in this language.
    pub fn texts(&self, language: Language) -> Texts {
        Texts::of(self.clone(), language.code())
    }
}

/// The texts of a catalog in one language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texts {
    catalog: Arc<Catalog>,
    language: String,
}

impl Texts {
    /// The texts that come with the framework, in this language.
    pub fn new(language: Language) -> Self {
        Catalog::builtin().texts(language)
    }

    /// Those of this catalog, in the language of this code, which may be one the framework does
    /// not come with.
    pub fn of(catalog: Catalog, language: impl Into<String>) -> Self {
        Self { catalog: Arc::new(catalog), language: language.into() }
    }

    /// The code of its language.
    pub fn language(&self) -> &str {
        &self.language
    }

    /// The catalog it reads from.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The text of a key, falling back to English, and to the key itself when nobody has it.
    pub fn get(&self, key: &str) -> String {
        self.catalog.text(&self.language, key).unwrap_or(key).to_owned()
    }

    /// The first character of the text of a key, or this when there is none.
    pub fn char(&self, key: &str, otherwise: char) -> char {
        self.catalog.text(&self.language, key).and_then(|text| text.chars().next()).unwrap_or(otherwise)
    }

    /// The texts under `key.0`, `key.1`... as many as there are.
    pub fn list(&self, key: &str) -> Vec<String> {
        (0..)
            .map_while(|i| self.catalog.text(&self.language, &format!("{key}.{i}")).map(str::to_owned))
            .collect()
    }

    /// The texts under `key.0` to `key.{N-1}`.
    pub fn array<const N: usize>(&self, key: &str) -> [String; N] {
        std::array::from_fn(|i| self.get(&format!("{key}.{i}")))
    }

    /// The texts under `key.0` and `key.1`.
    pub fn pair(&self, key: &str) -> (String, String) {
        (self.get(&format!("{key}.0")), self.get(&format!("{key}.1")))
    }

    /// The pairs under `key.0`, `key.1`... as many as there are, each with its `.0` and `.1`.
    pub fn pair_list(&self, key: &str) -> Vec<(String, String)> {
        (0..)
            .map_while(|i| {
                let at = format!("{key}.{i}");
                self.catalog.text(&self.language, &format!("{at}.0")).map(|_| self.pair(&at))
            })
            .collect()
    }

    /// The pairs under `key.0` to `key.{N-1}`.
    pub fn pairs<const N: usize>(&self, key: &str) -> [(String, String); N] {
        std::array::from_fn(|i| self.pair(&format!("{key}.{i}")))
    }

    /// The text of a key with `{0}`, `{1}`... replaced by these.
    pub fn format(&self, key: &str, values: &[&str]) -> String {
        format(&self.get(key), values)
    }
}

impl Default for Texts {
    fn default() -> Self {
        Self::new(Language::default())
    }
}

/// A template with `{0}`, `{1}`... replaced by these. A number with nothing for it stays as it was.
pub fn format(template: &str, values: &[&str]) -> String {
    format_with(template, |index, out| match values.get(index) {
        Some(value) => {
            out.push_str(value);
            true
        }
        None => false,
    })
}

/// A template with each `{0}`, `{1}`... written by `value`, straight into what it gives, so nothing
/// else is made for each value. `value` says `false` for a number it has nothing for, which then
/// stays as it was.
pub fn format_with(template: &str, mut value: impl FnMut(usize, &mut String) -> bool) -> String {
    let mut out = String::with_capacity(template.len() + 8);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let index =
            after.find('}').and_then(|close| after[..close].parse::<usize>().ok().map(|i| (i, close)));
        match index {
            Some((index, close)) if value(index, &mut out) => rest = &after[close + 1..],
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
