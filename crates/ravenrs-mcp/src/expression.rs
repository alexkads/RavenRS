//! Expression checks on top of `ravenrs-criteria`. Nothing here evaluates anything.

use std::collections::BTreeMap;

use ravenrs_criteria::functions::catalog;
use ravenrs_criteria::{Criteria, FieldType, Schema, validate};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};

/// What an expression may refer to.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct SchemaInput {
    /// Field name or dotted path, mapped to text, number, date, boolean or any.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// Parameter name without the `?`, mapped to text, number, date, boolean or any.
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
    /// Nested collections, mapped to the schema of their rows.
    #[serde(default)]
    pub collections: BTreeMap<String, SchemaInput>,
    /// Names of custom functions the expression may call.
    #[serde(default)]
    pub functions: Vec<String>,
}

impl SchemaInput {
    fn build(&self) -> Result<Schema, String> {
        let mut schema = Schema::new();
        for (name, kind) in &self.fields {
            schema = schema.field(name, field_type(kind)?);
        }
        for (name, kind) in &self.parameters {
            schema = schema.parameter(name, field_type(kind)?);
        }
        for (name, rows) in &self.collections {
            schema = schema.collection(name, rows.build()?);
        }
        for name in &self.functions {
            schema = schema.function(name);
        }
        Ok(schema)
    }
}

fn field_type(kind: &str) -> Result<FieldType, String> {
    match kind.to_lowercase().as_str() {
        "text" => Ok(FieldType::Text),
        "number" => Ok(FieldType::Number),
        "date" => Ok(FieldType::Date),
        "boolean" => Ok(FieldType::Boolean),
        "any" => Ok(FieldType::Any),
        _ => Err(format!("unknown field type '{kind}': use text, number, date, boolean or any")),
    }
}

/// Parses an expression and, given a schema, checks what it refers to.
/// An invalid expression is an answer, not a failure; only a bad schema is an error.
pub fn check(expression: &str, schema: Option<&SchemaInput>) -> Result<Value, String> {
    let schema = schema.map(SchemaInput::build).transpose()?;
    let parsed = match Criteria::parse(expression) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Ok(json!({
                "valid": false,
                "error": { "message": error.message, "offset": error.offset },
                "problems": [],
            }));
        }
    };
    let problems: Vec<String> = schema
        .map(|schema| validate(&parsed, &schema).iter().map(ToString::to_string).collect())
        .unwrap_or_default();
    Ok(json!({
        "valid": problems.is_empty(),
        "normalized": parsed.to_string(),
        "fields": parsed.properties(),
        "parameters": parsed.parameters(),
        "problems": problems,
    }))
}

/// The function catalog, filtered by category and by text.
pub fn functions(category: Option<&str>, query: Option<&str>) -> Value {
    let query = query.map(str::to_lowercase);
    let found: Vec<Value> = catalog()
        .into_iter()
        .map(|info| (format!("{:?}", info.category), info))
        .filter(|(kind, _)| category.is_none_or(|category| kind.eq_ignore_ascii_case(category)))
        .filter(|(_, info)| {
            let text = format!("{} {}", info.signature, info.description).to_lowercase();
            query.as_ref().is_none_or(|query| text.contains(query))
        })
        .map(|(kind, info)| {
            json!({
                "name": info.name,
                "category": kind,
                "signature": info.signature,
                "description": info.description,
            })
        })
        .collect();
    json!({ "total": found.len(), "functions": found })
}
