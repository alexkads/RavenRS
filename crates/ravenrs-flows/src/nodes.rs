//! The built-in kinds: a manual start, branches, merges, a loop in batches, and the nodes that
//! set, filter, sort, group, split and join items. Conditions and values are written in the
//! criteria language (`[paid] = true`, `[qty] * [price]`) and read each item's JSON. Above
//! `run`, `plan`, `catalog` and `item`; nothing in the crate uses it, so an app may leave it
//! out.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use ravenrs_criteria::{Criteria, Environment, Scope, Value as Criterion};
use serde_json::{Map, Value, json};

use crate::catalog::{Catalog, ERROR_PORT, NodeContext, NodeError, NodeKind, NodeSpec};
use crate::definition::{FlowDefinition, NodeDef};
use crate::item::{Item, Ports};
use crate::plan::Plan;
use crate::run::{Executor, RunOutcome};

/// The output most nodes give.
pub const MAIN: &str = "main";

fn main_port(items: Vec<Item>) -> Ports {
    Ports::from([(MAIN.to_owned(), items)])
}

/// The parameter `name` read as an expression; `None` when the node has none.
fn expression(cx: &NodeContext<'_>, name: &str) -> Result<Option<Criteria>, NodeError> {
    match cx.parameter(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Criteria::parse(text)
            .map(Some)
            .map_err(|e| NodeError::new(format!("the parameter {name} is not an expression: {e}"))),
        Some(_) => Err(NodeError::new(format!("the parameter {name} must be an expression, as text"))),
    }
}

/// The parameter `name` read as an expression the node cannot do without.
fn required(cx: &NodeContext<'_>, name: &str) -> Result<Criteria, NodeError> {
    expression(cx, name)?.ok_or_else(|| NodeError::new(format!("the parameter {name} is missing")))
}

/// `criteria` worked out over `item`. The criteria language is parsed into a tree and read
/// field by field; it runs no code, so a flow's expressions cannot reach the machine.
fn work_out(env: &Environment, criteria: &Criteria, item: &Item) -> Criterion {
    Scope::new(env).row(&item.json).evaluate(criteria).unwrap_or(Criterion::Null)
}

/// Whether a value counts as true: `true`, a number other than 0, text that is not empty, a date.
fn truthy(value: &Criterion) -> bool {
    match value {
        Criterion::Null => false,
        Criterion::Bool(b) => *b,
        Criterion::Number(n) => *n != 0.0,
        Criterion::Text(t) => !t.is_empty(),
        _ => true,
    }
}

/// A value of the criteria language as JSON: whole numbers stay whole.
fn to_json(value: Criterion) -> Value {
    match value {
        Criterion::Null => Value::Null,
        Criterion::Bool(b) => Value::Bool(b),
        Criterion::Number(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => json!(n as i64),
        Criterion::Number(n) => json!(n),
        Criterion::Text(t) => Value::String(t),
        Criterion::Date(d) => Value::String(d.to_string()),
        other => Value::String(format!("{other:?}")),
    }
}

/// A value as the text a case or a key is compared by.
fn as_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The value at a field path written with dots (`address.city`), when the item has it.
fn field<'a>(json: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(json, |at, step| match at {
        Value::Object(map) => map.get(step),
        Value::Array(items) => items.get(step.parse::<usize>().ok()?),
        _ => None,
    })
}

/// The parameter `name` as text, or `default`.
fn text_parameter<'a>(cx: &'a NodeContext<'_>, name: &str, default: &'a str) -> &'a str {
    cx.parameter(name).and_then(Value::as_str).unwrap_or(default)
}

/// JSON values in a total order: missing and null first, then false before true, numbers,
/// text, and anything else by how it is written.
fn compare(a: Option<&Value>, b: Option<&Value>) -> Ordering {
    fn rank(v: Option<&Value>) -> u8 {
        match v {
            None | Some(Value::Null) => 0,
            Some(Value::Bool(_)) => 1,
            Some(Value::Number(_)) => 2,
            Some(Value::String(_)) => 3,
            Some(_) => 4,
        }
    }
    match (a, b) {
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => x.cmp(y),
        (Some(Value::Number(x)), Some(Value::Number(y))) => {
            x.as_f64().unwrap_or(0.0).partial_cmp(&y.as_f64().unwrap_or(0.0)).unwrap_or(Ordering::Equal)
        }
        (Some(Value::String(x)), Some(Value::String(y))) => x.cmp(y),
        (Some(x), Some(y)) if rank(a) == 4 && rank(b) == 4 => x.to_string().cmp(&y.to_string()),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// Two objects as one; the fields of `over` win.
fn merged(base: &Value, over: &Value) -> Value {
    match (base, over) {
        (Value::Object(a), Value::Object(b)) => {
            let mut map = a.clone();
            map.extend(b.iter().map(|(k, v)| (k.clone(), v.clone())));
            Value::Object(map)
        }
        (_, Value::Null) => base.clone(),
        _ => over.clone(),
    }
}

/// Starts a run with the items of its parameter `items`, a list; one empty item without it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Manual;

impl NodeKind for Manual {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("manual", 1).title("Start by hand").output(MAIN).trigger()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let items = match cx.parameter("items") {
            Some(Value::Array(list)) => list.iter().cloned().map(Item::new).collect(),
            Some(_) => return Err(NodeError::new("the parameter items must be a list")),
            None => vec![Item::new(json!({}))],
        };
        Ok(main_port(items))
    }
}

/// Sends each item to `true` or `false` by its parameter `condition`. A side no item takes
/// gives nothing, so a merge after it goes on with the other.
#[derive(Clone, Copy, Debug, Default)]
pub struct If;

impl NodeKind for If {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("if", 1)
            .title("If")
            .input(MAIN)
            .output("true")
            .output("false")
            .requires("condition")
            .chooses()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let condition = required(cx, "condition")?;
        let env = Environment::new();
        let (yes, no): (Vec<Item>, Vec<Item>) =
            cx.input(MAIN).iter().cloned().partition(|item| truthy(&work_out(&env, &condition, item)));
        Ok(Ports::from([("true".to_owned(), yes), ("false".to_owned(), no)]))
    }
}

/// Sends each item to the output named after the text of its `value`, one output per entry of
/// `cases`; an item matching no case goes to `otherwise`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Switch;

impl NodeKind for Switch {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("switch", 1)
            .title("Switch")
            .input(MAIN)
            .output("otherwise")
            .requires("value")
            .requires("cases")
            .chooses()
    }
    fn outputs(&self, node: &NodeDef) -> Vec<String> {
        let mut outputs: Vec<String> = match node.parameters.get("cases") {
            Some(Value::Array(cases)) => cases.iter().map(as_text).collect(),
            _ => Vec::new(),
        };
        outputs.push("otherwise".to_owned());
        outputs
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let value = required(cx, "value")?;
        let cases = self.outputs(cx.node());
        let env = Environment::new();
        let mut given: Ports = cases.iter().map(|c| (c.clone(), Vec::new())).collect();
        for item in cx.input(MAIN) {
            let said = as_text(&to_json(work_out(&env, &value, item)));
            let port = if cases[..cases.len() - 1].contains(&said) { said } else { "otherwise".to_owned() };
            given.entry(port).or_default().push(item.clone());
        }
        Ok(given)
    }
}

/// Joins the items of its inputs `a` and `b`, by its parameter `mode`:
///
/// - `append` (the default): every item of `a`, then every item of `b`, once both arrived;
/// - `position`: the items at the same place made one, `b`'s fields winning;
/// - `key`: each item of `a` made one with each item of `b` whose `key` gives the same value
///   (`key_a` and `key_b` when the two sides name it differently); `keep_unmatched` keeps the
///   items of `a` that found none;
/// - `first`: the items of the first input that has any.
#[derive(Clone, Copy, Debug, Default)]
pub struct Merge;

impl NodeKind for Merge {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("merge", 1).title("Merge").input("a").input("b").output(MAIN)
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let (a, b) = (cx.input("a"), cx.input("b"));
        let items = match text_parameter(cx, "mode", "append") {
            "append" => a.iter().chain(b).cloned().collect(),
            "first" => {
                if a.is_empty() {
                    b.to_vec()
                } else {
                    a.to_vec()
                }
            }
            "position" => (0..a.len().max(b.len()))
                .map(|i| match (a.get(i), b.get(i)) {
                    (Some(x), Some(y)) => Item::derive_all([x, y], merged(&x.json, &y.json)),
                    (Some(x), None) | (None, Some(x)) => x.clone(),
                    (None, None) => Item::default(),
                })
                .collect(),
            "key" => {
                let env = Environment::new();
                let key_a = expression(cx, "key_a")?.map_or_else(|| required(cx, "key"), Ok)?;
                let key_b = expression(cx, "key_b")?.map_or_else(|| required(cx, "key"), Ok)?;
                let keep = cx.parameter("keep_unmatched").and_then(Value::as_bool).unwrap_or(false);
                let mut by_key: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
                for y in b {
                    by_key.entry(as_text(&to_json(work_out(&env, &key_b, y)))).or_default().push(y);
                }
                let mut out = Vec::new();
                for x in a {
                    let key = as_text(&to_json(work_out(&env, &key_a, x)));
                    match by_key.get(&key) {
                        Some(matches) if !key.is_empty() => {
                            out.extend(
                                matches.iter().map(|y| Item::derive_all([x, *y], merged(&x.json, &y.json))),
                            );
                        }
                        _ if keep => out.push(x.clone()),
                        _ => {}
                    }
                }
                out
            }
            other => {
                return Err(NodeError::new(format!(
                    "the merge mode {other} is not append, position, key or first"
                )));
            }
        };
        Ok(main_port(items))
    }
}

/// A loop over its input in batches of `size` (1 by default). Each turn it sends the next
/// batch to `batch`, which leads to the loop's body; what the body sends back reaches its input
/// `back`. When no batch is left, it gives everything that came back on `done`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Batches;

impl NodeKind for Batches {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("batches", 1)
            .title("Loop in batches")
            .input(MAIN)
            .input("back")
            .output("batch")
            .output("done")
            .loops()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let size = match cx.parameter("size") {
            None => 1,
            Some(v) => v
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| NodeError::new("the batch size must be a whole number above 0"))?
                as usize,
        };
        let all = cx.input(MAIN);
        let start = cx.iteration() * size;
        if start < all.len() {
            let batch = all[start..(start + size).min(all.len())].to_vec();
            Ok(Ports::from([("batch".to_owned(), batch)]))
        } else {
            Ok(Ports::from([("done".to_owned(), cx.input("back").to_vec())]))
        }
    }
}

/// Sets fields on each item: its parameter `fields` maps each name to an expression worked out
/// over the item. With `keep_only`, the item keeps only those fields.
#[derive(Clone, Copy, Debug, Default)]
pub struct Set;

impl NodeKind for Set {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("set", 1).title("Set fields").input(MAIN).output(MAIN).requires("fields")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let Some(Value::Object(fields)) = cx.parameter("fields") else {
            return Err(NodeError::new("the parameter fields must map names to expressions"));
        };
        let mut parsed = Vec::with_capacity(fields.len());
        for (name, text) in fields {
            let text = text
                .as_str()
                .ok_or_else(|| NodeError::new(format!("the field {name} must be an expression, as text")))?;
            let criteria =
                Criteria::parse(text).map_err(|e| NodeError::new(format!("the field {name}: {e}")))?;
            parsed.push((name.clone(), criteria));
        }
        let keep_only = cx.parameter("keep_only").and_then(Value::as_bool).unwrap_or(false);
        let env = Environment::new();
        let made = cx
            .input(MAIN)
            .iter()
            .map(|item| {
                let mut map = match (&item.json, keep_only) {
                    (Value::Object(map), false) => map.clone(),
                    _ => Map::new(),
                };
                for (name, criteria) in &parsed {
                    map.insert(name.clone(), to_json(work_out(&env, criteria, item)));
                }
                item.derive(Value::Object(map))
            })
            .collect();
        Ok(main_port(made))
    }
}

/// Keeps the items whose `condition` is true on `main`, unchanged; the others go to `discarded`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Filter;

impl NodeKind for Filter {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("filter", 1)
            .title("Filter")
            .input(MAIN)
            .output(MAIN)
            .output("discarded")
            .requires("condition")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let condition = required(cx, "condition")?;
        let env = Environment::new();
        let (kept, out): (Vec<Item>, Vec<Item>) =
            cx.input(MAIN).iter().cloned().partition(|item| truthy(&work_out(&env, &condition, item)));
        Ok(Ports::from([(MAIN.to_owned(), kept), ("discarded".to_owned(), out)]))
    }
}

/// Puts the items in order by the fields of its parameter `by`: a list of field paths, or of
/// `{ "field": …, "descending": true }`. Items that tie keep the order they came in.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sort;

impl NodeKind for Sort {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("sort", 1).title("Sort").input(MAIN).output(MAIN).requires("by")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let Some(Value::Array(by)) = cx.parameter("by") else {
            return Err(NodeError::new("the parameter by must be a list of fields"));
        };
        let mut keys = Vec::new();
        for key in by {
            match key {
                Value::String(path) => keys.push((path.clone(), false)),
                Value::Object(o) => match o.get("field").and_then(Value::as_str) {
                    Some(path) => keys.push((
                        path.to_owned(),
                        o.get("descending").and_then(Value::as_bool).unwrap_or(false),
                    )),
                    None => return Err(NodeError::new("each sort key needs a field")),
                },
                _ => return Err(NodeError::new("each sort key is a field or { field, descending }")),
            }
        }
        let mut items = cx.input(MAIN).to_vec();
        items.sort_by(|x, y| {
            keys.iter()
                .map(|(path, down)| {
                    let order = compare(field(&x.json, path), field(&y.json, path));
                    if *down { order.reverse() } else { order }
                })
                .find(|o| *o != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        });
        Ok(main_port(items))
    }
}

/// One item per group of items with the same values of the fields in `by` (one group of all
/// with none), in the order each group first appears. Its parameter `aggregates` lists
/// `{ "field": …, "op": "sum" | "count" | "min" | "max" | "avg" | "list", "as": … }`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Group;

impl NodeKind for Group {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("group", 1).title("Group").input(MAIN).output(MAIN)
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let by: Vec<String> = match cx.parameter("by") {
            None => Vec::new(),
            Some(Value::Array(list)) => list.iter().map(as_text).collect(),
            Some(_) => return Err(NodeError::new("the parameter by must be a list of fields")),
        };
        let mut aggregates = Vec::new();
        if let Some(list) = cx.parameter("aggregates") {
            let Value::Array(list) = list else {
                return Err(NodeError::new("the parameter aggregates must be a list"));
            };
            for a in list {
                let op = a.get("op").and_then(Value::as_str).unwrap_or("count");
                if !["sum", "count", "min", "max", "avg", "list"].contains(&op) {
                    return Err(NodeError::new(format!(
                        "the aggregate {op} is not sum, count, min, max, avg or list"
                    )));
                }
                let path = a.get("field").and_then(Value::as_str).unwrap_or_default().to_owned();
                let name =
                    a.get("as").and_then(Value::as_str).map_or_else(|| format!("{op}_{path}"), str::to_owned);
                aggregates.push((op.to_owned(), path, name));
            }
        }
        let mut groups: Vec<(Vec<Value>, Vec<&Item>)> = Vec::new();
        for item in cx.input(MAIN) {
            let key: Vec<Value> =
                by.iter().map(|p| field(&item.json, p).cloned().unwrap_or(Value::Null)).collect();
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, members)) => members.push(item),
                None => groups.push((key, vec![item])),
            }
        }
        let made = groups
            .into_iter()
            .map(|(key, members)| {
                let mut map = Map::new();
                for (path, value) in by.iter().zip(key) {
                    map.insert(path.clone(), value);
                }
                for (op, path, name) in &aggregates {
                    let values: Vec<&Value> = members
                        .iter()
                        .filter_map(|m| field(&m.json, path))
                        .filter(|v| !v.is_null())
                        .collect();
                    let numbers: Vec<f64> = values.iter().filter_map(|v| v.as_f64()).collect();
                    let result = match op.as_str() {
                        "count" => json!(if path.is_empty() { members.len() } else { values.len() }),
                        "sum" => to_json(Criterion::Number(numbers.iter().sum())),
                        "avg" if numbers.is_empty() => Value::Null,
                        "avg" => json!(numbers.iter().sum::<f64>() / numbers.len() as f64),
                        "min" => values
                            .iter()
                            .copied()
                            .min_by(|x, y| compare(Some(x), Some(y)))
                            .cloned()
                            .unwrap_or(Value::Null),
                        "max" => values
                            .iter()
                            .copied()
                            .max_by(|x, y| compare(Some(x), Some(y)))
                            .cloned()
                            .unwrap_or(Value::Null),
                        _ => Value::Array(values.into_iter().cloned().collect()),
                    };
                    map.insert(name.clone(), result);
                }
                Item::derive_all(members, Value::Object(map))
            })
            .collect();
        Ok(main_port(made))
    }
}

/// One item per entry of the list in each item's `field`: an object entry is the new item, any
/// other is kept under the field's name. An item whose field is not a list goes to the error
/// output.
#[derive(Clone, Copy, Debug, Default)]
pub struct Split;

impl NodeKind for Split {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("split", 1).title("Split a list").input(MAIN).output(MAIN).requires("field")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let path = text_parameter(cx, "field", "");
        let name = path.rsplit('.').next().unwrap_or(path);
        let (mut made, mut failed) = (Vec::new(), Vec::new());
        for item in cx.input(MAIN) {
            match field(&item.json, path) {
                Some(Value::Array(entries)) => made.extend(entries.iter().map(|entry| match entry {
                    Value::Object(_) => item.derive(entry.clone()),
                    other => item.derive(json!({ name: other })),
                })),
                _ => failed.push(item.failed(&format!("{path} is not a list"))),
            }
        }
        Ok(Ports::from([(MAIN.to_owned(), made), (ERROR_PORT.to_owned(), failed)]))
    }
}

/// One item holding a list of all that came: the whole items, or only their `field`, under the
/// name `as` (`items` by default).
#[derive(Clone, Copy, Debug, Default)]
pub struct Join;

impl NodeKind for Join {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("join", 1).title("Join into a list").input(MAIN).output(MAIN)
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let name = text_parameter(cx, "as", "items").to_owned();
        let items = cx.input(MAIN);
        let list: Vec<Value> = match cx.parameter("field").and_then(Value::as_str) {
            Some(path) => items.iter().filter_map(|i| field(&i.json, path).cloned()).collect(),
            None => items.iter().map(|i| i.json.clone()).collect(),
        };
        Ok(main_port(vec![Item::derive_all(items, json!({ name: list }))]))
    }
}

/// Waits `ms` milliseconds on the run's clock, then passes its items on. It stops early when
/// the run is cancelled or its time is up. The wait is in memory: a wait that outlives the app
/// needs a durable run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wait;

impl NodeKind for Wait {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("wait", 1).title("Wait").input(MAIN).output(MAIN).requires("ms")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let ms = cx
            .parameter("ms")
            .and_then(Value::as_u64)
            .ok_or_else(|| NodeError::new("the wait must be a whole number of ms"))?;
        cx.sleep(ms)?;
        Ok(main_port(cx.input(MAIN).to_vec()))
    }
}

/// Runs another flow with the items that came, and gives what it ends with. Its parameter
/// `flow` names one of the flows it was made with; `input` names the node of that flow that
/// starts with the items (its first trigger by default) and `output` the node whose first
/// output it gives (the last to run by default). The flow it calls stops when this one is
/// cancelled, and has only the time this node has left.
#[derive(Clone, Debug)]
pub struct Subflow {
    flows: Arc<BTreeMap<String, FlowDefinition>>,
    catalog: Arc<Catalog>,
}

impl Subflow {
    /// A subflow node that may call any of `flows`, run with the kinds of `catalog`.
    pub fn new(flows: impl IntoIterator<Item = FlowDefinition>, catalog: Catalog) -> Self {
        Self {
            flows: Arc::new(flows.into_iter().map(|f| (f.id.clone(), f)).collect()),
            catalog: Arc::new(catalog),
        }
    }
}

impl NodeKind for Subflow {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("subflow", 1).title("Run a flow").input(MAIN).output(MAIN).requires("flow")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let id = text_parameter(cx, "flow", "");
        let flow = self.flows.get(id).ok_or_else(|| NodeError::new(format!("there is no flow {id}")))?;
        let plan = Plan::compile(flow, &self.catalog).map_err(|errors| {
            let first = errors.first().map(ToString::to_string).unwrap_or_default();
            NodeError::new(format!("the flow {id} does not compile: {first}"))
        })?;
        let input = match cx.parameter("input").and_then(Value::as_str) {
            Some(node) => node.to_owned(),
            None => plan
                .order()
                .iter()
                .find(|n| plan.spec(n).is_some_and(|s| s.trigger))
                .cloned()
                .ok_or_else(|| NodeError::new(format!("the flow {id} has no trigger to start from")))?,
        };
        let output = match cx.parameter("output").and_then(Value::as_str) {
            Some(node) => node.to_owned(),
            None => plan.order().last().cloned().unwrap_or_default(),
        };
        let mut executor = Executor::new(&plan, &self.catalog)
            .mode(cx.mode())
            .shared_clock(cx.clock().clone())
            .cancel_with(cx.token().child())
            .start_with(input, cx.input(MAIN).to_vec());
        if let Some(left) = cx.time_left_ms() {
            executor = executor.time_limit_ms(left.max(0) as u64);
        }
        let run = executor.run();
        match run.outcome {
            RunOutcome::Finished => {
                let port = plan.outputs(&output).first().cloned().unwrap_or_default();
                Ok(main_port(run.output(&output, &port).to_vec()))
            }
            RunOutcome::Cancelled { reason } => Err(NodeError::cancelled(reason)),
            RunOutcome::TimedOut { reason } => Err(NodeError::timed_out(reason)),
            RunOutcome::Stopped { node, reason } => {
                Err(NodeError::new(format!("the flow {id} stopped at {node}: {reason}")))
            }
        }
    }
}

impl Catalog {
    /// A catalog with every built-in kind: `manual`, `if`, `switch`, `merge`, `batches`, `set`,
    /// `filter`, `sort`, `group`, `split`, `join` and `wait`, each at version 1. A [`Subflow`]
    /// is added apart, with the flows it may call.
    pub fn builtin() -> Self {
        Self::new()
            .with(Manual)
            .with(If)
            .with(Switch)
            .with(Merge)
            .with(Batches)
            .with(Set)
            .with(Filter)
            .with(Sort)
            .with(Group)
            .with(Split)
            .with(Join)
            .with(Wait)
    }
}
