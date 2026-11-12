//! What screen readers hear from the framework itself, in each language.
//!
//! Every text has a key under `a11y.`, as `a11y.saved`. The words come in English and in
//! Brazilian Portuguese; [`AccessWords::from_lookup`] takes them from anywhere else that answers
//! by key, which is where the words of the whole application framework plug in.

/// A text of [`AccessWords`] by its key, for whoever keeps the words of an application.
pub const KEYS: &[&str] =
    &["a11y.saved", "a11y.not_saved", "a11y.deleted_one", "a11y.deleted_many", "a11y.showing"];

/// The words of the announcements. `{0}` is what each one is about.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AccessWords {
    /// A record was kept: `{0}` is what names it.
    pub saved: String,
    /// A record could not be kept: `{0}` is why.
    pub not_saved: String,
    /// One record was deleted.
    pub deleted_one: String,
    /// More than one was: `{0}` is how many.
    pub deleted_many: String,
    /// A screen opened: `{0}` is its title.
    pub showing: String,
}

impl Default for AccessWords {
    fn default() -> Self {
        Self::english()
    }
}

impl AccessWords {
    /// In English.
    pub fn english() -> Self {
        Self {
            saved: "Saved: {0}".into(),
            not_saved: "Not saved. {0}".into(),
            deleted_one: "1 record deleted".into(),
            deleted_many: "{0} records deleted".into(),
            showing: "Showing {0}".into(),
        }
    }

    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            saved: "Salvo: {0}".into(),
            not_saved: "Não foi salvo. {0}".into(),
            deleted_one: "1 registro apagado".into(),
            deleted_many: "{0} registros apagados".into(),
            showing: "Mostrando {0}".into(),
        }
    }

    /// In the language of this code, as `en` or `pt-BR`; English for any other.
    pub fn for_language(code: &str) -> Self {
        let code = code.trim().replace('_', "-").to_ascii_lowercase();
        if code == "pt" || code.starts_with("pt-") { Self::pt_br() } else { Self::english() }
    }

    /// Takes each text from `lookup` by its key in [`KEYS`], keeping these words for a key it
    /// does not know. This is how the words of an application, kept anywhere, reach here.
    pub fn from_lookup(self, lookup: impl Fn(&str) -> Option<String>) -> Self {
        let take = |key: &str, kept: String| lookup(key).unwrap_or(kept);
        Self {
            saved: take("a11y.saved", self.saved),
            not_saved: take("a11y.not_saved", self.not_saved),
            deleted_one: take("a11y.deleted_one", self.deleted_one),
            deleted_many: take("a11y.deleted_many", self.deleted_many),
            showing: take("a11y.showing", self.showing),
        }
    }

    /// The text of this key, for the ones in [`KEYS`].
    pub fn get(&self, key: &str) -> Option<&str> {
        Some(match key {
            "a11y.saved" => &self.saved,
            "a11y.not_saved" => &self.not_saved,
            "a11y.deleted_one" => &self.deleted_one,
            "a11y.deleted_many" => &self.deleted_many,
            "a11y.showing" => &self.showing,
            _ => return None,
        })
    }

    /// That a record named `name` was kept.
    pub fn saved(&self, name: &str) -> String {
        self.saved.replace("{0}", name)
    }

    /// That a record was not kept, and why.
    pub fn not_saved(&self, why: &str) -> String {
        self.not_saved.replace("{0}", why).trim().to_owned()
    }

    /// That `count` records were deleted.
    pub fn deleted(&self, count: usize) -> String {
        if count == 1 {
            self.deleted_one.clone()
        } else {
            self.deleted_many.replace("{0}", &count.to_string())
        }
    }

    /// That the screen with this title opened.
    pub fn showing(&self, title: &str) -> String {
        self.showing.replace("{0}", title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_words_in_each_language() {
        for words in [AccessWords::english(), AccessWords::pt_br()] {
            for key in KEYS {
                assert!(words.get(key).is_some_and(|text| !text.is_empty()), "{key}");
            }
        }
        assert_eq!(AccessWords::for_language("pt_BR"), AccessWords::pt_br());
        assert_eq!(AccessWords::for_language("de"), AccessWords::english());
    }

    #[test]
    fn words_from_elsewhere_replace_by_key() {
        let words =
            AccessWords::pt_br().from_lookup(|key| (key == "a11y.saved").then(|| "Gravado: {0}".to_owned()));
        assert_eq!(words.saved("Conta"), "Gravado: Conta");
        assert_eq!(words.deleted(3), "3 registros apagados");
        assert_eq!(words.deleted(1), "1 registro apagado");
    }
}
