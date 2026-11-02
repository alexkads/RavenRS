//! What a record has to meet before it is kept.

use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

use ravenrs_criteria::{Criteria, Environment, Scope, Value};

use crate::{EntityInfo, ModelError, Object, ObjectSpace, Query};

/// The context of keeping a record. Rules are in it unless they say otherwise.
pub const CONTEXT_SAVE: &str = "Save";
/// The context of taking a record away.
pub const CONTEXT_DELETE: &str = "Delete";

/// How a [`RuleKind::Text`] compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
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
pub enum Operand {
    /// This value.
    Value(Value),
    /// What this other field holds.
    Field(String),
}

type CustomRule = Rc<dyn Fn(&Object, &dyn ObjectSpace) -> Result<(), String>>;

/// What a [`Rule`] asks for.
#[derive(Clone)]
#[non_exhaustive]
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

impl fmt::Debug for RuleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Required => "Required",
            Self::Range { .. } => "Range",
            Self::Matches(_) => "Matches",
            Self::Text { .. } => "Text",
            Self::Compare { .. } => "Compare",
            Self::Criteria(_) => "Criteria",
            Self::IsTrue => "IsTrue",
            Self::Unique => "Unique",
            Self::UniqueTogether(_) => "UniqueTogether",
            Self::Exists { .. } => "Exists",
            Self::Referenced { .. } => "Referenced",
            Self::Custom(_) => "Custom",
        })
    }
}

/// One thing a record has to meet, in some contexts, and perhaps only when it fits.
#[derive(Clone, Debug)]
#[non_exhaustive]
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
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Rule {
    /// A rule about the whole record.
    pub fn new(id: impl Into<String>, kind: RuleKind) -> Self {
        Self {
            id: id.into(),
            field: None,
            kind,
            contexts: vec![CONTEXT_SAVE.to_owned()],
            target_criteria: None,
            message: None,
            skip_empty: true,
        }
    }

    /// A rule about a field.
    pub fn on(field: impl Into<String>, kind: RuleKind) -> Self {
        let field = field.into();
        let skip_empty = !matches!(kind, RuleKind::Required | RuleKind::IsTrue);
        Self { skip_empty, ..Self::new(format!("{field}.{kind:?}"), kind) }.about(field)
    }

    fn about(mut self, field: String) -> Self {
        self.field = Some(field);
        self
    }

    /// The field has to have something in it.
    pub fn required(field: impl Into<String>) -> Self {
        Self::on(field, RuleKind::Required)
    }

    /// No other record can hold the same in the field.
    pub fn unique(field: impl Into<String>) -> Self {
        Self::on(field, RuleKind::Unique)
    }

    /// The field has to be within these limits.
    pub fn range(field: impl Into<String>, min: Option<f64>, max: Option<f64>) -> Self {
        Self::on(field, RuleKind::Range { min, max })
    }

    /// The field has to match this regular expression.
    pub fn matches(field: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::on(field, RuleKind::Matches(pattern.into()))
    }

    /// The record has to meet this expression.
    pub fn criteria(id: impl Into<String>, expression: impl Into<String>) -> Self {
        Self::new(id, RuleKind::Criteria(expression.into()))
    }

    /// Applies in these contexts, instead of on saving.
    pub fn contexts(mut self, contexts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.contexts = contexts.into_iter().map(Into::into).collect();
        self
    }

    /// Applies only to records that meet this.
    pub fn target_criteria(mut self, expression: impl Into<String>) -> Self {
        self.target_criteria = Some(expression.into());
        self
    }

    /// Says this when it fails.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Whether an empty field passes.
    pub fn skip_empty(mut self, skip: bool) -> Self {
        self.skip_empty = skip;
        self
    }

    /// Says what is wrong with how it was declared.
    pub(crate) fn check(&self, entity: &EntityInfo) -> Result<(), ModelError> {
        let wrong = |what: String| Err(ModelError(format!("{}: the rule {}: {what}", entity.name, self.id)));
        if let Some(field) = &self.field
            && entity.field_named(field).is_none()
        {
            return wrong(format!("there is no field {field}"));
        }
        let parses = |text: &str| {
            Criteria::parse(text)
                .map(|_| ())
                .map_err(|e| ModelError(format!("{}: the rule {}: {e}", entity.name, self.id)))
        };
        if let Some(text) = &self.target_criteria {
            parses(text)?;
        }
        match &self.kind {
            RuleKind::Criteria(text) => parses(text)?,
            RuleKind::Exists { criteria: Some(text), .. } => parses(text)?,
            RuleKind::Matches(pattern) => {
                if let Err(e) = regex_lite::Regex::new(pattern) {
                    return wrong(e.to_string());
                }
            }
            RuleKind::UniqueTogether(fields) => {
                if let Some(missing) = fields.iter().find(|f| entity.field_named(f).is_none()) {
                    return wrong(format!("there is no field {missing}"));
                }
            }
            RuleKind::Compare { with: Operand::Field(other), .. } if entity.field_named(other).is_none() => {
                return wrong(format!("there is no field {other}"));
            }
            _ => {}
        }
        Ok(())
    }
}

/// The words of what fails, where `{0}` is the name of the field.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RuleWords {
    /// [`RuleKind::Required`].
    pub required: String,
    /// [`RuleKind::Range`], below the limit, where `{1}` is the limit.
    pub too_low: String,
    /// [`RuleKind::Range`], above the limit, where `{1}` is the limit.
    pub too_high: String,
    /// [`RuleKind::Matches`] and [`RuleKind::Text`].
    pub invalid: String,
    /// [`RuleKind::Compare`].
    pub comparison: String,
    /// [`RuleKind::Criteria`] and [`RuleKind::Custom`], with no field.
    pub record: String,
    /// [`RuleKind::IsTrue`].
    pub must_be_true: String,
    /// [`RuleKind::Unique`] and [`RuleKind::UniqueTogether`].
    pub duplicate: String,
    /// [`RuleKind::Exists`].
    pub none_exists: String,
    /// [`RuleKind::Referenced`].
    pub not_referenced: String,
}

impl Default for RuleWords {
    fn default() -> Self {
        Self {
            required: "{0} is required".into(),
            too_low: "{0} cannot be below {1}".into(),
            too_high: "{0} cannot be above {1}".into(),
            invalid: "{0} is not valid".into(),
            comparison: "{0} does not compare as it should".into(),
            record: "This record is not valid".into(),
            must_be_true: "{0} has to be checked".into(),
            duplicate: "There is already a record with this {0}".into(),
            none_exists: "A record it needs does not exist".into(),
            not_referenced: "Nothing points to this record yet".into(),
        }
    }
}

impl RuleWords {
    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            required: "{0} é obrigatório".into(),
            too_low: "{0} não pode ser menor que {1}".into(),
            too_high: "{0} não pode ser maior que {1}".into(),
            invalid: "{0} não é válido".into(),
            comparison: "{0} não está como deveria".into(),
            record: "Este registro não é válido".into(),
            must_be_true: "{0} precisa ser marcado".into(),
            duplicate: "Já existe um registro com este valor em {0}".into(),
            none_exists: "Um registro necessário não existe".into(),
            not_referenced: "Nada aponta para este registro ainda".into(),
        }
    }
}

/// A rule that was not met.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Violation {
    /// The rule.
    pub rule: String,
    /// The field it is about.
    pub field: Option<String>,
    /// In words for people.
    pub message: String,
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Text(text) => text.trim().is_empty(),
        _ => false,
    }
}

fn text_of(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::Number(number) if number.fract() == 0.0 => format!("{}", *number as i64),
        Value::Number(number) => number.to_string(),
        Value::Bool(on) => on.to_string(),
        Value::Null => String::new(),
        other => format!("{other:?}"),
    }
}

fn same(a: &Value, b: &Value) -> bool {
    text_of(a) == text_of(b)
}

fn compares(value: &Value, comparison: Comparison, other: &Value) -> bool {
    let order = value.compare(other);
    match comparison {
        Comparison::Equal => order == Ordering::Equal,
        Comparison::NotEqual => order != Ordering::Equal,
        Comparison::Less => order == Ordering::Less,
        Comparison::LessOrEqual => order != Ordering::Greater,
        Comparison::Greater => order == Ordering::Greater,
        Comparison::GreaterOrEqual => order != Ordering::Less,
    }
}

fn meets(object: &Object, expression: &str, environment: &Environment) -> bool {
    Criteria::parse(expression).is_ok_and(|parsed| Scope::new(environment).row(object).matches(&parsed))
}

/// Whether another record holds the same in all these fields. Only records storage already has
/// or this space is changing can be found, so this needs a space that reads.
fn duplicated(entity: &EntityInfo, object: &Object, fields: &[&str], space: &dyn ObjectSpace) -> bool {
    let key = entity.key_field().map(|f| f.name.as_str()).unwrap_or_default();
    let found = space.query(&entity.name, &Query::all()).unwrap_or_default();
    found.iter().any(|other| {
        !same(&other.get(key), &object.get(key))
            && fields.iter().all(|field| same(&other.get(field), &object.get(field)))
    })
}

/// What a rule says, or nothing if it is met.
fn broken(
    rule: &Rule,
    entity: &EntityInfo,
    object: &Object,
    space: &dyn ObjectSpace,
    words: &RuleWords,
) -> Option<String> {
    let environment = Environment::new();
    let caption = rule
        .field
        .as_deref()
        .and_then(|name| entity.field_named(name))
        .map_or_else(String::new, |field| field.caption.clone());
    let say = |text: &str, limit: Option<f64>| {
        let limit = limit.map_or_else(String::new, |n| text_of(&Value::Number(n)));
        text.replace("{0}", &caption).replace("{1}", &limit)
    };
    let value = rule.field.as_deref().map_or(Value::Null, |name| object.get(name));
    if rule.field.is_some() && rule.skip_empty && is_empty(&value) {
        return None;
    }
    let failed = match &rule.kind {
        RuleKind::Required => is_empty(&value).then(|| say(&words.required, None)),
        RuleKind::Range { min, max } => {
            let number = value.as_number()?;
            if min.is_some_and(|min| number < min) {
                Some(say(&words.too_low, *min))
            } else if max.is_some_and(|max| number > max) {
                Some(say(&words.too_high, *max))
            } else {
                None
            }
        }
        RuleKind::Matches(pattern) => {
            let fits = regex_lite::Regex::new(pattern).is_ok_and(|regex| regex.is_match(&text_of(&value)));
            (!fits).then(|| say(&words.invalid, None))
        }
        RuleKind::Text { comparison, text, ignore_case } => {
            let fold = |s: &str| if *ignore_case { s.to_lowercase() } else { s.to_owned() };
            let (held, wanted) = (fold(&text_of(&value)), fold(text));
            let fits = match comparison {
                TextComparison::NotEquals => held != wanted,
                TextComparison::Contains => held.contains(&wanted),
                TextComparison::StartsWith => held.starts_with(&wanted),
                TextComparison::EndsWith => held.ends_with(&wanted),
                _ => held == wanted,
            };
            (!fits).then(|| say(&words.invalid, None))
        }
        RuleKind::Compare { comparison, with } => {
            let other = match with {
                Operand::Value(other) => other.clone(),
                Operand::Field(name) => object.get(name),
            };
            (!compares(&value, *comparison, &other)).then(|| say(&words.comparison, None))
        }
        RuleKind::Criteria(expression) => {
            (!meets(object, expression, &environment)).then(|| words.record.clone())
        }
        RuleKind::IsTrue => (!matches!(value, Value::Bool(true))).then(|| say(&words.must_be_true, None)),
        RuleKind::Unique => {
            let field = rule.field.as_deref()?;
            duplicated(entity, object, &[field], space).then(|| say(&words.duplicate, None))
        }
        RuleKind::UniqueTogether(fields) => {
            let names: Vec<&str> = fields.iter().map(String::as_str).collect();
            let captions: Vec<&str> = fields
                .iter()
                .filter_map(|name| entity.field_named(name))
                .map(|field| field.caption.as_str())
                .collect();
            duplicated(entity, object, &names, space)
                .then(|| words.duplicate.replace("{0}", &captions.join(", ")))
        }
        RuleKind::Exists { entity: other, criteria } => {
            let found = space.query(other, &Query::all()).unwrap_or_default();
            let any = found.iter().any(|record| {
                criteria.as_deref().is_none_or(|expression| meets(record, expression, &environment))
            });
            (!any).then(|| words.none_exists.clone())
        }
        RuleKind::Referenced { entity: other, field } => {
            let key = entity.key_field().map(|f| object.get(&f.name)).unwrap_or_default();
            let found = space.query(other, &Query::all()).unwrap_or_default();
            (!found.iter().any(|record| same(&record.get(field), &key))).then(|| words.not_referenced.clone())
        }
        RuleKind::Custom(check) => check(object, space).err(),
    };
    failed.map(|text| rule.message.clone().unwrap_or(text))
}

/// Checks a record against the rules of its entity that apply in this context. It does not
/// run by itself: forms call it before keeping, and anything else that writes calls it itself.
pub fn validate(
    entity: &EntityInfo,
    object: &Object,
    context: &str,
    space: &dyn ObjectSpace,
    words: &RuleWords,
) -> Vec<Violation> {
    let mut object = object.clone();
    let environment = Environment::new();
    entity.compute(&mut object, &environment);
    entity
        .rules
        .iter()
        .filter(|rule| rule.contexts.iter().any(|c| c == context))
        .filter(|rule| {
            rule.target_criteria.as_deref().is_none_or(|expression| meets(&object, expression, &environment))
        })
        .filter_map(|rule| {
            broken(rule, entity, &object, space, words).map(|message| Violation {
                rule: rule.id.clone(),
                field: rule.field.clone(),
                message,
            })
        })
        .collect()
}
