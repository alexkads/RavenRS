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
}

impl<S: ObjectSpace + ?Sized> ObjectSpaceExt for S {
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
