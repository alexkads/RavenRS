//! Records as Rust types.

use ravenrs_criteria::{DateTime, Value};

use crate::{EntityInfo, FieldKind, Object, ObjectSpace, Query, SpaceError};

/// A Rust type a field can have.
pub trait FieldValue: Sized {
    /// What it holds, unless the field's attribute says otherwise.
    fn kind() -> FieldKind;
    /// As a record holds it.
    fn to_value(&self) -> Value;
    /// From what a record holds. What cannot be read gives the type's empty value.
    fn from_value(value: Value) -> Self;
}

impl FieldValue for String {
    fn kind() -> FieldKind {
        FieldKind::Text
    }
    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }
    fn from_value(value: Value) -> Self {
        if let Value::Text(text) = value { text } else { String::new() }
    }
}

impl FieldValue for bool {
    fn kind() -> FieldKind {
        FieldKind::Boolean
    }
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_value(value: Value) -> Self {
        matches!(value, Value::Bool(true))
    }
}

macro_rules! whole_numbers {
    ($($ty:ty),*) => {$(
        impl FieldValue for $ty {
            fn kind() -> FieldKind {
                FieldKind::Integer
            }
            fn to_value(&self) -> Value {
                Value::Number(*self as f64)
            }
            fn from_value(value: Value) -> Self {
                if let Value::Number(number) = value { number.round() as $ty } else { 0 }
            }
        }
    )*};
}

whole_numbers!(i32, i64, u32, u64, usize);

/// The decimal places a record's number keeps exactly: a number as records hold it has about 15
/// digits to spare, so a value with up to 9 before the comma comes back the same.
const DECIMAL_PLACES: u32 = 6;

/// A decimal, for calculations that cannot lose a cent. Records keep numbers as they always do; this
/// is what comes in and out of them, rounded to the places they keep exactly.
impl FieldValue for rust_decimal::Decimal {
    fn kind() -> FieldKind {
        FieldKind::Decimal(DECIMAL_PLACES as u8)
    }
    fn to_value(&self) -> Value {
        use rust_decimal::prelude::ToPrimitive;
        Value::Number(self.to_f64().unwrap_or(0.0))
    }
    fn from_value(value: Value) -> Self {
        let Value::Number(number) = value else { return Self::ZERO };
        Self::from_f64_retain(number).map_or(Self::ZERO, |exact| exact.round_dp(DECIMAL_PLACES).normalize())
    }
}

/// The most significant digits a record's number holds without changing: what has more is refused
/// rather than rounded where nobody sees it.
pub const EXACT_DIGITS: u32 = 15;

/// A number on its way to a `Decimal` field kept by a database: rounded to its places, as
/// [`rust_decimal::Decimal`] rounds (half to even), starting from the shortest text that reads
/// back as this number, which is what the person typed. Refused, with the reason, when it is not
/// finite or has more than [`EXACT_DIGITS`] significant digits after rounding.
pub fn exact_decimal(number: f64, places: u8) -> Result<f64, String> {
    if !number.is_finite() {
        return Err(format!("{number} is not a number a database can keep"));
    }
    let text = number.to_string();
    let exact: rust_decimal::Decimal =
        text.parse().map_err(|_| format!("{text} has more digits than a decimal can keep"))?;
    let rounded = exact.round_dp(u32::from(places)).normalize();
    // Normalized, the digits of the mantissa are the significant ones, and the whole part of a
    // round number such as 1200 counts in full: it has to fit the column too.
    let digits = rounded.mantissa().unsigned_abs().to_string().len();
    if digits > EXACT_DIGITS as usize {
        return Err(format!(
            "{text} has more than {EXACT_DIGITS} significant digits, which a number keeps exactly"
        ));
    }
    rounded.to_string().parse().map_err(|_| format!("{text} could not be kept"))
}

/// Rounds the `Decimal` fields of a record to their places before it is written, and refuses one
/// a number cannot keep exactly, rather than letting the database round it unseen.
pub fn exact_decimals(entity: &EntityInfo, object: &mut Object) -> Result<(), SpaceError> {
    for field in &entity.fields {
        let FieldKind::Decimal(places) = field.kind else { continue };
        if let Value::Number(number) = object.get(&field.name) {
            let kept = exact_decimal(number, places)
                .map_err(|why| SpaceError::Refused(format!("{}: {why}", field.name)))?;
            object.set(&field.name, Value::Number(kept));
        }
    }
    Ok(())
}

impl FieldValue for f64 {
    fn kind() -> FieldKind {
        FieldKind::Decimal(2)
    }
    fn to_value(&self) -> Value {
        Value::Number(*self)
    }
    fn from_value(value: Value) -> Self {
        if let Value::Number(number) = value { number } else { 0.0 }
    }
}

impl FieldValue for DateTime {
    fn kind() -> FieldKind {
        FieldKind::DateTime
    }
    fn to_value(&self) -> Value {
        Value::Date(*self)
    }
    fn from_value(value: Value) -> Self {
        if let Value::Date(moment) = value { moment } else { DateTime { days: 0, seconds: 0 } }
    }
}

impl<T: FieldValue> FieldValue for Option<T> {
    fn kind() -> FieldKind {
        T::kind()
    }
    fn to_value(&self) -> Value {
        self.as_ref().map_or(Value::Null, T::to_value)
    }
    fn from_value(value: Value) -> Self {
        if matches!(value, Value::Null) { None } else { Some(T::from_value(value)) }
    }
}

/// A record as a Rust struct. Derive it:
///
/// ```
/// use ravenrs_app::{BusinessEnum, BusinessObject};
///
/// #[derive(BusinessEnum, Clone, Debug, PartialEq)]
/// enum Kind {
///     #[option(caption = "Corrente")]
///     Checking,
///     #[option(caption = "Poupança")]
///     Savings,
/// }
///
/// #[derive(BusinessObject, Clone, Debug, PartialEq)]
/// #[object(caption = "Contas", navigation = "Cadastros", icon = "wallet")]
/// struct Account {
///     #[field(key)]
///     id: i64,
///     #[field(caption = "Nome", default_property)]
///     name: String,
///     #[field(caption = "Tipo")]
///     kind: Kind,
///     #[field(caption = "Saldo inicial", money)]
///     opening: i64,
/// }
///
/// let entity = Account::entity_info();
/// assert_eq!(entity.caption, "Contas");
/// assert!(entity.check().is_ok());
/// let account = Account { id: 1, name: "Carteira".into(), kind: Kind::Savings, opening: 1250 };
/// assert_eq!(Account::from_object(&account.to_object()), account);
/// ```
pub trait BusinessObject: Sized {
    /// What the record is.
    fn entity_info() -> EntityInfo;
    /// As a dynamic record.
    fn to_object(&self) -> Object;
    /// From a dynamic record.
    fn from_object(object: &Object) -> Self;
}

/// Reading and writing with Rust structs, on any object space.
pub trait ObjectSpaceExt {
    /// Marks this as a new record. A numbered key left at zero gets the next number.
    fn add<T: BusinessObject>(&mut self, record: &T) -> Result<(), SpaceError>;
    /// Reads records.
    fn read<T: BusinessObject>(&self, query: &Query) -> Result<Vec<T>, SpaceError>;
    /// Adds an event to what the next commit keeps, so that the change and its event are kept
    /// together or not at all. Its organization keeps one event per key: emitting the same key again
    /// adds nothing. The space needs the entity of [`outbox_entity`](crate::outbox_entity).
    fn emit(&mut self, event: &crate::Event) -> Result<crate::Emitted, SpaceError>;
}

impl<S: ObjectSpace + ?Sized> ObjectSpaceExt for S {
    fn emit(&mut self, event: &crate::Event) -> Result<crate::Emitted, SpaceError> {
        crate::outbox::emit(self, event)
    }

    fn add<T: BusinessObject>(&mut self, record: &T) -> Result<(), SpaceError> {
        let entity = T::entity_info();
        let mut object = self.new_object(&entity.name)?;
        let given = record.to_object();
        for (name, value) in given.values() {
            let numbered_key =
                entity.field_named(name).is_some_and(|f| f.is_key && matches!(f.kind, FieldKind::Integer))
                    && matches!(value, Value::Number(n) if *n == 0.0);
            if !numbered_key {
                object.set(name, value.clone());
            }
        }
        self.insert(object)
    }

    fn read<T: BusinessObject>(&self, query: &Query) -> Result<Vec<T>, SpaceError> {
        let found = self.query(&T::entity_info().name, query)?;
        Ok(found.iter().map(T::from_object).collect())
    }
}

#[cfg(test)]
mod decimal_tests {
    use rust_decimal::Decimal;

    use super::*;

    #[test]
    fn a_decimal_comes_back_exactly() {
        for text in ["0.1", "5.49", "16.47", "123456789.123456", "0.000001", "1234.5"] {
            let exact: Decimal = text.parse().unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(Decimal::from_value(exact.to_value()), exact.normalize(), "{text}");
        }
    }

    #[test]
    fn a_decimal_for_a_database_is_rounded_to_its_places() {
        assert_eq!(exact_decimal(0.1 + 0.2, 2), Ok(0.3));
        assert_eq!(exact_decimal(16.47, 2), Ok(16.47));
        assert_eq!(exact_decimal(2.675, 2), Ok(2.68));
        assert_eq!(exact_decimal(2.665, 2), Ok(2.66));
        assert_eq!(exact_decimal(123_456_789.123_456, 6), Ok(123_456_789.123_456));
        assert_eq!(exact_decimal(0.000_001, 6), Ok(0.000_001));
        assert_eq!(exact_decimal(-5.499, 2), Ok(-5.5));
    }

    #[test]
    fn a_decimal_with_more_digits_than_a_number_keeps_is_refused() {
        assert!(exact_decimal(1_234_567_890_123.456, 3).is_err());
        assert!(exact_decimal(f64::NAN, 2).is_err());
        assert!(exact_decimal(f64::INFINITY, 2).is_err());
        assert_eq!(exact_decimal(1_234_567_890_123.456, 2), Ok(1_234_567_890_123.46));
    }

    #[test]
    fn what_is_not_a_number_comes_as_zero() {
        assert_eq!(Decimal::from_value(Value::Text("x".into())), Decimal::ZERO);
    }
}
