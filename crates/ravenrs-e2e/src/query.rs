//! What to look for on the screen.

use std::fmt;

use ravenrs_core::{AccessNode, Role};

/// What a test looks for: a node of the accessibility tree, by the id the app declared, by
/// role and name, by the text it shows or by its reference in a snapshot. Narrow it with
/// [`Query::name`], [`Query::within`] and [`Query::nth`].
///
/// The test id is the sturdiest choice, because it survives reordering, wrapping and new
/// wording. Role and name read the way a person does. Never a position on the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    role: Option<Role>,
    name: Option<String>,
    test_id: Option<String>,
    text: Option<String>,
    reference: Option<String>,
    within: Option<Box<Query>>,
    nth: Option<usize>,
}

impl Query {
    /// Nodes with this role, such as `Query::role(Role::Button).name("Save")`.
    pub fn role(role: Role) -> Self {
        Self { role: Some(role), ..Self::default() }
    }

    /// The node the app tagged with `ViewExt::test_id`.
    pub fn test_id(id: impl Into<String>) -> Self {
        Self { test_id: Some(id.into()), ..Self::default() }
    }

    /// Nodes whose name, or whose value when it is not secret, is exactly this text.
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: Some(text.into()), ..Self::default() }
    }

    /// The node a snapshot showed as `[ref=e7]`; pass `"e7"`.
    pub fn by_ref(reference: impl Into<String>) -> Self {
        Self { reference: Some(reference.into()), ..Self::default() }
    }

    /// Only nodes with exactly this accessible name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Only nodes inside the one `parent` finds, such as the buttons of one dialog.
    pub fn within(mut self, parent: Query) -> Self {
        self.within = Some(Box::new(parent));
        self
    }

    /// The match at this position, counting from zero in reading order, when several match.
    pub fn nth(mut self, index: usize) -> Self {
        self.nth = Some(index);
        self
    }

    pub(crate) fn parent(&self) -> Option<&Query> {
        self.within.as_deref()
    }

    pub(crate) fn index(&self) -> Option<usize> {
        self.nth
    }

    pub(crate) fn reference(&self) -> Option<&str> {
        self.reference.as_deref()
    }

    /// Whether `node` matches everything but `within`, `nth` and the reference.
    pub(crate) fn matches(&self, node: &AccessNode) -> bool {
        self.role.is_none_or(|role| node.role == role)
            && self.name.as_ref().is_none_or(|name| node.name.as_ref() == Some(name))
            && self.test_id.as_ref().is_none_or(|id| node.test_id.as_ref() == Some(id))
            && self.text.as_ref().is_none_or(|text| {
                node.name.as_ref() == Some(text) || (!node.protected && node.value.as_ref() == Some(text))
            })
    }

    /// How close `node` comes, for the "close matches" of a failure: higher is closer, zero is
    /// not close at all.
    pub(crate) fn closeness(&self, node: &AccessNode) -> u32 {
        let mut score = 0;
        if self.role.is_some_and(|role| node.role == role) {
            score += 2;
        }
        let wanted = self.name.as_deref().or(self.text.as_deref());
        if let (Some(wanted), Some(name)) = (wanted, node.name.as_deref()) {
            score += similarity(wanted, name);
        }
        if let (Some(wanted), Some(id)) = (self.test_id.as_deref(), node.test_id.as_deref()) {
            score += similarity(wanted, id);
        }
        score
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(role) = self.role {
            parts.push(format!("role {}", crate::snapshot::role_name(role)));
        }
        if let Some(name) = &self.name {
            parts.push(format!("name {name:?}"));
        }
        if let Some(id) = &self.test_id {
            parts.push(format!("test id #{id}"));
        }
        if let Some(text) = &self.text {
            parts.push(format!("text {text:?}"));
        }
        if let Some(reference) = &self.reference {
            parts.push(format!("ref {reference}"));
        }
        if let Some(n) = self.nth {
            parts.push(format!("match {n}"));
        }
        f.write_str(&parts.join(", "))?;
        if let Some(parent) = &self.within {
            write!(f, " within ({parent})")?;
        }
        Ok(())
    }
}

/// 3 for the same text ignoring case, 2 when one contains the other, 1 when a few edits apart.
fn similarity(a: &str, b: &str) -> u32 {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    if a == b {
        3
    } else if a.contains(&b) || b.contains(&a) {
        2
    } else if edits(&a, &b) <= (a.chars().count().max(b.chars().count()) / 3).max(2) {
        1
    } else {
        0
    }
}

/// Levenshtein distance between two short strings.
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let replaced = previous + usize::from(ca != *cb);
            previous = row[j + 1];
            row[j + 1] = replaced.min(row[j] + 1).min(previous + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_words_score_by_how_close_they_are() {
        assert_eq!(similarity("Salvar", "salvar"), 3);
        assert_eq!(similarity("Salvar", "Salvar tudo"), 2);
        assert_eq!(similarity("Salvar", "Salvr"), 1);
        assert_eq!(similarity("Salvar", "Cancelar tudo"), 0);
        assert_eq!(edits("kitten", "sitting"), 3);
    }
}
