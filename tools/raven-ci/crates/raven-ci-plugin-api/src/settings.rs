//! Checking settings against the entity that describes them: the configuration of a plugin, a
//! credential, the settings of a point in a job.

use ravenrs_app::{
    CONTEXT_SAVE, EntityInfo, FieldInfo, FieldKind, MemoryDatabase, RuleWords, Severity, object_from_json,
    validate_in_place,
};
use ravenrs_app_i18n::{Language, Texts};
use serde_json::Value;

use crate::CiError;

/// The key the record of the settings gets while its rules are checked.
const SETTINGS_KEY: &str = "$settings";

/// Checks `value` against `entity`: that it is an object, that each key is a field, that each
/// value has the field's kind (one of the options, for a choice), and the entity's rules
/// (required, ranges, patterns), as a form of `ravenrs-app` checks them. Every problem is
/// returned, each with its field, in Portuguese.
///
/// ```
/// use raven_ci_plugin_api::check_settings;
/// use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Rule};
/// use serde_json::json;
///
/// let github = EntityInfo::new("GithubSettings")
///     .field(FieldInfo::new("context", FieldKind::Text))
///     .field(FieldInfo::new("every_minutes", FieldKind::Integer))
///     .rule(Rule::required("context"));
/// assert!(check_settings(&github, &json!({ "context": "raven-ci", "every_minutes": 5 })).is_ok());
/// let wrong = check_settings(&github, &json!({ "every_minutes": "five", "colour": "red" })).unwrap_err();
/// assert_eq!(wrong.len(), 3);
/// ```
pub fn check_settings(entity: &EntityInfo, value: &Value) -> Result<(), Vec<CiError>> {
    let empty = serde_json::Map::new();
    let object = match value {
        Value::Object(object) => object,
        Value::Null => &empty,
        _ => {
            return Err(vec![CiError::settings(
                &entity.name,
                "tem que ser um objeto, com um valor por campo",
            )]);
        }
    };
    let mut problems = Vec::new();
    for (key, given) in object {
        let Some(field) = entity.fields.iter().find(|field| field.name == *key) else {
            problems.push(CiError::settings(key, "não existe nesta configuração"));
            continue;
        };
        if given.is_null() {
            continue;
        }
        let fits = match &field.kind {
            FieldKind::Integer => given.as_i64().is_some() || given.as_u64().is_some(),
            FieldKind::Decimal(_) | FieldKind::Money => given.is_number(),
            FieldKind::Boolean => given.is_boolean(),
            FieldKind::Enum(options) => {
                given.as_str().is_some_and(|code| options.iter().any(|o| o.code == code))
            }
            // Several of the list (#932): a list of its codes, each one of them.
            FieldKind::Tags(options) => given.as_array().is_some_and(|items| {
                items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|code| options.iter().any(|o| o.code == code)))
            }),
            FieldKind::Text
            | FieldKind::LongText
            | FieldKind::Color
            | FieldKind::Criteria
            | FieldKind::Date
            | FieldKind::DateTime => given.is_string(),
            _ => false,
        };
        if !fits {
            problems.push(CiError::settings(key, expected(&field.kind)));
        }
    }
    // Settings are one record with no key; the space the rules read needs one, so it gets a key
    // of its own that the settings never see.
    let mut keyed = entity.clone();
    if !keyed.fields.iter().any(|field| field.is_key) {
        keyed.fields.insert(0, FieldInfo::new(SETTINGS_KEY, FieldKind::Integer).key());
    }
    match MemoryDatabase::new(vec![keyed]) {
        Ok(database) => {
            let space = database.space();
            let words = RuleWords::from_texts(&Texts::new(Language::BrazilianPortuguese));
            let record = object_from_json(entity, object);
            for violation in validate_in_place(entity, &record, CONTEXT_SAVE, &space, &words) {
                if violation.severity == Severity::Error {
                    let field = violation.field.unwrap_or_else(|| entity.name.clone());
                    if !problems
                        .iter()
                        .any(|p| matches!(p, CiError::Settings { field: f, .. } if *f == field))
                    {
                        problems.push(CiError::settings(field, violation.message));
                    }
                }
            }
        }
        Err(wrong) => problems.push(CiError::settings(
            &entity.name,
            format!("a descrição da configuração está errada: {wrong}"),
        )),
    }
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

/// What a field of `kind` takes, in words.
fn expected(kind: &FieldKind) -> String {
    match kind {
        FieldKind::Integer => "tem que ser um número inteiro".to_owned(),
        FieldKind::Decimal(_) | FieldKind::Money => "tem que ser um número".to_owned(),
        FieldKind::Boolean => "tem que ser true ou false".to_owned(),
        FieldKind::Enum(options) => {
            let codes: Vec<&str> = options.iter().map(|o| o.code.as_str()).collect();
            format!("tem que ser um destes: {}", codes.join(", "))
        }
        FieldKind::Tags(options) => {
            let codes: Vec<&str> = options.iter().map(|o| o.code.as_str()).collect();
            format!("tem que ser uma lista com alguns destes: {}", codes.join(", "))
        }
        FieldKind::Text
        | FieldKind::LongText
        | FieldKind::Color
        | FieldKind::Criteria
        | FieldKind::Date
        | FieldKind::DateTime => "tem que ser um texto".to_owned(),
        _ => "é de um tipo que não cabe numa configuração".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use ravenrs_app::{EnumOption, FieldInfo, Rule};
    use serde_json::json;

    use super::*;

    fn docker() -> EntityInfo {
        EntityInfo::new("DockerSettings")
            .field(FieldInfo::new(
                "engine",
                FieldKind::Enum(vec![
                    EnumOption::new("docker", "Docker"),
                    EnumOption::new("podman", "Podman"),
                ]),
            ))
            .field(FieldInfo::new("keep", FieldKind::Boolean))
            .rule(Rule::required("engine"))
    }

    #[test]
    fn each_problem_comes_with_its_field() {
        assert!(check_settings(&docker(), &json!({ "engine": "podman", "keep": false })).is_ok());
        let problems =
            check_settings(&docker(), &json!({ "engine": "lxc", "keep": "yes" })).expect_err("wrong");
        let said: Vec<String> = problems.iter().map(ToString::to_string).collect();
        assert_eq!(
            said,
            [
                "a configuração engine: tem que ser um destes: docker, podman",
                "a configuração keep: tem que ser true ou false",
            ]
        );
    }

    #[test]
    fn a_required_field_left_empty_is_said_by_the_rules_of_the_entity() {
        let problems = check_settings(&docker(), &json!({})).expect_err("empty");
        assert_eq!(problems.len(), 1);
        assert!(matches!(&problems[0], CiError::Settings { field, .. } if field == "engine"));
        assert!(check_settings(&docker(), &json!([1])).is_err());
    }
}
