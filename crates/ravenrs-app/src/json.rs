//! Values and records as JSON, the way the web API and its client agree on.

use serde_json::{Map, Value as Json};

use crate::{EntityInfo, FieldKind, Object, Value};

/// A value as JSON. A date goes as text.
pub fn value_to_json(value: &Value) -> Json {
    match value {
        Value::Bool(on) => Json::Bool(*on),
        Value::Number(number) if number.fract() == 0.0 && number.abs() < 9e15 => Json::from(*number as i64),
        Value::Number(number) => serde_json::Number::from_f64(*number).map_or(Json::Null, Json::Number),
        Value::Text(text) => Json::String(text.clone()),
        Value::Date(date) => Json::String(date.to_string()),
        _ => Json::Null,
    }
}

/// A record from JSON that came from the API, taking what its fields hold and leaving out the rest.
pub fn object_from_json(entity: &EntityInfo, sent: &Map<String, Json>) -> Object {
    let mut object = Object::from_values(&entity.name, std::collections::BTreeMap::new());
    for field in &entity.fields {
        let Some(value) = sent.get(&field.name) else { continue };
        let value = match (value, &field.kind) {
            (Json::Bool(on), _) => Value::Bool(*on),
            (Json::Number(number), _) => number.as_f64().map_or(Value::Null, Value::Number),
            (Json::String(text), FieldKind::Date | FieldKind::DateTime) => {
                ravenrs_criteria::DateTime::parse(text).map_or_else(|| Value::Text(text.clone()), Value::Date)
            }
            (Json::String(text), _) => Value::Text(text.clone()),
            _ => Value::Null,
        };
        object.set(&field.name, value);
    }
    object
}
