//! The values of a build's parameters, as the pipeline declares them (V-03 of #803).
//!
//! A build asked with no value for a parameter gets its `default`, as Jenkins does: the steps
//! read `SIMULAR=true`, not an empty variable that a script would take for "no". A value that
//! came as text from a command line or a form (`--param SIMULAR=true`, `--param VEZES=3`) is read
//! as the kind the parameter says, so `raven-ci rodar` does not have to know the pipeline.

use ravenrs_app::{FieldInfo, FieldKind, Value as Field};
use serde_json::{Map, Value};

/// `given`, with the default of each parameter it lacks and each text read as its parameter's kind.
/// What does not fit stays as it came, for the check that follows to refuse with its reason.
pub(crate) fn complete(fields: &[FieldInfo], given: &Map<String, Value>) -> Map<String, Value> {
    let mut out = given.clone();
    for field in fields {
        match out.get(&field.name) {
            None | Some(Value::Null) => {
                if let Some(default) = json_of(&field.default_value, &field.kind) {
                    out.insert(field.name.clone(), default);
                }
            }
            Some(Value::String(text)) => {
                if let Some(read) = read_text(text, &field.kind) {
                    out.insert(field.name.clone(), read);
                }
            }
            Some(_) => {}
        }
    }
    out
}

/// The default of a field as JSON, of the field's kind; `None` when there is none.
fn json_of(value: &Field, kind: &FieldKind) -> Option<Value> {
    match value {
        Field::Null => None,
        Field::Bool(b) => Some(Value::Bool(*b)),
        Field::Number(n) if matches!(kind, FieldKind::Integer) && n.fract() == 0.0 => {
            // A whole number of a field of whole numbers: as an integer, which the check wants.
            #[allow(clippy::cast_possible_truncation, reason = "a whole f64 that fits")]
            Some(Value::from(*n as i64))
        }
        Field::Number(n) => serde_json::Number::from_f64(*n).map(Value::Number),
        Field::Text(text) => read_text(text, kind).or_else(|| Some(Value::String(text.clone()))),
        Field::Date(date) => Some(Value::String(date.to_string())),
        _ => None,
    }
}

/// Text read as `kind`: `true`/`false` (and `1`/`0`, `sim`/`não`) for yes-or-no, a number for the
/// kinds of numbers. `None` when the kind takes text, or the text does not read as it.
fn read_text(text: &str, kind: &FieldKind) -> Option<Value> {
    let text = text.trim();
    match kind {
        FieldKind::Boolean => match text.to_lowercase().as_str() {
            "true" | "1" | "sim" | "yes" => Some(Value::Bool(true)),
            "false" | "0" | "não" | "nao" | "no" => Some(Value::Bool(false)),
            _ => None,
        },
        FieldKind::Integer => text.parse::<i64>().ok().map(Value::from),
        FieldKind::Decimal(_) | FieldKind::Money => {
            text.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn fields() -> Vec<FieldInfo> {
        vec![
            FieldInfo::new("SIMULAR", FieldKind::Boolean).default_value(true),
            FieldInfo::new("DIAS", FieldKind::Integer).default_value(3_i64),
            FieldInfo::new("BRANCH", FieldKind::Text).default_value("develop"),
            FieldInfo::new("SEM", FieldKind::Text),
        ]
    }

    #[test]
    fn a_parameter_not_given_gets_its_default_of_its_kind() {
        let out = complete(&fields(), &Map::new());
        assert_eq!(Value::Object(out), json!({ "SIMULAR": true, "DIAS": 3, "BRANCH": "develop" }));
    }

    #[test]
    fn text_of_a_command_line_is_read_as_the_kind_and_what_does_not_fit_stays() {
        let given = json!({ "SIMULAR": "false", "DIAS": "7", "BRANCH": "feat/x" });
        let out = complete(&fields(), given.as_object().expect("an object"));
        assert_eq!(out["SIMULAR"], json!(false));
        assert_eq!(out["DIAS"], json!(7));
        assert_eq!(out["BRANCH"], json!("feat/x"));
        let odd = json!({ "SIMULAR": "talvez" });
        let out = complete(&fields(), odd.as_object().expect("an object"));
        assert_eq!(out["SIMULAR"], json!("talvez"), "left for the check to refuse");
    }
}
