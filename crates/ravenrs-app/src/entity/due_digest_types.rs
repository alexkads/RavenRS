//! How an entity says it has a due date to sum up (#860): data only. The code that works out the
//! summary lives in `due_digest` and imports it from here, so it can stand on the entity.

use ravenrs_criteria::Criteria;

use crate::{EntityInfo, FieldKind};

/// How a due date of an entity is summed up: the date, what is pending, what names a record, the
/// amount, and the words of the text.
///
/// ```
/// use ravenrs_app::{DueDigest, EntityInfo, FieldInfo, FieldKind};
///
/// let invoice = EntityInfo::new("Invoice")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .field(FieldInfo::new("description", FieldKind::Text))
///     .field(FieldInfo::new("due_on", FieldKind::Date))
///     .field(FieldInfo::new("amount", FieldKind::Money))
///     .field(FieldInfo::new("status", FieldKind::Text))
///     .due_digest(
///         DueDigest::new("due_on")
///             .pending("[status] = 'open'")
///             .label("description")
///             .amount("amount")
///             .words("conta", "contas")
///             .named(3),
///     );
/// assert!(invoice.check().is_ok());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DueDigest {
    /// The date, or date and time, it is due.
    pub field: String,
    /// What a record meets while it is still pending; nothing means every one.
    pub pending: Option<String>,
    /// What names a record; nothing means what names the entity.
    pub label: Option<String>,
    /// A money field to add up, if any.
    pub amount: Option<String>,
    /// The word for one record and for several.
    pub words: (String, String),
    /// How many it names; the rest it counts.
    pub named: usize,
    /// What the text says, instead of the amounts, to whom may not read them.
    pub hidden_amounts: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DueDigest {
    /// Due on this date or date and time field.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            pending: None,
            label: None,
            amount: None,
            words: ("item".to_owned(), "items".to_owned()),
            named: 3,
            hidden_amounts: None,
        }
    }

    /// Only records that meet this are pending.
    pub fn pending(mut self, expression: impl Into<String>) -> Self {
        self.pending = Some(expression.into());
        self
    }

    /// This field names a record in the text.
    pub fn label(mut self, field: impl Into<String>) -> Self {
        self.label = Some(field.into());
        self
    }

    /// This money field is shown and added up, to whom may read it.
    pub fn amount(mut self, field: impl Into<String>) -> Self {
        self.amount = Some(field.into());
        self
    }

    /// The word for one record and for several, as "bill" and "bills".
    pub fn words(mut self, one: impl Into<String>, many: impl Into<String>) -> Self {
        self.words = (one.into(), many.into());
        self
    }

    /// Names this many records; the rest it counts.
    pub fn named(mut self, how_many: usize) -> Self {
        self.named = how_many;
        self
    }

    /// Says this in the text when the amounts are hidden from the person.
    pub fn hidden_amounts(mut self, text: impl Into<String>) -> Self {
        self.hidden_amounts = Some(text.into());
        self
    }

    /// What is wrong with it on this entity.
    pub(crate) fn problem(&self, entity: &EntityInfo) -> Option<String> {
        match entity.field_named(&self.field).map(|field| &field.kind) {
            Some(FieldKind::Date | FieldKind::DateTime) => {}
            Some(_) => return Some(format!("its due date {} is not a date", self.field)),
            None => return Some(format!("its due date {} is not a field", self.field)),
        }
        if let Some(field) = &self.label
            && entity.field_named(field).is_none()
        {
            return Some(format!("its label {field} is not a field"));
        }
        if let Some(field) = &self.amount
            && !entity.field_named(field).is_some_and(|field| matches!(field.kind, FieldKind::Money))
        {
            return Some(format!("its amount {field} is not a money field"));
        }
        if let Some(expression) = &self.pending
            && let Err(e) = Criteria::parse(expression)
        {
            return Some(format!("what is pending does not read: {e}"));
        }
        None
    }
}
