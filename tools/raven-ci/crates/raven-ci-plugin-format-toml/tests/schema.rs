//! The schema of `.raven-ci.toml` and the reader agree (#941): every example of the guides and of
//! the crate's documentation is taken or refused by both, and a few broken ones are refused by both.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use raven_ci_plugin_api::PipelineFormat;
use raven_ci_plugin_format_toml::{TomlFormat, schema};
use serde_json::Value;

/// Whether `value` fits `schema`, for the part of JSON Schema the schema of the pipeline uses:
/// type, const, enum, properties, required, additionalProperties, items, minItems and oneOf.
fn fits(schema: &Value, value: &Value) -> Result<(), String> {
    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        let fitting = options.iter().filter(|option| fits(option, value).is_ok()).count();
        return if fitting == 1 { Ok(()) } else { Err(format!("{value} fits {fitting} of oneOf")) };
    }
    if let Some(wanted) = schema.get("const")
        && wanted != value
    {
        return Err(format!("{value} is not {wanted}"));
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array)
        && !options.contains(value)
    {
        return Err(format!("{value} is not one of {options:?}"));
    }
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        let right = match kind {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "integer" => value.is_i64() || value.is_u64(),
            "boolean" => value.is_boolean(),
            _ => true,
        };
        if !right {
            return Err(format!("{value} is not {kind}"));
        }
    }
    if let Some(object) = value.as_object() {
        let properties = schema.get("properties").and_then(Value::as_object);
        for required in schema.get("required").and_then(Value::as_array).into_iter().flatten() {
            let name = required.as_str().unwrap_or_default();
            if !object.contains_key(name) {
                return Err(format!("missing {name}"));
            }
        }
        for (key, field) in object {
            match properties.and_then(|p| p.get(key)) {
                Some(described) => fits(described, field).map_err(|e| format!("{key}: {e}"))?,
                None => match schema.get("additionalProperties") {
                    Some(Value::Bool(false)) => return Err(format!("{key} is not a field")),
                    Some(other @ Value::Object(_)) => {
                        fits(other, field).map_err(|e| format!("{key}: {e}"))?
                    }
                    _ => {}
                },
            }
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(least) = schema.get("minItems").and_then(Value::as_u64)
            && (items.len() as u64) < least
        {
            return Err(format!("fewer than {least} items"));
        }
        if let Some(item) = schema.get("items") {
            for (n, element) in items.iter().enumerate() {
                fits(item, element).map_err(|e| format!("[{n}]: {e}"))?;
            }
        }
    }
    Ok(())
}

/// The ```toml blocks of `markdown` that are pipelines: those that say `[[stages]]`.
fn pipelines_in(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut block: Option<String> = None;
    for line in markdown.lines() {
        let line = line.strip_prefix("//! ").or_else(|| line.strip_prefix("//!")).unwrap_or(line);
        match &mut block {
            None if line.trim_start().starts_with("```toml") => block = Some(String::new()),
            Some(text) if line.trim_start().starts_with("```") => {
                if text.contains("[[stages]]") {
                    found.push(text.clone());
                }
                block = None;
            }
            Some(text) => {
                text.push_str(line);
                text.push('\n');
            }
            None => {}
        }
    }
    found
}

fn agree(text: &str) -> (bool, Result<(), String>) {
    let read = TomlFormat.read("exemplo", text);
    let value: Result<Value, _> = toml::from_str(text);
    let fit = value.map_err(|e| e.to_string()).and_then(|value| fits(&schema(), &value));
    (read.is_ok(), fit)
}

#[test]
fn every_pipeline_of_the_guides_is_taken_by_the_reader_and_by_the_schema() {
    let sources = [
        include_str!("../../../docs/pipeline.md"),
        include_str!("../../../docs/migrating-from-jenkins.md"),
        include_str!("../../../docs/agents.md"),
        include_str!("../src/lib.rs"),
    ];
    let examples: Vec<String> = sources.iter().flat_map(|source| pipelines_in(source)).collect();
    assert!(examples.len() >= 3, "found {} examples", examples.len());
    for example in &examples {
        let (read, fit) = agree(example);
        assert!(
            read,
            "the reader refused an example of the guides:\n{example}\n{:?}",
            TomlFormat.read("x", example).err()
        );
        assert!(fit.is_ok(), "the schema refused an example the reader takes: {fit:?}\n{example}");
    }
}

#[test]
fn what_the_reader_refuses_for_its_shape_the_schema_refuses_too() {
    let broken = [
        "[[stages]]\nname = \"a\"\nstep = [\"x\"]\n",
        "[[stages]]\nname = \"a\"\nsteps = [{ sh = \"x\", nada = 1 }]\n",
        "[pipeline]\nkeep = \"dez\"\n[[stages]]\nname = \"a\"\nsteps = [\"x\"]\n",
        "[[parameters]]\nname = \"E\"\ntype = \"lista\"\n[[stages]]\nname = \"a\"\nsteps = [\"x\"]\n",
        "[[stages]]\nname = \"a\"\nsteps = [\"x\"]\nwhen = { tag = \"v1\" }\n",
        "[[stages]]\nname = \"a\"\nsteps = [\"x\"]\ncredentials = { T = 3 }\n",
        "pipeline = 3\n[[stages]]\nname = \"a\"\nsteps = [\"x\"]\n",
    ];
    for text in broken {
        let (read, fit) = agree(text);
        assert!(!read, "the reader took:\n{text}");
        assert!(fit.is_err(), "the schema took what the reader refuses:\n{text}");
    }
}
