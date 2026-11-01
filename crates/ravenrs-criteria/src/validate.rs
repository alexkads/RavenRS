//! Static checks against a schema, before anything runs: what a designer shows
//! while the user types, and what a server checks before it accepts a rule.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::Criteria;

/// The kind of value a field holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FieldType {
    /// Text.
    Text,
    /// A number.
    Number,
    /// A date and time.
    Date,
    /// True or false.
    Boolean,
    /// Any kind of value.
    Any,
}

/// What an expression may refer to.
///
/// ```
/// use ravenrs_criteria::{Criteria, FieldType, Schema, validate};
///
/// let schema = Schema::new().field("Price", FieldType::Number).field("Name", FieldType::Text);
/// let ok = Criteria::parse("[Price] > 10").expect("valid");
/// assert!(validate(&ok, &schema).is_empty());
/// let bad = Criteria::parse("[Prize] > 10 And Upper([Price]) = 'X'").expect("valid");
/// assert_eq!(validate(&bad, &schema).len(), 1, "the misspelled field");
/// ```
#[derive(Clone, Debug, Default)]
pub struct Schema {
    fields: BTreeMap<String, FieldType>,
    collections: BTreeMap<String, Schema>,
    parameters: BTreeMap<String, FieldType>,
    functions: BTreeSet<String>,
}

impl Schema {
    /// An empty schema.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a field; a dotted path reads a nested value.
    pub fn field(mut self, path: &str, kind: FieldType) -> Self {
        self.fields.insert(path.to_owned(), kind);
        self
    }

    /// Adds a nested collection with the schema of its rows.
    pub fn collection(mut self, path: &str, rows: Schema) -> Self {
        self.collections.insert(path.to_owned(), rows);
        self
    }

    /// Adds a parameter.
    pub fn parameter(mut self, name: &str, kind: FieldType) -> Self {
        self.parameters.insert(name.to_owned(), kind);
        self
    }

    /// Declares a custom function.
    pub fn function(mut self, name: &str) -> Self {
        self.functions.insert(name.to_lowercase());
        self
    }
}

/// Something wrong with an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Problem {
    /// A field the schema does not have.
    UnknownField(String),
    /// A collection the schema does not have.
    UnknownCollection(String),
    /// A parameter the schema does not have.
    UnknownParameter(String),
    /// A function that is neither built in nor declared.
    UnknownFunction(String),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownField(n) => write!(f, "unknown field '{n}'"),
            Self::UnknownCollection(n) => write!(f, "unknown collection '{n}'"),
            Self::UnknownParameter(n) => write!(f, "unknown parameter '{n}'"),
            Self::UnknownFunction(n) => write!(f, "unknown function '{n}'"),
        }
    }
}

/// Every problem found in `criteria`, in order, without repeats.
pub fn validate(criteria: &Criteria, schema: &Schema) -> Vec<Problem> {
    let mut problems = Vec::new();
    check(criteria, schema, None, &mut problems);
    problems
}

fn add(problems: &mut Vec<Problem>, p: Problem) {
    if !problems.contains(&p) {
        problems.push(p);
    }
}

fn check(c: &Criteria, schema: &Schema, parent: Option<&Schema>, problems: &mut Vec<Problem>) {
    match c {
        Criteria::Property(path) => {
            let (owner, path) = match path.split_first() {
                Some((first, rest)) if first == "^" => (parent.unwrap_or(schema), rest),
                _ => (schema, path.as_slice()),
            };
            let joined = path.join(".");
            if !owner.fields.contains_key(&joined) && !owner.collections.contains_key(&joined) {
                add(problems, Problem::UnknownField(joined));
            }
        }
        Criteria::Parameter(name) => {
            if !schema.parameters.contains_key(name)
                && !parent.is_some_and(|p| p.parameters.contains_key(name))
            {
                add(problems, Problem::UnknownParameter(name.clone()));
            }
        }
        Criteria::Custom(name, args) => {
            if !schema.functions.contains(&name.to_lowercase())
                && !parent.is_some_and(|p| p.functions.contains(&name.to_lowercase()))
            {
                add(problems, Problem::UnknownFunction(name.clone()));
            }
            args.iter().for_each(|a| check(a, schema, parent, problems));
        }
        Criteria::Aggregate(call) => match &call.collection {
            Some(path) => {
                let joined = path.join(".");
                match schema.collections.get(&joined) {
                    Some(rows) => {
                        let inner = call.condition.iter().chain(&call.args);
                        inner.for_each(|a| check(a, rows, Some(schema), problems));
                    }
                    None => add(problems, Problem::UnknownCollection(joined)),
                }
            }
            None => call.condition.iter().chain(&call.args).for_each(|a| check(a, schema, parent, problems)),
        },
        Criteria::Value(_) => {}
        Criteria::Unary(_, a) | Criteria::IsNull(a) => check(a, schema, parent, problems),
        Criteria::Binary(_, a, b) => {
            check(a, schema, parent, problems);
            check(b, schema, parent, problems);
        }
        Criteria::Group(_, items) | Criteria::Function(_, items) => {
            items.iter().for_each(|a| check(a, schema, parent, problems));
        }
        Criteria::In(a, items) => {
            check(a, schema, parent, problems);
            items.iter().for_each(|i| check(i, schema, parent, problems));
        }
        Criteria::Between(a, b, c) => {
            check(a, schema, parent, problems);
            check(b, schema, parent, problems);
            check(c, schema, parent, problems);
        }
    }
}
