//! What a record has to meet before it is kept.

use std::cmp::Ordering;
use std::fmt;

use ravenrs_app_i18n::{Language, Texts};
use ravenrs_criteria::{Criteria, Environment, Scope, Value};

pub use crate::entity::rules::{Comparison, Operand, Rule, RuleKind, Severity, TextComparison};
use crate::{EntityInfo, ModelError, Object, ObjectSpace, Query};

/// The context of keeping a record. Rules are in it unless they say otherwise.
pub const CONTEXT_SAVE: &str = "Save";
/// The context of taking a record away.
pub const CONTEXT_DELETE: &str = "Delete";

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Severity {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "Error" => Some(Self::Error),
            "Warning" => Some(Self::Warning),
            "Information" => Some(Self::Information),
            _ => None,
        }
    }
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

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Rule {
    /// A rule set in the model, under an entity's `Rules`: `Kind` is `Required`, `Range`, `Matches`,
    /// `Criteria` or `Unique`, with `Field`, `Min`, `Max`, `Pattern`, `Expression`, `Message`,
    /// `Target` and `Contexts`, separated by commas, as each asks for.
    pub(crate) fn from_model(node: &crate::ModelNode) -> Result<Self, crate::ModelError> {
        let wrong = |what: &str| crate::ModelError(format!("the rule {} {what}", node.id));
        let text = |name: &str| node.text(name).map(str::to_owned);
        let field = || text("Field").ok_or_else(|| wrong("says nothing about its field"));
        let kind = node.text("Kind").ok_or_else(|| wrong("says nothing about its kind"))?;
        let mut rule = match kind {
            "Required" => Self::on(field()?, RuleKind::Required),
            "Unique" => Self::on(field()?, RuleKind::Unique),
            "Range" => {
                Self::on(field()?, RuleKind::Range { min: node.number("Min"), max: node.number("Max") })
            }
            "Matches" => {
                let pattern = text("Pattern").ok_or_else(|| wrong("says nothing about its pattern"))?;
                Self::on(field()?, RuleKind::Matches(pattern))
            }
            "Criteria" => {
                let expression = text("Expression").ok_or_else(|| wrong("has no expression"))?;
                Self::new(node.id.clone(), RuleKind::Criteria(expression))
            }
            other => return Err(wrong(&format!("is of an unknown kind: {other}"))),
        };
        rule.id = node.id.clone();
        rule.message = text("Message");
        rule.target_criteria = text("Target");
        if let Some(name) = node.text("Severity") {
            rule.severity = Severity::from_name(name)
                .ok_or_else(|| wrong(&format!("has an unknown severity: {name}")))?;
        }
        if let Some(contexts) = node.text("Contexts") {
            rule.contexts = contexts.split(',').map(|context| context.trim().to_owned()).collect();
        }
        Ok(rule)
    }

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
            severity: Severity::Error,
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

    /// Says this when it fails. `{field}` is what a field of the record holds, as it shows, and
    /// `{reference.field}` the same of the record a reference points to. `{$field}` is what the
    /// rule's field is called, `{$entity}` the entity, `{$min}` and `{$max}` the limits.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// How much not meeting it matters.
    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// People are told, and can go on anyway.
    pub fn warning(self) -> Self {
        self.severity(Severity::Warning)
    }

    /// It goes on, with a note on the field.
    pub fn information(self) -> Self {
        self.severity(Severity::Information)
    }

    /// Whether finding out takes reading other records.
    fn reads_records(&self) -> bool {
        matches!(
            self.kind,
            RuleKind::Unique
                | RuleKind::UniqueTogether(_)
                | RuleKind::Exists { .. }
                | RuleKind::Referenced { .. }
                | RuleKind::Custom(_)
        )
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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
    /// Before decimals, where a message shows money.
    pub decimal: char,
}

impl Default for RuleWords {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RuleWords {
    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            required: texts.get("rules.required"),
            too_low: texts.get("rules.too_low"),
            too_high: texts.get("rules.too_high"),
            invalid: texts.get("rules.invalid"),
            comparison: texts.get("rules.comparison"),
            record: texts.get("rules.record"),
            must_be_true: texts.get("rules.must_be_true"),
            duplicate: texts.get("rules.duplicate"),
            none_exists: texts.get("rules.none_exists"),
            not_referenced: texts.get("rules.not_referenced"),
            decimal: texts.char("format.decimal", '.'),
        }
    }

    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }
}

/// A rule that was not met.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Violation {
    /// The rule.
    pub rule: String,
    /// The field it is about.
    pub field: Option<String>,
    /// In words for people.
    pub message: String,
    /// How much it matters.
    pub severity: Severity,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Violation {
    /// Whether it stops what was being done.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
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

/// What a value is, as a message says it.
fn shown(field: Option<&crate::FieldInfo>, value: &Value, words: &RuleWords) -> String {
    match (field.map(|field| &field.kind), value) {
        (Some(crate::FieldKind::Money), Value::Number(cents)) => {
            format!("{:.2}", cents / 100.0).replace('.', &words.decimal.to_string())
        }
        (Some(crate::FieldKind::Enum(options)), Value::Text(code)) => options
            .iter()
            .find(|option| option.code == *code)
            .map_or_else(|| code.clone(), |option| option.caption.clone()),
        (_, Value::Date(moment)) => moment.to_string(),
        _ => text_of(value),
    }
}

/// What `{name}` in a message stands for.
fn message_part(
    name: &str,
    rule: &Rule,
    entity: &EntityInfo,
    object: &Object,
    space: &dyn ObjectSpace,
    words: &RuleWords,
) -> Option<String> {
    let number = |limit: Option<f64>| limit.map_or_else(String::new, |n| text_of(&Value::Number(n)));
    match name {
        "$entity" => return Some(entity.caption.clone()),
        "$field" => {
            let field = rule.field.as_deref().and_then(|field| entity.field_named(field));
            return Some(field.map_or_else(String::new, |field| field.caption.clone()));
        }
        "$min" | "$max" => {
            let RuleKind::Range { min, max } = &rule.kind else { return Some(String::new()) };
            return Some(number(if name == "$min" { *min } else { *max }));
        }
        _ => {}
    }
    if let Some((reference, rest)) = name.split_once('.') {
        let field = entity.field_named(reference)?;
        let crate::FieldKind::Reference(target) = &field.kind else { return None };
        let other = space.entities().into_iter().find(|e| e.name == *target)?;
        let record = space.find(target, &object.get(reference)).ok().flatten();
        let Some(record) = record else { return Some(String::new()) };
        return message_part(rest, rule, &other, &record, space, words);
    }
    let field = entity.field_named(name)?;
    Some(shown(Some(field), &object.get(name), words))
}

/// A rule's message with what it refers to filled in. What it does not know stays as written.
fn filled_message(
    text: &str,
    rule: &Rule,
    entity: &EntityInfo,
    object: &Object,
    space: &dyn ObjectSpace,
    words: &RuleWords,
) -> String {
    let mut filled = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        filled.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            rest = &rest[start..];
            break;
        };
        let inside = &rest[start + 1..start + end];
        match message_part(inside, rule, entity, object, space, words) {
            Some(part) => filled.push_str(&part),
            None => filled.push_str(&rest[start..=start + end]),
        }
        rest = &rest[start + end + 1..];
    }
    filled.push_str(rest);
    filled
}

/// What a rule says, or nothing if it is met.
fn broken(
    rule: &Rule,
    entity: &EntityInfo,
    object: &Object,
    space: &dyn ObjectSpace,
    words: &RuleWords,
) -> Option<String> {
    let environment = crate::environment();
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
    failed.map(|text| match &rule.message {
        Some(message) => filled_message(message, rule, entity, object, space, words),
        None => text,
    })
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
    checked(entity, object, context, space, words, false)
}

/// The same, leaving out rules that have to read other records, for checking every row of a list.
pub fn validate_in_place(
    entity: &EntityInfo,
    object: &Object,
    context: &str,
    space: &dyn ObjectSpace,
    words: &RuleWords,
) -> Vec<Violation> {
    checked(entity, object, context, space, words, true)
}

fn checked(
    entity: &EntityInfo,
    object: &Object,
    context: &str,
    space: &dyn ObjectSpace,
    words: &RuleWords,
    in_place: bool,
) -> Vec<Violation> {
    let mut object = object.clone();
    let environment = crate::environment();
    entity.compute(&mut object, &environment);
    entity
        .rules
        .iter()
        .filter(|rule| rule.contexts.iter().any(|c| c == context))
        .filter(|rule| !(in_place && rule.reads_records()))
        .filter(|rule| {
            rule.target_criteria.as_deref().is_none_or(|expression| meets(&object, expression, &environment))
        })
        .filter_map(|rule| {
            broken(rule, entity, &object, space, words).map(|message| Violation {
                rule: rule.id.clone(),
                field: rule.field.clone(),
                message,
                severity: rule.severity,
            })
        })
        .collect()
}
