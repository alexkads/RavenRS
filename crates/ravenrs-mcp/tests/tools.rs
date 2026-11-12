//! Drives the real binary over stdio, as an MCP client does, and exercises every tool.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test helpers fail loudly")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Client {
    fn start() -> Self {
        Self::start_with("2025-11-25").0
    }

    fn start_with(protocol: &str) -> (Self, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ravenrs-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the server starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut client = Self { child, stdin, stdout, next_id: 0 };
        let init = client.request(
            "initialize",
            json!({
                "protocolVersion": protocol,
                "capabilities": {},
                "clientInfo": { "name": "tests", "version": "0" },
            }),
        );
        client.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        (client, init)
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(self.stdout.read_line(&mut line).unwrap() > 0, "the server closed stdout");
            let message: Value = serde_json::from_str(&line).expect("stdout carries only JSON-RPC");
            if message["id"] == id {
                assert!(message["error"].is_null(), "protocol error: {message}");
                return message["result"].clone();
            }
        }
    }

    /// Calls a tool that must succeed.
    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.request("tools/call", json!({ "name": tool, "arguments": arguments }));
        assert_eq!(result["isError"], json!(false), "{tool} failed: {result}");
        assert_eq!(
            serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
            result["structuredContent"],
            "text and structured content carry the same answer"
        );
        result["structuredContent"].clone()
    }

    /// Calls a tool that must report a failure to the model.
    fn call_err(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.request("tools/call", json!({ "name": tool, "arguments": arguments }));
        assert_eq!(result["isError"], json!(true), "{tool} should fail: {result}");
        result["structuredContent"].clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn names(list: &Value) -> Vec<&str> {
    list.as_array().unwrap().iter().map(|entry| entry["name"].as_str().unwrap()).collect()
}

#[test]
fn announces_every_tool_as_read_only() {
    let (mut client, init) = Client::start_with("2025-11-25");
    assert_eq!(init["serverInfo"]["name"], "ravenrs-mcp");
    assert_eq!(init["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(init["instructions"].as_str().unwrap().contains("component_api"));

    let tools = client.request("tools/list", json!({}));
    let mut listed = names(&tools["tools"]);
    listed.sort_unstable();
    assert_eq!(
        listed,
        [
            "app_contracts",
            "check_report_layout",
            "component_api",
            "component_examples",
            "create_report_layout",
            "design_tokens",
            "expression_functions",
            "list_components",
            "parity_status",
            "report_layout_format",
            "report_layout_templates",
            "validate_expression",
        ]
    );
    for tool in tools["tools"].as_array().unwrap() {
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
        assert!(tool["description"].as_str().is_some_and(|text| !text.is_empty()), "{tool}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
}

#[test]
fn the_plugin_matches_the_server() {
    let skill = include_str!("../../../plugins/ravenrs/skills/ravenrs/SKILL.md");
    let mut client = Client::start();
    let tools = client.request("tools/list", json!({}));
    for tool in names(&tools["tools"]) {
        assert!(skill.contains(&format!("`{tool}`")), "the skill does not mention {tool}");
    }

    let config: Value = serde_json::from_str(include_str!("../../../plugins/ravenrs/.mcp.json")).unwrap();
    assert_eq!(config["mcpServers"]["ravenrs"]["command"], "cargo");
    assert_eq!(config["mcpServers"]["ravenrs"]["args"], json!(["ravenrs", "mcp"]));
}

#[test]
fn negotiates_an_older_protocol() {
    let (_client, init) = Client::start_with("2025-06-18");
    assert_eq!(init["protocolVersion"], "2025-06-18");
}

#[test]
fn list_components_filters_by_group_and_text() {
    let mut client = Client::start();
    let all = client.call("list_components", json!({}));
    assert_eq!(all["ravenrs_version"], env!("CARGO_PKG_VERSION"));
    assert!(all["total"].as_u64().unwrap() >= 60, "{}", all["total"]);
    let listed = names(&all["components"]);
    for expected in ["Button", "DataGrid", "Ribbon", "VStack", "Dialog"] {
        assert!(listed.contains(&expected), "missing {expected}");
    }
    assert!(!listed.contains(&"Styled"), "view plumbing is not a component");
    assert!(!listed.contains(&"BuildCx"), "support types are not components");
    assert!(all["groups"].as_array().unwrap().contains(&json!("Navigation")));

    let inputs = client.call("list_components", json!({ "group": "text and choices" }));
    let inputs = names(&inputs["components"]);
    assert!(inputs.contains(&"Slider"));
    assert!(!inputs.contains(&"Ribbon"));

    let grids = client.call("list_components", json!({ "query": "datagrid" }));
    assert!(names(&grids["components"]).contains(&"DataGrid"));
}

#[test]
fn component_api_shows_signatures_docs_and_quality() {
    let mut client = Client::start();
    let answer = client.call("component_api", json!({ "name": "button" }));
    let button = &answer["matches"][0];
    assert_eq!(button["crate"], "ravenrs-widgets");
    assert_eq!(button["kind"], "struct");
    assert!(button["source"].as_str().unwrap().starts_with("crates/ravenrs-widgets/src/button.rs:"));
    assert!(button["traits"].as_array().unwrap().contains(&json!("View")));
    assert!(button["quality"]["Keyboard"].as_str().unwrap().contains("Enter"));
    let methods = &button["methods"]["items"];
    let signatures: Vec<&str> =
        methods.as_array().unwrap().iter().map(|m| m["signature"].as_str().unwrap()).collect();
    assert!(signatures.iter().any(|s| s.starts_with("pub fn new(")), "{signatures:?}");
    assert!(signatures.iter().any(|s| s.starts_with("pub fn on_click")), "{signatures:?}");

    // Traits, enums and other crates work too.
    let view_ext = client.call("component_api", json!({ "name": "ViewExt" }));
    assert_eq!(view_ext["matches"][0]["kind"], "trait");
    let semantic = client.call("component_api", json!({ "name": "Semantic" }));
    assert_eq!(semantic["matches"][0]["crate"], "ravenrs-style");
    assert_eq!(semantic["matches"][0]["non_exhaustive"], true);
    assert_eq!(semantic["matches"][0]["variants"]["items"][0]["signature"], "Canvas");
    // Defined by a macro in the source.
    let stack = client.call("component_api", json!({ "name": "VStack" }));
    assert!(stack["matches"][0]["docs"].as_str().unwrap().len() > 10, "{stack}");
    assert_eq!(stack["matches"][0]["signature"], "pub struct VStack<C>");
    assert!(stack["matches"][0]["traits"].as_array().unwrap().contains(&json!("View")));
    let state = client.call("component_api", json!({ "name": "State" }));
    assert_eq!(state["matches"][0]["crate"], "ravenrs-reactive");
}

#[test]
fn component_api_narrows_long_lists_and_suggests_names() {
    let mut client = Client::start();
    let icons = client.call("component_api", json!({ "name": "IconName" }));
    let variants = &icons["matches"][0]["variants"];
    assert!(variants["total"].as_u64().unwrap() >= 200);
    assert_eq!(variants["items"].as_array().unwrap().len(), 60);
    assert!(variants["hint"].is_string());

    let search = client.call("component_api", json!({ "name": "IconName", "filter": "search" }));
    assert_eq!(search["matches"][0]["variants"]["items"][0]["signature"], "Search");

    let typo = client.call_err("component_api", json!({ "name": "Buton" }));
    assert!(typo["did_you_mean"].as_array().unwrap().contains(&json!("Button")));
}

#[test]
fn component_examples_come_from_real_code() {
    let mut client = Client::start();
    let answer = client.call("component_examples", json!({ "name": "Button" }));
    assert_eq!(answer["name"], "Button");
    assert!(answer["usages_in_examples"].as_u64().unwrap() > 3);
    let examples = answer["examples"].as_array().unwrap();
    assert_eq!(examples.len(), 3);
    for example in examples {
        assert_eq!(example["kind"], "example");
        assert!(example["source"].as_str().unwrap().starts_with("examples/"));
        assert!(example["code"].as_str().unwrap().contains("Button::"), "{example}");
    }

    let one = client.call("component_examples", json!({ "name": "Slider", "limit": 1 }));
    assert_eq!(one["examples"].as_array().unwrap().len(), 1);

    // A type with no chain in the examples falls back to its rustdoc.
    let chip = client.call("component_examples", json!({ "name": "ViewExt", "limit": 10 }));
    assert!(chip["examples"].as_array().unwrap().iter().any(|e| e["kind"] == "rustdoc"), "{chip}");

    let missing = client.call_err("component_examples", json!({ "name": "NoSuchThing" }));
    assert!(missing["error"].as_str().unwrap().contains("NoSuchThing"));
}

#[test]
fn design_tokens_map_to_rust_names() {
    let mut client = Client::start();
    let colors = client.call("design_tokens", json!({ "category": "color", "query": "ink-muted" }));
    let token = &colors["tokens"][0];
    assert_eq!(token["name"], "ink-muted");
    assert_eq!(token["rust"], "Semantic::InkMuted");
    assert!(token["value"]["dark"].as_str().unwrap().starts_with('#'));
    assert!(colors["themes"].as_array().unwrap().len() >= 3);

    let styles = client.call("design_tokens", json!({ "category": "type", "query": "title" }));
    let styles = styles["tokens"].as_array().unwrap();
    let title = styles.iter().find(|token| token["name"] == "title").expect("the title style");
    assert_eq!(title["rust"], "TextStyle::Title");

    let all = client.call("design_tokens", json!({}));
    for category in ["color", "typography", "spacing", "radius", "shadow", "duration", "spring"] {
        assert!(
            all["tokens"].as_array().unwrap().iter().any(|t| t["category"] == category),
            "nothing in {category}"
        );
    }

    let bad = client.call_err("design_tokens", json!({ "category": "sound" }));
    assert!(bad["error"].as_str().unwrap().contains("spacing"));
}

#[test]
fn parity_status_searches_both_documents() {
    let mut client = Client::start();
    let tabs = client.call("parity_status", json!({ "document": "components", "query": "tabstrip" }));
    assert_eq!(tabs["rows"][0]["status"], "done");
    assert_eq!(tabs["rows"][0]["document"], "components");
    assert_eq!(tabs["rows"][0]["section"], "Navigation");

    let barcode = client.call("parity_status", json!({ "document": "reporting", "query": "barcode" }));
    assert_eq!(barcode["rows"][0]["document"], "reporting");
    assert_eq!(barcode["rows"][0]["status"], "partial");
    assert!(barcode["rows"][0]["reference"]["Reference"].is_string());

    let planned = client.call("parity_status", json!({ "status": "planned", "limit": 2 }));
    assert_eq!(planned["shown"], 2);
    assert!(planned["total"].as_u64().unwrap() > 2);
    assert_eq!(planned["done"], 0);

    assert!(client.call_err("parity_status", json!({ "status": "maybe" }))["error"].is_string());
    assert!(
        client.call_err("parity_status", json!({ "document": "other" }))["error"]
            .as_str()
            .unwrap()
            .contains("application")
    );
}

#[test]
fn parity_status_reads_the_application_tracker_with_its_issue() {
    let mut client = Client::start();
    let manifest = client.call("parity_status", json!({ "document": "application", "query": "manifest" }));
    assert_eq!(manifest["rows"][0]["document"], "application");
    assert_eq!(manifest["rows"][0]["status"], "partial");
    assert!(manifest["rows"][0]["ravenrs"].as_str().unwrap().contains("ModuleManifest"));
    assert_eq!(manifest["rows"][0]["reference"]["Step"], "#271");

    let both = client.call("parity_status", json!({ "query": "manifest" }));
    assert!(both["rows"].as_array().unwrap().iter().all(|row| row["document"] != "application"));
}

#[test]
fn app_contracts_lists_scenarios_and_groups_with_their_owners() {
    let mut client = Client::start();
    let all = client.call("app_contracts", json!({}));
    assert_eq!(all["scenarios"].as_array().unwrap().len(), 19);
    assert_eq!(all["groups"].as_array().unwrap().len(), 19);
    assert!(
        all["groups"].as_array().unwrap().iter().all(|group| !group["owner"].as_str().unwrap().is_empty())
    );

    let events = client.call("app_contracts", json!({ "query": "C07" }));
    assert_eq!(events["scenarios"].as_array().unwrap().len(), 1);
    assert!(events["scenarios"][0]["owner"].as_str().unwrap().contains("#254"));
}

#[test]
fn validate_expression_reports_syntax_and_schema_problems() {
    let mut client = Client::start();
    let good = client.call("validate_expression", json!({ "expression": "[Price]*2 > ?Min" }));
    assert_eq!(good["valid"], true);
    assert_eq!(good["fields"], json!(["Price"]));
    assert_eq!(good["parameters"], json!(["Min"]));
    assert!(good["normalized"].is_string());

    let broken = client.call("validate_expression", json!({ "expression": "[Price] >" }));
    assert_eq!(broken["valid"], false);
    assert!(broken["error"]["offset"].is_u64());

    let schema = json!({
        "fields": { "Price": "number" },
        "parameters": { "Min": "number" },
        "collections": { "Items": { "fields": { "Qty": "number" } } },
    });
    let known = client.call(
        "validate_expression",
        json!({ "expression": "[Price] > ?Min And [Items][].Sum([Qty]) > 0", "schema": schema }),
    );
    assert_eq!(known["valid"], true, "{known}");

    let unknown = client.call(
        "validate_expression",
        json!({ "expression": "[Prize] > ?Min", "schema": { "fields": { "Price": "number" }, "parameters": { "Min": "any" } } }),
    );
    assert_eq!(unknown["valid"], false);
    assert!(unknown["problems"][0].as_str().unwrap().contains("Prize"), "{unknown}");

    // Hostile nesting is an answer, not a crash.
    let deep = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
    assert_eq!(client.call("validate_expression", json!({ "expression": deep }))["valid"], false);

    let bad_type = client.call_err(
        "validate_expression",
        json!({ "expression": "1", "schema": { "fields": { "A": "colour" } } }),
    );
    assert!(bad_type["error"].as_str().unwrap().contains("colour"));
}

#[test]
fn expression_functions_search_the_catalog() {
    let mut client = Client::start();
    let all = client.call("expression_functions", json!({}));
    assert!(all["total"].as_u64().unwrap() > 100);

    let rounding = client.call("expression_functions", json!({ "query": "round" }));
    assert!(names(&rounding["functions"]).contains(&"Round"));

    let aggregates = client.call("expression_functions", json!({ "category": "aggregate" }));
    assert!(names(&aggregates["functions"]).contains(&"Sum"));
    assert!(aggregates["functions"].as_array().unwrap().iter().all(|f| f["category"] == "Aggregate"));
}

#[test]
fn report_layout_format_shows_every_control_and_a_full_example() {
    let mut client = Client::start();
    let format = client.call("report_layout_format", json!({}));
    for control in ["label", "field", "table", "barcode", "chart", "checkBox"] {
        assert!(format["controls"][control].is_object(), "{control}: {format}");
    }
    assert_eq!(format["controls"]["chart"]["type"], "chart");
    let example = &format["example"];
    assert_eq!(example["name"], "Orders");
    let check = client.call("check_report_layout", json!({ "layout": example }));
    assert_eq!(check["valid"], true, "the example is a working layout: {check}");
}

#[test]
fn create_report_layout_builds_a_grouped_table_that_renders() {
    let mut client = Client::start();
    let rows: Vec<Value> = (0..40)
        .map(|i| json!({ "Region": if i % 2 == 0 { "North" } else { "South" }, "Customer": format!("C{i}"), "Amount": i * 10, "Paid": i % 3 == 0 }))
        .collect();
    let made = client.call(
        "create_report_layout",
        json!({
            "title": "Vendas por região",
            "culture": "pt-BR",
            "data": rows,
            "columns": [{ "field": "Customer", "caption": "Cliente" }, { "field": "Amount", "format": "C2" }],
            "group_by": "Region",
            "totals": ["Amount"],
        }),
    );
    assert_eq!(made["file_name"], "vendas-por-região.rvreport.json");
    assert_eq!(made["layout"]["name"], "Vendas por região");
    assert_eq!(made["layout"]["culture"], "pt-BR");
    assert_eq!(made["check"]["valid"], true, "{}", made["check"]);
    assert!(made["check"]["render"]["pages"].as_u64().unwrap() >= 1);
    assert!(made["check"]["expressions_checked"].as_u64().unwrap() > 3);
    let text = made["layout"].to_string().to_lowercase();
    assert!(text.contains("cliente"), "the caption given: {text}");
    assert!(text.contains("sum([amount])"), "the totals: {text}");

    let wrong = client
        .call_err("create_report_layout", json!({ "title": "X", "data": [{ "A": 1 }], "group_by": "B" }));
    assert!(wrong["error"].as_str().unwrap().contains("the fields are: A"), "{wrong}");
    let not_number = client
        .call_err("create_report_layout", json!({ "title": "X", "data": [{ "A": "a" }], "totals": ["A"] }));
    assert!(not_number["error"].as_str().unwrap().contains("not a number"), "{not_number}");
}

#[test]
fn create_report_layout_takes_fields_and_a_url_for_the_server() {
    let mut client = Client::start();
    let made = client.call(
        "create_report_layout",
        json!({
            "title": "Pedidos",
            "fields": [{ "name": "Numero", "kind": "number" }, { "name": "Cliente" }, { "name": "Data", "kind": "date" }],
            "data_url": "https://erp.example.com/pedidos?ano={?Ano}",
            "landscape": true,
        }),
    );
    let source = &made["layout"]["dataSources"][0];
    assert_eq!(source["type"], "url");
    assert_eq!(source["url"], "https://erp.example.com/pedidos?ano={?Ano}");
    assert!(
        made["layout"]["page"]["width"].as_f64() > made["layout"]["page"]["height"].as_f64(),
        "landscape"
    );
    assert!(made["check"]["render"]["skipped"].is_string(), "{}", made["check"]);
}

#[test]
fn check_report_layout_finds_json_and_expression_errors() {
    let mut client = Client::start();
    let broken = client.call("check_report_layout", json!({ "layout": "{\n  \"name\": \"X\",\n  oops\n}" }));
    assert_eq!(broken["valid"], false);
    assert_eq!(broken["json_error"]["line"], 3, "{broken}");

    let mut layout =
        client.call("report_layout_templates", json!({ "name": "sales_by_region" }))["layout"].clone();
    assert_eq!(client.call("check_report_layout", json!({ "layout": layout.clone() }))["valid"], true);
    layout["filter"] = json!("[Totl] >= ?MinTotal");
    let checked = client.call("check_report_layout", json!({ "layout": layout.clone() }));
    let warning = &checked["warnings"][0];
    assert_eq!(warning["at"], "filter", "{checked}");
    assert!(warning["unknown"][0].as_str().unwrap().contains("Totl"), "{checked}");
    layout["filter"] = json!("[Total] >= (");
    let syntax = client.call("check_report_layout", json!({ "layout": layout }));
    assert_eq!(syntax["valid"], false);
    assert_eq!(syntax["problems"][0]["at"], "filter", "{syntax}");
    assert!(syntax["problems"][0]["error"].is_string(), "{syntax}");
}

#[test]
fn report_layout_templates_list_and_every_one_checks() {
    let mut client = Client::start();
    let list = client.call("report_layout_templates", json!({}));
    let all = names(&list["templates"]).iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    assert_eq!(all, ["invoice", "sales_by_region", "sales_dashboard", "customer_list"]);
    for name in all {
        let one = client.call("report_layout_templates", json!({ "name": name }));
        let check = client.call("check_report_layout", json!({ "layout": one["layout"] }));
        assert_eq!(check["valid"], true, "{name}: {check}");
    }
    let missing = client.call_err("report_layout_templates", json!({ "name": "nope" }));
    assert!(missing["error"].as_str().unwrap().contains("invoice"));
}
