//! Records as JSON, by what the model says each field holds.

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Object, Value, value_to_json as to_json};
use ravenrs_criteria::DateTime;
use serde_json::{Map, Value as Json};

use crate::Failure;

/// The largest whole number a record holds exactly. Past this, one value would quietly become another.
const MOST_EXACT: u64 = 1 << 53;

/// A whole number, if a record can hold it as it is.
fn exact(number: &serde_json::Number) -> Option<f64> {
    if let Some(whole) = number.as_i64() {
        return (whole.unsigned_abs() <= MOST_EXACT).then_some(whole as f64);
    }
    if let Some(whole) = number.as_u64() {
        return (whole <= MOST_EXACT).then_some(whole as f64);
    }
    number.as_f64().filter(|value| value.fract() == 0.0 && value.abs() <= MOST_EXACT as f64)
}

/// Whether a field can be read or written over the API at all.
pub(crate) fn exposed(field: &FieldInfo) -> bool {
    !field.secret && !matches!(field.kind, FieldKind::Collection(_))
}

/// A record as JSON, with only what can leave.
pub(crate) fn object_to_json(entity: &EntityInfo, object: &Object) -> Map<String, Json> {
    entity
        .fields
        .iter()
        .filter(|field| exposed(field))
        .map(|field| (field.name.clone(), to_json(&object.get(&field.name))))
        .collect()
}

/// What was sent for a field, as what the field holds.
pub(crate) fn from_json(field: &FieldInfo, sent: &Json) -> Result<Value, Failure> {
    let wrong = || Failure::bad_request(format!("{} does not take this value", field.name));
    if sent.is_null() {
        return Ok(Value::Null);
    }
    match &field.kind {
        FieldKind::Integer | FieldKind::Money => {
            sent.as_number().and_then(exact).map(Value::Number).ok_or_else(wrong)
        }
        FieldKind::Decimal(_) => sent.as_f64().map(Value::Number).ok_or_else(wrong),
        FieldKind::Boolean => sent.as_bool().map(Value::Bool).ok_or_else(wrong),
        FieldKind::Date | FieldKind::DateTime => {
            sent.as_str().and_then(DateTime::parse).map(Value::Date).ok_or_else(wrong)
        }
        FieldKind::Enum(options) => {
            let code = sent.as_str().ok_or_else(wrong)?;
            options
                .iter()
                .find(|option| option.code == code)
                .map(|_| Value::Text(code.to_owned()))
                .ok_or_else(wrong)
        }
        // A reference holds the key of the other record, a number or text as that key is.
        FieldKind::Reference(_) => match sent {
            Json::Number(number) => exact(number).map(Value::Number).ok_or_else(wrong),
            Json::String(text) => Ok(Value::Text(text.clone())),
            _ => Err(wrong()),
        },
        FieldKind::Collection(_) => Err(wrong()),
        _ => sent.as_str().map(|text| Value::Text(text.to_owned())).ok_or_else(wrong),
    }
}

/// A key as it came in a path.
pub(crate) fn key_from_path(entity: &EntityInfo, text: &str) -> Result<Value, Failure> {
    let field =
        entity.key_field().ok_or_else(|| Failure::bad_request(format!("{} has no key", entity.name)))?;
    match field.kind {
        FieldKind::Integer | FieldKind::Decimal(_) | FieldKind::Money => {
            // What is not a number is no record: the same answer as a number nobody has.
            text.parse::<f64>().map(Value::Number).map_err(|_| Failure::not_found())
        }
        _ => Ok(Value::Text(text.to_owned())),
    }
}

/// Applies what was sent to a record. Anything that is not a field that can be written refuses the
/// whole request, so a mistaken or a hopeful field never goes unnoticed.
pub(crate) fn apply(
    entity: &EntityInfo,
    object: &mut Object,
    sent: &Map<String, Json>,
    creating: bool,
) -> Result<(), Failure> {
    for (name, value) in sent {
        // Nested records are taken apart by whoever called, and are not values of this one.
        let Some(field) = entity.field_named(name) else {
            return Err(Failure::bad_request(format!("{} has no field {name}", entity.name)));
        };
        if matches!(field.kind, FieldKind::Collection(_)) {
            continue;
        }
        // A state shows as read-only on a form, which has actions for it. Here it is how a state
        // changes, and storage refuses a way the machine does not have.
        let is_state = entity.state_machines.iter().any(|machine| machine.field == *name);
        // Whoever cannot see the state does not get to learn it from what refusing it would say.
        if is_state && !creating && object.get(name) == Value::Null {
            return Err(Failure::denied());
        }
        if field.secret || (field.read_only && !is_state) || field.calculated.is_some() {
            return Err(Failure::bad_request(format!("{name} cannot be written")));
        }
        if field.is_key && (!creating || matches!(field.kind, FieldKind::Integer)) {
            return Err(Failure::bad_request(format!("{name} is given by the server")));
        }
        object.set(name, from_json(field, value)?);
    }
    Ok(())
}
