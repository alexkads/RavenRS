//! What an expression of a flow can read, and how a parameter written as text with
//! expressions inside it (`Olá {{ nome }}`) is checked, previewed and worked out.
//!
//! The expressions are of the criteria language (`ravenrs-criteria`). An item's own fields read
//! as they are (`[nome]` or `{{ nome }}`), and four names reach further: `item` (the item),
//! `nodes.<id>` (the first item a node that already ran gave), `vars` (the app's variables)
//! and `run` (the run: its `started`, in milliseconds since 1970). Each evaluation gets its
//! own environment, so runs on different threads never share one.

use std::collections::BTreeMap;

use ravenrs_criteria::{Environment, RenderError, Scope, Template, TemplateError};
use serde_json::{Map, Value, json};

use crate::definition::FlowDefinition;

/// The name of the item itself.
pub const ITEM: &str = "item";
/// The name of the nodes that already ran, each by its id.
pub const NODES: &str = "nodes";
/// The name of the app's variables.
pub const VARS: &str = "vars";
/// The name of the run.
pub const RUN: &str = "run";

/// How many bytes a preview may grow to before it stops and says so.
pub const PREVIEW_LIMIT: usize = 64 * 1024;

/// What an expression reads for `item`: the item's own fields, and [`ITEM`], [`NODES`],
/// [`VARS`] and [`RUN`].
pub fn record(
    item: &Value,
    nodes: &BTreeMap<String, Value>,
    vars: &Map<String, Value>,
    run: &Value,
) -> Value {
    let mut map = item.as_object().cloned().unwrap_or_default();
    map.insert(ITEM.into(), item.clone());
    map.insert(NODES.into(), Value::Object(nodes.iter().map(|(k, v)| (k.clone(), v.clone())).collect()));
    map.insert(VARS.into(), Value::Object(vars.clone()));
    map.insert(RUN.into(), run.clone());
    Value::Object(map)
}

/// The run as expressions read it.
pub fn run_value(started_ms: i64) -> Value {
    json!({ "started": started_ms })
}

/// The problems of `text` as a template of `flow`: what does not parse, and each
/// `nodes.<id>` that names no node of the flow, marked where it is.
pub fn check(text: &str, flow: &FlowDefinition) -> Vec<TemplateError> {
    let template = match Template::parse(text) {
        Ok(t) => t,
        Err(errors) => return errors,
    };
    template
        .references()
        .into_iter()
        .filter_map(|(path, at)| match path.as_slice() {
            [head, id, ..] if head == NODES && flow.find(id).is_none() => {
                Some(TemplateError { at, message: format!("there is no node {id} in this flow") })
            }
            _ => None,
        })
        .collect()
}

/// What a preview of a template says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preview {
    /// The text it works out to.
    Text(String),
    /// What is wrong with it, and where; nothing was worked out.
    Problems(Vec<TemplateError>),
    /// It parsed, but working it out failed, saying why: never shown as an empty value.
    Failed(String),
}

/// `text` worked out over `record` (see [`record`]), checked first against `flow` when there
/// is one, and stopped past [`PREVIEW_LIMIT`].
pub fn preview(text: &str, record: &Value, flow: Option<&FlowDefinition>) -> Preview {
    let problems = match flow {
        Some(flow) => check(text, flow),
        None => Template::parse(text).err().unwrap_or_default(),
    };
    if !problems.is_empty() {
        return Preview::Problems(problems);
    }
    let Ok(template) = Template::parse(text) else { return Preview::Problems(Vec::new()) };
    let env = Environment::new();
    match template.render_within(&Scope::new(&env).row(record), PREVIEW_LIMIT) {
        Ok(text) => Preview::Text(text),
        Err(RenderError::TooLong(limit)) => {
            Preview::Failed(format!("the text passed {limit} bytes, the limit of a preview"))
        }
        Err(error) => Preview::Failed(error.to_string()),
    }
}

/// `text` worked out for one item of a run, a failure saying why.
pub fn render(text: &str, record: &Value) -> Result<String, String> {
    let template = Template::parse(text)
        .map_err(|errors| errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "))?;
    let env = Environment::new();
    template.render(&Scope::new(&env).row(record)).map_err(|e| e.to_string())
}
