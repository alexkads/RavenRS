//! What a validation rule says: the field, the kind of check, when it applies and how serious it is.
//! The entity keeps them; the code that runs them lives in its own module.

use std::rc::Rc;

use ravenrs_criteria::Value;

use crate::{Object, ObjectSpace};

/// How a [`RuleKind::Text`] compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum TextComparison {
    /// The field holds exactly the text.
    Equals,
    /// The field holds something else.
    NotEquals,
    /// The text is somewhere in the field.
    Contains,
    /// The field starts with the text.
    StartsWith,
    /// The field ends with the text.
    EndsWith,
}

/// How a [`RuleKind::Compare`] compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Comparison {
    /// The same.
    Equal,
    /// Not the same.
    NotEqual,
    /// Below.
    Less,
    /// Below or the same.
    LessOrEqual,
    /// Above.
    Greater,
    /// Above or the same.
    GreaterOrEqual,
}

/// What a [`RuleKind::Compare`] compares with.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Operand {
    /// This value.
    Value(Value),
    /// What this other field holds.
    Field(String),
}

pub(crate) type CustomRule = Rc<dyn Fn(&Object, &dyn ObjectSpace) -> Result<(), String>>;

/// How much not meeting a rule matters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Severity {
    /// It stops what was being done until it is fixed.
    #[default]
    Error,
    /// People are told, and can go on anyway.
    Warning,
    /// It goes on, with a note on the field.
    Information,
}

/// What a [`Rule`] asks for.
#[derive(Clone)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum RuleKind {
    /// The field has something in it.
    Required,
    /// The field is within these limits.
    Range {
        /// The lowest it can be.
        min: Option<f64>,
        /// The highest it can be.
        max: Option<f64>,
    },
    /// The field matches this regular expression.
    Matches(String),
    /// The field compares with this text.
    Text {
        /// How.
        comparison: TextComparison,
        /// With what.
        text: String,
        /// Whether case counts for nothing.
        ignore_case: bool,
    },
    /// The field compares with a value or another field.
    Compare {
        /// How.
        comparison: Comparison,
        /// With what.
        with: Operand,
    },
    /// The record meets this expression.
    Criteria(String),
    /// The field is true, as with an agreement.
    IsTrue,
    /// No other record holds the same in the field.
    Unique,
    /// No other record holds the same in all these fields together.
    UniqueTogether(Vec<String>),
    /// Some record of this entity meets this expression.
    Exists {
        /// The entity.
        entity: String,
        /// The expression, or none for any record at all.
        criteria: Option<String>,
    },
    /// Some record of this entity points to this one through this field.
    Referenced {
        /// The entity.
        entity: String,
        /// Its field that holds this record's key.
        field: String,
    },
    /// Anything else.
    Custom(CustomRule),
}

/// One thing a record has to meet, in some contexts, and perhaps only when it fits.
#[derive(Clone, Debug)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Rule {
    /// What it is known by.
    pub id: String,
    /// The field it is about, where the error shows.
    pub field: Option<String>,
    /// What it asks for.
    pub kind: RuleKind,
    /// When it applies.
    pub contexts: Vec<String>,
    /// It applies only to records that meet this.
    pub target_criteria: Option<String>,
    /// What to say, instead of the usual words.
    pub message: Option<String>,
    /// Whether an empty field passes, as it does for all but [`RuleKind::Required`].
    pub skip_empty: bool,
    /// How much not meeting it matters.
    pub severity: Severity,
}
