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
            "component_api",
            "component_examples",
            "design_tokens",
            "expression_functions",
            "guides",
            "list_components",
            "parity_status",
            "search_api",
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
    assert_eq!(config["mcpServers"]["ravenrs"]["command"], env!("CARGO_PKG_NAME"));
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
fn parity_status_searches_every_document() {
    let mut client = Client::start();
    let tabs = client.call("parity_status", json!({ "document": "components", "query": "tabstrip" }));
    assert_eq!(tabs["rows"][0]["status"], "done");
    assert_eq!(tabs["rows"][0]["document"], "components");
    assert_eq!(tabs["rows"][0]["section"], "Navigation");

    let barcode = client.call("parity_status", json!({ "document": "reporting", "query": "barcode" }));
    assert_eq!(barcode["rows"][0]["document"], "reporting");
    assert_eq!(barcode["rows"][0]["status"], "partial");
    assert!(barcode["rows"][0]["reference"]["DevExpress"].is_string());

    let planned = client.call("parity_status", json!({ "status": "planned", "limit": 2 }));
    assert_eq!(planned["shown"], 2);
    assert!(planned["total"].as_u64().unwrap() > 2);
    assert_eq!(planned["done"], 0);

    let singleton =
        client.call("parity_status", json!({ "document": "application", "query": "one of a kind" }));
    assert_eq!(singleton["rows"][0]["document"], "application");
    assert_eq!(singleton["rows"][0]["status"], "done");
    assert!(singleton["rows"][0]["ravenrs"].as_str().unwrap().contains("singleton"), "{singleton}");

    assert!(client.call_err("parity_status", json!({ "status": "maybe" }))["error"].is_string());
    assert!(client.call_err("parity_status", json!({ "document": "other" }))["error"].is_string());
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

fn found_in(answer: &Value, name: &str) -> Value {
    let items = answer["items"].as_array().unwrap();
    items.iter().find(|item| item["name"] == name).cloned().unwrap_or_else(|| panic!("no {name} in {answer}"))
}

#[test]
fn the_api_of_the_counter_the_notes_and_the_printers_can_be_found() {
    let mut client = Client::start();
    let cases = [
        ("ScanField", "ravenrs-widgets"),
        ("KeyBar", "ravenrs-widgets"),
        ("SefazSandbox", "ravenrs-nfe"),
        ("LabelJob", "ravenrs-report-label"),
        ("Carrinho", "ravenrs-pos"),
        ("NfeModule", "ravenrs-app-nfe"),
        ("Pos", "ravenrs-app-pos"),
        ("Symbology", "ravenrs-barcode"),
        ("PrinterCommand", "ravenrs-report"),
    ];
    for (name, krate) in cases {
        let by_name = client.call("component_api", json!({ "name": name }));
        let matches = by_name["matches"].as_array().unwrap();
        assert!(matches.iter().any(|item| item["crate"] == krate), "{name} in {krate}: {by_name}");
        let search = client.call("search_api", json!({ "query": name, "limit": 100 }));
        assert_eq!(found_in(&search, name)["crate"], krate, "{search}");
    }

    let listed = client.call("list_components", json!({ "query": "ScanField" }));
    assert_eq!(listed["components"][0]["name"], "ScanField");
    let audited = client.call("component_api", json!({ "name": "ScanField" }));
    assert!(audited["matches"][0]["quality"].is_object(), "{audited}");
}

#[test]
fn search_api_finds_what_has_no_known_name() {
    let mut client = Client::start();
    let sefaz = client.call("search_api", json!({ "query": "sefaz sandbox" }));
    assert!(sefaz["total"].as_u64().unwrap() > 0, "{sefaz}");
    let only = client.call("search_api", json!({ "query": "language", "krate": "ravenrs-report-label" }));
    for item in only["items"].as_array().unwrap() {
        assert_eq!(item["crate"], "ravenrs-report-label");
    }
    assert!(only["total"].as_u64().unwrap() > 0, "{only}");
    let none = client.call("search_api", json!({ "query": "no such thing anywhere" }));
    assert_eq!(none["total"], 0);
}

#[test]
fn an_example_can_come_from_the_guide_of_a_crate() {
    let mut client = Client::start();
    let answer = client.call("component_examples", json!({ "name": "SefazSandbox", "limit": 10 }));
    let examples = answer["examples"].as_array().unwrap();
    assert!(!examples.is_empty(), "{answer}");
    assert!(examples.iter().all(|example| example["code"].as_str().unwrap().contains("SefazSandbox")));
}

#[test]
fn guides_list_show_and_search_the_documents() {
    let mut client = Client::start();
    let all = client.call("guides", json!({}));
    let paths: Vec<&str> =
        all["documents"].as_array().unwrap().iter().map(|d| d["document"].as_str().unwrap()).collect();
    for wanted in [
        "README.md",
        "ARCHITECTURE.md",
        "docs/APP-FRAMEWORK.md",
        "docs/AUTOMACAO-COMERCIAL.md",
        "docs/NFSE.md",
        "docs/REPORTING.md",
        "crates/ravenrs-nfe/README.md",
        "crates/ravenrs-nfe/REGRAS.md",
        "crates/ravenrs-report-label/README.md",
        "crates/ravenrs-report-label/FONTES.md",
        "crates/ravenrs-pos/README.md",
        "crates/ravenrs-app-nfe/README.md",
        "examples/mini-pdv/README.md",
    ] {
        assert!(paths.contains(&wanted), "{wanted} is not among {paths:?}");
    }
    assert!(!paths.contains(&"CHANGELOG.md"));

    let parts = client.call("guides", json!({ "document": "ravenrs-nfe/README" }));
    let headings = parts["documents"][0]["sections"].as_array().unwrap();
    assert!(headings.iter().any(|h| h.as_str().unwrap().contains("Sandbox")), "{parts}");

    let part = client.call("guides", json!({ "document": "ravenrs-nfe/README", "section": "Sandbox" }));
    assert!(part["sections"][0]["text"].as_str().unwrap().contains("SefazSandbox"), "{part}");
    assert_eq!(part["sections"][0]["whole"], true);

    let sandbox = client.call("guides", json!({ "query": "SefazSandbox on_unhandled" }));
    assert_eq!(sandbox["matches"][0]["document"], "crates/ravenrs-nfe/README.md", "{sandbox}");
    let printers = client.call("guides", json!({ "query": "ESC/POS", "document": "report-label" }));
    assert!(printers["total"].as_u64().unwrap() > 0, "{printers}");
    let editor = client.call("guides", json!({ "query": "Impressora direta" }));
    assert!(editor["total"].as_u64().unwrap() > 0, "{editor}");
    let shop = client.call("guides", json!({ "query": "homologação A1", "document": "AUTOMACAO" }));
    assert!(shop["total"].as_u64().unwrap() > 0, "{shop}");

    assert!(client.call_err("guides", json!({ "document": "nothing-like-this" }))["error"].is_string());
    assert!(
        client.call_err("guides", json!({ "document": "NFSE", "section": "nothing-like-this" }))["error"]
            .is_string()
    );
}
