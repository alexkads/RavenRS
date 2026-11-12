//! A description of the API for this person, in OpenAPI 3.

use ravenrs_app::{EntityInfo, FieldKind};
use serde_json::{Map, Value as Json, json};

use crate::json::exposed;

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
    let one = |name: &str| json!({ "description": "The record.", "content": { "application/json": { "schema": record(name) } } });
    for entity in entities {
        let name = entity.name.as_str();
        schemas.insert(entity.name.clone(), schema_of(entity));
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
                    "requestBody": body(name),
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
                    "requestBody": body(name),
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
