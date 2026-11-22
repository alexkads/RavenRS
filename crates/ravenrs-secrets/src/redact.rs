//! Keeping secret values out of logs, errors and events, the same way everywhere.

use std::sync::{Arc, Mutex, PoisonError};

/// What a secret value becomes in text.
pub const MARK: &str = "•••";
/// Shorter values are not taken out: they would mark ordinary words.
const SHORTEST: usize = 4;

/// `text` with every one of `values` replaced by [`MARK`], longest first, so a value that holds
/// another goes whole.
pub fn redact(text: &str, values: &[String]) -> String {
    let mut values: Vec<&String> = values.iter().filter(|value| value.chars().count() >= SHORTEST).collect();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.iter().fold(text.to_owned(), |text, value| text.replace(value.as_str(), MARK))
}

/// The values to keep out of text, shared by everything that writes it. A [`Vault`](crate::Vault)
/// adds each value it hands out. Clones share the same values.
#[derive(Clone, Default)]
pub struct Redactor {
    values: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The values themselves never show.
        f.debug_struct("Redactor").field("values", &self.len()).finish()
    }
}

impl Redactor {
    /// Keeping nothing out yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Keeps `value` out from now on.
    pub fn add(&self, value: &str) {
        if value.chars().count() < SHORTEST {
            return;
        }
        let mut values = self.values.lock().unwrap_or_else(PoisonError::into_inner);
        if !values.iter().any(|known| known == value) {
            values.push(value.to_owned());
        }
    }

    /// `text` with every value it keeps replaced by [`MARK`].
    pub fn redact(&self, text: &str) -> String {
        let values = self.values.lock().unwrap_or_else(PoisonError::into_inner);
        redact(text, &values)
    }

    /// How many values it keeps out.
    pub fn len(&self) -> usize {
        self.values.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Whether it keeps nothing out.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
