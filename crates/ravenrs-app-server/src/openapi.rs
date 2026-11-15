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
/// A state is read-only on a form, and how it changes here.
fn writable(entity: &EntityInfo, field: &FieldInfo, creating: bool) -> bool {
    let is_state = entity.state_machines.iter().any(|machine| machine.field == field.name);
    exposed(field)
        && (!field.read_only || is_state)
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

/// The entities a description is of: those this person can reach, and all of them, where an
/// owner finds the field its records point back through.
struct Model<'a> {
    reachable: &'a [EntityInfo],
    all: &'a [EntityInfo],
}

fn reference(name: &str) -> Json {
    json!({ "$ref": format!("#/components/schemas/{name}") })
}

/// The schema of an entity's key, as it is read.
fn key_schema(entity: &EntityInfo) -> Option<Json> {
    let key = entity.key_field()?;
    schema_of(entity).get("properties")?.get(&key.name).cloned()
}

/// The names of what a record within its owner is written as: new, or one it already has.
fn nested_names(owner: &EntityInfo, field: &FieldInfo) -> (String, String) {
    (format!("{}.{}.Create", owner.name, field.name), format!("{}.{}.Change", owner.name, field.name))
}

/// What an aggregated collection takes, as `nested` in the service reads it: a list of records to
/// create, records it has to change, by their key, and `{"$delete": key}`. None when nothing is
/// taken, not even an empty list.
fn collection_of(model: &Model, owner: &EntityInfo, field: &FieldInfo) -> Option<Json> {
    let (child, _) = crate::back_reference(model.all, owner, field)?;
    let key = key_schema(&child)?;
    if !model.reachable.iter().any(|entity| entity.name == child.name) {
        // Nothing of it can be written by this person, and an empty list writes nothing.
        return Some(json!({ "type": "array", "maxItems": 0 }));
    }
    let (new, existing) = nested_names(owner, field);
    let delete = json!({ "type": "object", "required": ["$delete"], "properties": { "$delete": key } });
    Some(json!({
        "type": "array",
        "items": { "anyOf": [reference(&new), reference(&existing), delete] },
    }))
}

/// What a body that creates or changes a record has to be, by the rules the server holds it to.
/// Within its owner, `back` is filled by the owner and taken from nobody.
///
/// It never turns down what the server takes: it says less, never more. The record as read stays
/// apart, since records kept before a rule came in need not meet it.
fn input_of(model: &Model, entity: &EntityInfo, creating: bool, back: Option<&str>) -> Json {
    let mut schema = schema_of(entity);
    // Every space starts a record from this, and only numbers a whole-number key on top.
    let fresh = entity.new_object();
    let mut required = Vec::new();
    let mut taken = Map::new();
    let read = schema.get("properties").and_then(Json::as_object).cloned().unwrap_or_default();
    for field in &entity.fields {
        if back == Some(field.name.as_str()) {
            continue;
        }
        if matches!(field.kind, FieldKind::Collection(_)) {
            if field.aggregated
                && let Some(collection) = collection_of(model, entity, field)
            {
                taken.insert(field.name.clone(), collection);
            }
            continue;
        }
        // Within an owner, a record sent with its key is one to change, never a new one.
        let keyed_within = back.is_some() && field.is_key;
        if !writable(entity, field, creating) || keyed_within {
            continue;
        }
        let Some(Json::Object(mut property)) = read.get(&field.name).cloned() else { continue };
        let rules: Vec<&Rule> = entity
            .rules
            .iter()
            .filter(|rule| rule.field.as_deref() == Some(field.name.as_str()) && always(rule))
            .collect();
        constrain(&mut property, field, &rules);
        let needed = rules.iter().any(|rule| matches!(rule.kind, RuleKind::Required));
        if creating && needed && empty(&fresh.get(&field.name)) {
            required.push(field.name.clone());
        }
        taken.insert(field.name.clone(), Json::Object(property));
    }
    if let Some(object) = schema.as_object_mut() {
        object.insert("properties".to_owned(), Json::Object(taken));
        // Anything else refuses the whole request.
        object.insert("additionalProperties".to_owned(), json!(false));
        if !required.is_empty() {
            object.insert("required".to_owned(), json!(required));
        }
    }
    schema
}

/// A record within its owner that it already has: what changes, and the key that finds it.
fn existing_within(model: &Model, child: &EntityInfo, back: &str) -> Json {
    let mut schema = input_of(model, child, false, Some(back));
    if let (Some(key), Some(key_schema)) = (child.key_field(), key_schema(child))
        && let Some(object) = schema.as_object_mut()
    {
        if let Some(properties) = object.get_mut("properties").and_then(Json::as_object_mut) {
            properties.insert(key.name.clone(), key_schema);
        }
        object.insert("required".to_owned(), json!([key.name]));
    }
    schema
}

/// The body of `/api/batch`: each operation as `one_of_batch` in the service reads it.
fn batch_of(model: &Model) -> Json {
    // What the body of an operation can also be: nothing, which is the same as `{}`.
    let nothing = json!({ "type": "object", "nullable": true, "maxProperties": 0 });
    let mut operations = Vec::new();
    for entity in model.reachable {
        let name = entity.name.as_str();
        let operation = |method: &str, body: Json, key: Option<&Json>| {
            let mut properties = Map::new();
            properties.insert("method".to_owned(), json!({ "type": "string", "enum": [method] }));
            properties.insert("entity".to_owned(), json!({ "type": "string", "enum": [name] }));
            properties.insert("body".to_owned(), body);
            let mut required = vec!["method", "entity"];
            if let Some(key) = key {
                properties.insert("key".to_owned(), key.clone());
                required.push("key");
            }
            json!({ "type": "object", "required": required, "properties": properties })
        };
        let create = format!("{name}.Create");
        operations.push(operation("POST", json!({ "anyOf": [reference(&create), nothing] }), None));
        if let Some(key) = key_schema(entity) {
            let change = format!("{name}.Change");
            operations.push(operation(
                "PATCH",
                json!({ "anyOf": [reference(&change), nothing] }),
                Some(&key),
            ));
            operations.push(operation("DELETE", json!({ "type": "object", "nullable": true }), Some(&key)));
        }
    }
    let items = if operations.is_empty() { json!({}) } else { json!({ "anyOf": operations }) };
    let most = if operations.is_empty() { 0 } else { crate::MOST_OPERATIONS };
    json!({
        "type": "object",
        "required": ["operations"],
        "properties": { "operations": { "type": "array", "maxItems": most, "items": items } },
    })
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

/// The description, with only the entities in `entities`, out of `all` the model has.
pub(crate) fn document(title: &str, entities: &[EntityInfo], all: &[EntityInfo]) -> Json {
    let model = Model { reachable: entities, all };
    let mut paths = Map::new();
    let mut schemas = Map::new();
    let record = |name: &str| json!({ "$ref": format!("#/components/schemas/{name}") });
    let body = |name: &str| json!({ "required": true, "content": { "application/json": { "schema": record(name) } } });
    let (creating, changing) = (|name: &str| format!("{name}.Create"), |name: &str| format!("{name}.Change"));
    let one = |name: &str| json!({ "description": "The record.", "content": { "application/json": { "schema": record(name) } } });
    for entity in entities {
        let name = entity.name.as_str();
        schemas.insert(entity.name.clone(), schema_of(entity));
        schemas.insert(creating(name), input_of(&model, entity, true, None));
        schemas.insert(changing(name), input_of(&model, entity, false, None));
        for field in entity.fields.iter().filter(|field| field.aggregated) {
            if let Some((child, back)) = crate::back_reference(all, entity, field)
                && entities.iter().any(|reached| reached.name == child.name)
            {
                let (new, existing) = nested_names(entity, field);
                schemas.insert(new, input_of(&model, &child, true, Some(&back)));
                schemas.insert(existing, existing_within(&model, &child, &back));
            }
        }
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
            "requestBody": { "required": true, "content": { "application/json": { "schema": batch_of(&model) } } },
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

    use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Rule, StateMachine};
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

    fn order() -> EntityInfo {
        EntityInfo::new("Order")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text))
            .field(FieldInfo::new("state", FieldKind::Text).read_only())
            .field(
                FieldInfo::new("lines", FieldKind::Collection("Line".into()))
                    .back_reference("order")
                    .aggregated(),
            )
            .field(FieldInfo::new("notes", FieldKind::Collection("Note".into())).back_reference("order"))
            .state_machine(StateMachine::new("state").state("o", "Open").start("o"))
    }

    fn line() -> EntityInfo {
        EntityInfo::new("Line")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("order", FieldKind::Reference("Order".into())))
            .field(FieldInfo::new("amount", FieldKind::Integer))
            .rule(Rule::required("order"))
            .rule(Rule::required("amount"))
    }

    fn note() -> EntityInfo {
        EntityInfo::new("Note")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("order", FieldKind::Reference("Order".into())))
    }

    fn orders(reachable: &[EntityInfo]) -> Json {
        document("Teste", reachable, &[order(), line(), note()])
    }

    fn schemas() -> Json {
        document("Teste", &[plate()], &[plate()])["components"]["schemas"].clone()
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
        let any = r"(?:[\uD800-\uDBFF][\uDC00-\uDFFF]|[^\uD800-\uDFFF\n])";
        assert_eq!(create["loose"]["pattern"], format!(r"^[\s\u0085]*$|(?:^{any}+@{any}+$)"));
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
        let paths = document("Teste", &[plate()], &[plate()])["paths"].clone();
        let schema = |path: &str, method: &str| {
            paths[path][method]["requestBody"]["content"]["application/json"]["schema"]["$ref"].clone()
        };
        assert_eq!(schema("/api/data/Plate", "post"), "#/components/schemas/Plate.Create");
        assert_eq!(schema("/api/data/Plate/{key}", "patch"), "#/components/schemas/Plate.Change");
    }

    #[test]
    fn a_body_holds_what_the_server_takes_and_nothing_else() {
        let create = &schemas()["Plate.Create"];
        assert_eq!(create["additionalProperties"], false);
        assert!(create["properties"].get("stamp").is_none(), "read-only, refused when sent");
        assert!(create["properties"].get("id").is_none(), "a numbered key is given by the server");
        let order = &orders(&[order(), line(), note()])["components"]["schemas"];
        assert!(order["Order.Create"]["properties"].get("state").is_some(), "a state is how it changes");
        assert!(order["Order.Create"]["properties"].get("notes").is_none(), "not written through its owner");
    }

    #[test]
    fn records_within_their_owner_go_as_the_service_reads_them() {
        let all = orders(&[order(), line(), note()]);
        let schemas = &all["components"]["schemas"];
        let items = &schemas["Order.Create"]["properties"]["lines"]["items"]["anyOf"];
        assert_eq!(items[0]["$ref"], "#/components/schemas/Order.lines.Create");
        assert_eq!(items[1]["$ref"], "#/components/schemas/Order.lines.Change");
        assert_eq!(items[2]["required"], json!(["$delete"]));
        assert_eq!(items[2]["properties"]["$delete"]["type"], "integer");
        let new = &schemas["Order.lines.Create"];
        assert!(new["properties"].get("order").is_none(), "the owner fills it");
        assert_eq!(new["required"], json!(["amount"]), "and does not ask for it");
        let existing = &schemas["Order.lines.Change"];
        assert_eq!(existing["required"], json!(["id"]));
        assert!(existing["properties"].get("order").is_none());
        assert_eq!(existing["additionalProperties"], false);
    }

    #[test]
    fn a_collection_nobody_here_can_write_takes_only_an_empty_list() {
        let all = orders(&[order()]);
        let lines = &all["components"]["schemas"]["Order.Create"]["properties"]["lines"];
        assert_eq!(lines["maxItems"], 0);
        assert!(all["components"]["schemas"].get("Order.lines.Create").is_none());
    }

    #[test]
    fn each_operation_of_a_batch_is_of_one_entity_and_one_method() {
        let all = orders(&[order(), line()]);
        let body =
            &all["paths"]["/api/batch"]["post"]["requestBody"]["content"]["application/json"]["schema"];
        assert_eq!(body["required"], json!(["operations"]));
        let operations = &body["properties"]["operations"];
        assert_eq!(operations["maxItems"], crate::MOST_OPERATIONS);
        let each = operations["items"]["anyOf"].as_array().unwrap();
        assert_eq!(each.len(), 6, "POST, PATCH and DELETE of two entities");
        let post = &each[0];
        assert_eq!(post["properties"]["method"]["enum"], json!(["POST"]));
        assert_eq!(post["properties"]["entity"]["enum"], json!(["Order"]));
        assert_eq!(post["properties"]["body"]["anyOf"][0]["$ref"], "#/components/schemas/Order.Create");
        assert_eq!(post["properties"]["body"]["anyOf"][1]["nullable"], true);
        assert_eq!(each[1]["required"], json!(["method", "entity", "key"]));
    }
}
