//! A description of the API for this person, in OpenAPI 3.

use ravenrs_app::{CONTEXT_SAVE, EntityInfo, FieldInfo, FieldKind, Rule, RuleKind, Severity, Value};
use serde_json::{Map, Value as Json, json};

use crate::json::{exposed, takes_text};
use crate::pattern::ecma;

fn schema_of(entity: &EntityInfo) -> Json {
    let mut properties = Map::new();
    for field in entity.fields.iter().filter(|field| exposed(field)) {
        let mut schema = match &field.kind {
            FieldKind::Integer => json!({ "type": "integer" }),
            FieldKind::Money => {
                json!({ "type": "integer", "description": "In cents, so it stays exact." })
            }
            FieldKind::Decimal(_) => json!({ "type": "number" }),
            FieldKind::Boolean => json!({ "type": "boolean" }),
            FieldKind::Date => json!({ "type": "string", "format": "date" }),
            FieldKind::DateTime => json!({ "type": "string", "format": "date-time" }),
            FieldKind::Guid => json!({ "type": "string", "format": "uuid" }),
            FieldKind::Enum(options) => {
                let codes: Vec<&str> = options.iter().map(|option| option.code.as_str()).collect();
                json!({ "type": "string", "enum": codes })
            }
            FieldKind::Reference(target) => {
                json!({ "description": format!("The key of a record of {target}.") })
            }
            _ => json!({ "type": "string" }),
        };
        if let Some(object) = schema.as_object_mut() {
            object.insert("title".to_owned(), json!(field.caption));
            if field.read_only || field.calculated.is_some() {
                object.insert("readOnly".to_owned(), json!(true));
            }
            if matches!(field.kind, FieldKind::Reference(_)) || !field.is_key {
                object.insert("nullable".to_owned(), json!(true));
            }
        }
        properties.insert(field.name.clone(), schema);
    }
    let collections: Vec<&str> = entity
        .fields
        .iter()
        .filter(|field| matches!(field.kind, FieldKind::Collection(_)) && field.aggregated)
        .map(|field| field.name.as_str())
        .collect();
    let mut schema = json!({ "type": "object", "title": entity.caption, "properties": properties });
    if !collections.is_empty()
        && let Some(object) = schema.as_object_mut()
    {
        object.insert(
            "description".to_owned(),
            json!(format!(
                "Writing also takes these collections, as a list of records to create or change, or \
                 {{\"$delete\": key}}: {}.",
                collections.join(", ")
            )),
        );
    }
    schema
}

/// Whether a rule holds for every write, whatever the record: the only ones a client is told of.
/// A warning lets the write through, and a rule with a target depends on the rest of the record.
fn always(rule: &Rule) -> bool {
    rule.severity == Severity::Error
        && rule.target_criteria.is_none()
        && rule.contexts.iter().any(|context| context == CONTEXT_SAVE)
}

/// Whether the server takes this field in a body: what [`crate::json::apply`] does not refuse.
fn writable(field: &FieldInfo, creating: bool) -> bool {
    exposed(field)
        && !field.read_only
        && field.calculated.is_none()
        && !(field.is_key && (!creating || matches!(field.kind, FieldKind::Integer)))
}

/// Empty as a `Required` rule sees it.
fn empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Text(text) => text.trim().is_empty(),
        _ => false,
    }
}

fn number(value: f64) -> Json {
    if value.fract() == 0.0 && value.abs() < 9e15 { json!(value as i64) } else { json!(value) }
}

/// What the rules on one field say of the value sent for it, as JSON Schema keywords.
fn constrain(property: &mut Map<String, Json>, field: &FieldInfo, rules: &[&Rule]) {
    let mut lowest: Option<f64> = None;
    let mut highest: Option<f64> = None;
    let mut patterns = Vec::new();
    for rule in rules {
        match &rule.kind {
            RuleKind::Required => {
                property.remove("nullable");
                if takes_text(&field.kind) {
                    // Blank text is empty too; one character is as much as is sure for every client.
                    property.insert("minLength".to_owned(), json!(1));
                }
            }
            RuleKind::Range { min, max }
                if matches!(field.kind, FieldKind::Integer | FieldKind::Money | FieldKind::Decimal(_)) =>
            {
                let finite = |limit: &Option<f64>| limit.filter(|limit| limit.is_finite());
                if let Some(min) = finite(min) {
                    lowest = Some(lowest.map_or(min, |lowest| lowest.max(min)));
                }
                if let Some(max) = finite(max) {
                    highest = Some(highest.map_or(max, |highest| highest.min(max)));
                }
            }
            RuleKind::Matches(pattern) if takes_text(&field.kind) => {
                if let Some(translated) = ecma(pattern) {
                    // A rule that skips the empty lets blank text by, as Rust's `trim` sees blank.
                    patterns.push(if rule.skip_empty {
                        format!(r"^[\s\u0085]*$|(?:{translated})")
                    } else {
                        translated
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(lowest) = lowest {
        property.insert("minimum".to_owned(), number(lowest));
    }
    if let Some(highest) = highest {
        property.insert("maximum".to_owned(), number(highest));
    }
    match patterns.len() {
        0 => {}
        1 => {
            property.insert("pattern".to_owned(), json!(patterns.remove(0)));
        }
        _ => {
            let all: Vec<Json> = patterns.into_iter().map(|pattern| json!({ "pattern": pattern })).collect();
            property.insert("allOf".to_owned(), Json::Array(all));
        }
    }
}

/// What a body that creates or changes a record has to be, by the rules the server holds it to.
///
/// It never turns down what the server takes: it says less, never more. The record as read stays
/// apart, since records kept before a rule came in need not meet it.
fn input_of(entity: &EntityInfo, creating: bool) -> Json {
    let mut schema = schema_of(entity);
    // Every space starts a record from this, and only numbers a whole-number key on top.
    let fresh = entity.new_object();
    let mut required = Vec::new();
    if let Some(properties) = schema.get_mut("properties").and_then(Json::as_object_mut) {
        for field in entity.fields.iter().filter(|field| writable(field, creating)) {
            let Some(property) = properties.get_mut(&field.name).and_then(Json::as_object_mut) else {
                continue;
            };
            let rules: Vec<&Rule> = entity
                .rules
                .iter()
                .filter(|rule| rule.field.as_deref() == Some(field.name.as_str()) && always(rule))
                .collect();
            constrain(property, field, &rules);
            let needed = rules.iter().any(|rule| matches!(rule.kind, RuleKind::Required));
            if creating && needed && empty(&fresh.get(&field.name)) {
                required.push(field.name.clone());
            }
        }
    }
    if !required.is_empty()
        && let Some(object) = schema.as_object_mut()
    {
        object.insert("required".to_owned(), json!(required));
    }
    schema
}

fn answers(ok: Json) -> Json {
    let problem = |text: &str| json!({ "description": text });
    json!({
        "200": ok,
        "400": problem("What was asked does not fit the entity."),
        "401": problem("A valid token is required."),
        "403": problem("This person cannot do this."),
        "404": problem("There is no such record, or this person cannot see it."),
        "409": problem("Someone else got there first."),
        "422": problem("The rules of the entity refuse it."),
    })
}

/// The description, with only the entities in `entities`.
pub(crate) fn document(title: &str, entities: &[EntityInfo]) -> Json {
    let mut paths = Map::new();
    let mut schemas = Map::new();
    let record = |name: &str| json!({ "$ref": format!("#/components/schemas/{name}") });
    let body = |name: &str| json!({ "required": true, "content": { "application/json": { "schema": record(name) } } });
    let (creating, changing) = (|name: &str| format!("{name}.Create"), |name: &str| format!("{name}.Change"));
    let one = |name: &str| json!({ "description": "The record.", "content": { "application/json": { "schema": record(name) } } });
    for entity in entities {
        let name = entity.name.as_str();
        schemas.insert(entity.name.clone(), schema_of(entity));
        schemas.insert(creating(name), input_of(entity, true));
        schemas.insert(changing(name), input_of(entity, false));
        let list = json!({
            "description": "The records.",
            "content": { "application/json": { "schema": {
                "type": "object",
                "properties": {
                    "items": { "type": "array", "items": record(name) },
                    "count": { "type": "integer" },
                },
            } } },
        });
        let query = |parameter: &str, kind: &str, text: &str| json!({ "name": parameter, "in": "query", "description": text, "schema": { "type": kind } });
        paths.insert(
            format!("/api/data/{name}"),
            json!({
                "get": {
                    "summary": format!("Reads records of {}", entity.caption),
                    "parameters": [
                        query("filter", "string", "An expression, such as [amount] > 10 And [name] Like 'a%'."),
                        query("order", "string", "Fields separated by commas, with - before one to go down."),
                        query("skip", "integer", "How many to skip."),
                        query("top", "integer", "How many to return, 100 by default and 1000 at most."),
                        query("count", "boolean", "Also says how many there are in all."),
                        query("expand", "string", "Collections to bring within each record."),
                    ],
                    "responses": answers(list),
                },
                "post": {
                    "summary": format!("Creates a record of {}", entity.caption),
                    "requestBody": body(&creating(name)),
                    "responses": answers(one(name)),
                },
            }),
        );
        let key = json!({ "name": "key", "in": "path", "required": true, "schema": { "type": "string" } });
        paths.insert(
            format!("/api/data/{name}/{{key}}"),
            json!({
                "parameters": [key],
                "get": { "summary": "Reads one record", "responses": answers(one(name)) },
                "patch": {
                    "summary": "Changes the fields sent, and no others",
                    "requestBody": body(&changing(name)),
                    "responses": answers(one(name)),
                },
                "delete": {
                    "summary": "Deletes a record",
                    "responses": answers(json!({ "description": "It is gone." })),
                },
            }),
        );
    }
    paths.insert(
        "/api/batch".to_owned(),
        json!({ "post": {
            "summary": "Several writes that stay or go together",
            "requestBody": { "required": true, "content": { "application/json": { "schema": {
                "type": "object",
                "properties": { "operations": { "type": "array", "items": {
                    "type": "object",
                    "required": ["method", "entity"],
                    "properties": {
                        "method": { "type": "string", "enum": ["POST", "PATCH", "DELETE"] },
                        "entity": { "type": "string" },
                        "key": {},
                        "body": { "type": "object" },
                    },
                } } },
            } } } },
            "responses": answers(json!({ "description": "What each operation left." })),
        } }),
    );
    json!({
        "openapi": "3.0.3",
        "info": { "title": title, "version": env!("CARGO_PKG_VERSION") },
        "security": [{ "token": [] }],
        "paths": paths,
        "components": {
            "schemas": schemas,
            "securitySchemes": { "token": { "type": "http", "scheme": "bearer" } },
        },
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Rule};
    use serde_json::{Value as Json, json};

    use super::document;

    fn plate() -> EntityInfo {
        EntityInfo::new("Plate")
            .caption("Placa")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text))
            .field(FieldInfo::new("country", FieldKind::Text).default_value("BR"))
            .field(FieldInfo::new("seats", FieldKind::Integer))
            .field(FieldInfo::new("price", FieldKind::Money))
            .field(FieldInfo::new("code", FieldKind::Text))
            .field(FieldInfo::new("loose", FieldKind::Text))
            .field(FieldInfo::new("strict", FieldKind::Text))
            .field(FieldInfo::new("soft", FieldKind::Integer))
            .field(FieldInfo::new("maybe", FieldKind::Integer))
            .field(FieldInfo::new("stamp", FieldKind::Text).read_only())
            .rule(Rule::required("id"))
            .rule(Rule::required("name"))
            .rule(Rule::required("country"))
            .rule(Rule::required("stamp"))
            .rule(Rule::range("seats", Some(1.0), Some(10.0)))
            .rule(Rule::range("seats", Some(2.0), None))
            .rule(Rule::range("price", Some(0.0), Some(99_999.0)))
            .rule(Rule::matches("code", "^[A-Z]{3}-\\d{4}$"))
            .rule(Rule::matches("loose", "^.+@.+$"))
            .rule(Rule::matches("strict", "^[a-z]+$").skip_empty(false))
            .rule(Rule::range("soft", Some(5.0), None).warning())
            .rule(Rule::range("maybe", Some(5.0), None).target_criteria("[seats] > 2"))
    }

    fn schemas() -> Json {
        document("Teste", &[plate()])["components"]["schemas"].clone()
    }

    #[test]
    fn creating_asks_only_for_what_nothing_else_fills() {
        let create = &schemas()["Plate.Create"];
        assert_eq!(
            create["required"],
            json!(["name"]),
            "a numbered key, a default and a read-only field are left out"
        );
        let name = &create["properties"]["name"];
        assert!(name.get("nullable").is_none());
        assert_eq!(name["minLength"], 1);
        assert!(create["properties"]["country"].get("nullable").is_none(), "null still wipes the default");
    }

    #[test]
    fn changing_takes_any_part_of_the_record() {
        let change = &schemas()["Plate.Change"];
        assert!(change.get("required").is_none());
        assert!(change["properties"]["name"].get("nullable").is_none(), "sending null is refused there too");
        assert_eq!(change["properties"]["seats"]["minimum"], 2);
    }

    #[test]
    fn limits_and_patterns_go_with_the_writes() {
        let create = &schemas()["Plate.Create"]["properties"];
        assert_eq!(create["seats"]["minimum"], 2, "the tightest of the two rules");
        assert_eq!(create["seats"]["maximum"], 10);
        assert_eq!(create["price"]["maximum"], 99_999, "in cents, as the JSON is");
        assert_eq!(create["code"]["pattern"], r"^[\s\u0085]*$|(?:^[A-Z]{3}-[0-9]{4}$)");
        assert_eq!(create["strict"]["pattern"], "^[a-z]+$");
        assert!(create["loose"].get("pattern").is_none(), "`.` is not read the same everywhere");
        assert!(create["soft"].get("minimum").is_none(), "a warning lets the write through");
        assert!(create["maybe"].get("minimum").is_none(), "a target depends on the record");
    }

    #[test]
    fn the_record_as_read_is_held_to_nothing() {
        let all = schemas();
        let read = &all["Plate"];
        assert!(read.get("required").is_none());
        assert!(read["properties"]["seats"].get("minimum").is_none());
        assert_eq!(read["properties"]["name"]["nullable"], true);
        let paths = document("Teste", &[plate()])["paths"].clone();
        let schema = |path: &str, method: &str| {
            paths[path][method]["requestBody"]["content"]["application/json"]["schema"]["$ref"].clone()
        };
        assert_eq!(schema("/api/data/Plate", "post"), "#/components/schemas/Plate.Create");
        assert_eq!(schema("/api/data/Plate/{key}", "patch"), "#/components/schemas/Plate.Change");
    }
}
