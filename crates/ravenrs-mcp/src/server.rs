//! The MCP tools. Each one is read-only and answers from embedded data.

use std::collections::BTreeSet;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{Index, Item, Member, Usage, code_blocks_with};
use crate::docs;
use crate::expression::{self, SchemaInput};

/// Members listed per list in one answer, unless a filter narrows them down.
const MAX_MEMBERS: usize = 60;

const INSTRUCTIONS: &str = "Answers questions about the RavenRS GUI framework for Rust, from the sources and docs \
of the version this server was built from. Check component_api and component_examples before writing a view \
instead of guessing builder methods. Use design_tokens to find the semantic name of a color or text style, \
parity_status before promising that a feature exists, and validate_expression for filters and report expressions.";

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct ListComponents {
    /// Only this group, such as Inputs, Navigation or Data.
    group: Option<String>,
    /// Only components whose name or summary contains this text.
    query: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ComponentApi {
    /// A component or any other public type, trait or function: Button, ViewExt, State, Semantic.
    name: String,
    /// Only fields, variants and methods whose name contains this text.
    filter: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ComponentExamples {
    /// A component or other public type, such as DataGrid.
    name: String,
    /// How many examples to return, 1 to 10. Defaults to 3.
    limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct DesignTokens {
    /// One of color, typography, spacing, radius, shadow, duration or spring.
    category: Option<String>,
    /// Only tokens whose name, value or usage contains this text.
    query: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct ParityStatus {
    /// components (docs/PARITY.md) or reporting (docs/REPORTING-PARITY.md). Defaults to both.
    document: Option<String>,
    /// Only rows whose section or text contains this, such as "barcode" or "tree".
    query: Option<String>,
    /// Only rows with this status: done, partial or planned.
    status: Option<String>,
    /// How many rows to return, up to 200. Defaults to 50.
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ValidateExpression {
    /// The expression, such as `[Price] * [Qty] > ?Minimum`.
    expression: String,
    /// When given, also checks that its fields, parameters, collections and functions exist.
    schema: Option<SchemaInput>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct ExpressionFunctions {
    /// One of Logical, String, Math, DateTime, Reporting or Aggregate.
    category: Option<String>,
    /// Only functions whose signature or description contains this text.
    query: Option<String>,
}

/// The server. It holds no state: the data is static and parsed once, on first use.
#[derive(Debug, Clone, Default)]
pub struct RavenServer;

impl RavenServer {
    pub fn new() -> Self {
        Self
    }
}

#[tool_router]
impl RavenServer {
    #[tool(
        description = "List RavenRS components with their group and one-line summary.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn list_components(&self, Parameters(input): Parameters<ListComponents>) -> CallToolResult {
        let query = input.query.map(|query| query.to_lowercase());
        let mut groups = BTreeSet::new();
        let components: Vec<Value> = Index::get()
            .components()
            .into_iter()
            .map(|item| (docs::component_group(&item.name), item))
            .inspect(|(group, _)| {
                groups.insert(group.clone());
            })
            .filter(|(group, _)| input.group.as_ref().is_none_or(|wanted| group.eq_ignore_ascii_case(wanted)))
            .filter(|(_, item)| {
                query.as_ref().is_none_or(|query| {
                    item.name.to_lowercase().contains(query)
                        || summary(&item.docs).to_lowercase().contains(query)
                })
            })
            .map(|(group, item)| {
                json!({
                    "name": item.name,
                    "group": group,
                    "summary": summary(&item.docs),
                    "source": location(&item.file, item.line),
                })
            })
            .collect();
        CallToolResult::structured(json!({
            "ravenrs_version": env!("CARGO_PKG_VERSION"),
            "total": components.len(),
            "groups": groups,
            "components": components,
        }))
    }

    #[tool(
        description = "Show the public API of a component or any other public RavenRS item: docs, constructors and builder methods with their signatures, fields, enum variants, implemented traits, source location and, for components, the quality audit.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn component_api(&self, Parameters(input): Parameters<ComponentApi>) -> CallToolResult {
        let items = Index::get().find(input.name.trim());
        if items.is_empty() {
            return not_found(&input.name);
        }
        let filter = input.filter.map(|filter| filter.to_lowercase());
        let matches: Vec<Value> = items.into_iter().map(|item| describe(item, filter.as_deref())).collect();
        CallToolResult::structured(json!({
            "name": input.name,
            "matches": matches,
            "note": "Every view also takes the ViewExt modifiers, such as padding, background and frame. See component_api with name ViewExt.",
        }))
    }

    #[tool(
        description = "Show real code that uses a component or type, taken from the RavenRS examples, its rustdoc and the getting started guide. Examples are picked to cover as many builder methods as possible.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn component_examples(&self, Parameters(input): Parameters<ComponentExamples>) -> CallToolResult {
        let index = Index::get();
        let items = index.find(input.name.trim());
        let Some(name) = items.first().map(|item| item.name.as_str()) else {
            return not_found(&input.name);
        };
        let limit = input.limit.unwrap_or(3).clamp(1, 10);

        let usages: Vec<&Usage> = index.usages.iter().filter(|usage| usage.root == name).collect();
        let found = usages.len();
        let mut examples: Vec<Value> = pick_varied(usages, limit)
            .into_iter()
            .map(|usage| {
                json!({ "kind": "example", "source": location(&usage.file, usage.line), "code": usage.code })
            })
            .collect();
        for item in &items {
            for code in code_blocks_with(&item.docs, name) {
                examples.push(
                    json!({ "kind": "rustdoc", "source": location(&item.file, item.line), "code": code }),
                );
            }
        }
        for code in code_blocks_with(docs::GETTING_STARTED, name) {
            examples.push(json!({ "kind": "guide", "source": "docs/GETTING_STARTED.md", "code": code }));
        }
        examples.truncate(limit);
        CallToolResult::structured(json!({
            "name": name,
            "usages_in_examples": found,
            "examples": examples,
        }))
    }

    #[tool(
        description = "Look up design tokens from docs/design-system/tokens.json: colors per theme, type scale, spacing, radii, shadows, durations and springs, with what each is for and its Rust name, such as Semantic::Ink or TextStyle::Title.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn design_tokens(&self, Parameters(input): Parameters<DesignTokens>) -> CallToolResult {
        answer(docs::tokens(input.category.as_deref(), input.query.as_deref()))
    }

    #[tool(
        description = "Check whether a feature exists: searches the parity trackers against shadcn/ui, DevExpress and Telerik (components) and DevExpress Reporting (reporting), and returns each row's status (done, partial or planned) with counts.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn parity_status(&self, Parameters(input): Parameters<ParityStatus>) -> CallToolResult {
        answer(docs::parity(
            input.document.as_deref(),
            input.query.as_deref(),
            input.status.as_deref(),
            input.limit.unwrap_or(50).clamp(1, 200),
        ))
    }

    #[tool(
        description = "Check an expression in the ravenrs-criteria language without running it: syntax errors with their offset, the normalized text, the fields and parameters it uses and, given a schema, unknown names.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn validate_expression(&self, Parameters(input): Parameters<ValidateExpression>) -> CallToolResult {
        answer(expression::check(&input.expression, input.schema.as_ref()))
    }

    #[tool(
        description = "Search the functions and aggregates of the ravenrs-criteria expression language, with signatures and descriptions.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    fn expression_functions(&self, Parameters(input): Parameters<ExpressionFunctions>) -> CallToolResult {
        CallToolResult::structured(expression::functions(input.category.as_deref(), input.query.as_deref()))
    }
}

#[tool_handler]
impl ServerHandler for RavenServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("ravenrs-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

fn answer(result: Result<Value, String>) -> CallToolResult {
    match result {
        Ok(value) => CallToolResult::structured(value),
        Err(message) => CallToolResult::structured_error(json!({ "error": message })),
    }
}

fn not_found(name: &str) -> CallToolResult {
    CallToolResult::structured_error(json!({
        "error": format!("no public item named '{name}'"),
        "did_you_mean": Index::get().suggest(name.trim()),
    }))
}

fn location(file: &str, line: usize) -> String {
    format!("{file}:{line}")
}

/// The first paragraph of some docs, on one line.
fn summary(docs: &str) -> String {
    let paragraph = docs.split("\n\n").next().unwrap_or_default();
    paragraph.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn describe(item: &Item, filter: Option<&str>) -> Value {
    let mut value = json!({
        "crate": item.krate,
        "kind": item.kind,
        "signature": item.signature,
        "source": location(&item.file, item.line),
        "docs": item.docs,
    });
    if item.non_exhaustive {
        value["non_exhaustive"] = json!(true);
    }
    if !item.traits.is_empty() {
        value["traits"] = json!(item.traits);
    }
    if !item.fields.is_empty() {
        value["fields"] = members(&item.fields, &item.file, filter);
    }
    if !item.variants.is_empty() {
        value["variants"] = members(&item.variants, &item.file, filter);
    }
    if !item.methods.is_empty() {
        value["methods"] = members(&item.methods, &item.file, filter);
    }
    if item.krate == "ravenrs-widgets"
        && let Some(quality) = docs::quality_row(&item.name)
    {
        value["quality"] = quality;
    }
    value
}

fn members(members: &[Member], file: &str, filter: Option<&str>) -> Value {
    let matching: Vec<&Member> = members
        .iter()
        .filter(|member| filter.is_none_or(|filter| member.name.to_lowercase().contains(filter)))
        .collect();
    let items: Vec<Value> = matching
        .iter()
        .take(MAX_MEMBERS)
        .map(|member| {
            json!({
                "signature": member.signature,
                "docs": member.docs,
                "source": location(file, member.line),
            })
        })
        .collect();
    let mut value = json!({ "total": matching.len(), "items": items });
    if matching.len() > MAX_MEMBERS {
        value["hint"] = json!("Too many to list. Pass `filter` to narrow them down.");
    }
    value
}

/// Picks usages that together show the most methods, preferring short ones.
fn pick_varied(mut usages: Vec<&Usage>, limit: usize) -> Vec<&Usage> {
    usages.sort_by_key(|usage| usage.code.len());
    let mut seen = BTreeSet::new();
    let mut picked = Vec::new();
    while picked.len() < limit && !usages.is_empty() {
        let new_methods =
            |usage: &&Usage| usage.methods.iter().filter(|method| !seen.contains(*method)).count();
        // On a tie, `max_by_key` would take the last and longest, so walk from the short end.
        let best = usages
            .iter()
            .enumerate()
            .rev()
            .max_by_key(|(_, usage)| new_methods(usage))
            .map_or(0, |(position, _)| position);
        let usage = usages.remove(best);
        seen.extend(usage.methods.iter().cloned());
        picked.push(usage);
    }
    picked
}
