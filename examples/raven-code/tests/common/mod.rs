//! A fake language server on pipes, shared by the tests.
#![allow(dead_code, clippy::unwrap_used, reason = "each test uses part of it")]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use raven_code::lsp::uri_of;
use serde_json::{Value, json};

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(n) = line.strip_prefix("Content-Length:") {
            length = n.trim().parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn write_message(writer: &mut impl Write, message: &Value) {
    let body = message.to_string();
    let _ = write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len());
    let _ = writer.flush();
}

/// A server that answers like rust-analyzer would about the sample project: an error where
/// `printn!` is, `hello` and `help` as suggestions, a tip, and `hello` defined in greeting.rs.
/// It also asks the client something of its own, as rust-analyzer does for progress bars.
pub fn fake_server(root: &Path) -> (impl Read + Send + 'static, impl Write + Send + 'static) {
    let (from_server, mut to_client) = std::io::pipe().unwrap();
    let (from_client, to_server) = std::io::pipe().unwrap();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(from_client);
        while let Some(message) = read_message(&mut reader) {
            let id = message.get("id").cloned();
            let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
            match message["method"].as_str() {
                Some("initialize") => {
                    write_message(
                        &mut to_client,
                        &json!({"jsonrpc": "2.0", "id": "progress-1", "method": "window/workDoneProgress/create", "params": {}}),
                    );
                    write_message(&mut to_client, &reply(json!({"capabilities": {}})));
                }
                Some("textDocument/didOpen") | Some("textDocument/didChange") => {
                    let uri = message["params"]["textDocument"]["uri"].as_str().unwrap().to_owned();
                    let text = message["params"]["textDocument"]["text"]
                        .as_str()
                        .or_else(|| message["params"]["contentChanges"][0]["text"].as_str())
                        .unwrap_or_default();
                    let diagnostics: Vec<Value> = text
                        .lines()
                        .enumerate()
                        .filter_map(|(n, line)| {
                            let at = line.find("printn!")?;
                            Some(json!({
                                "severity": 1,
                                "message": "cannot find macro `printn` in this scope\nmore",
                                "range": {"start": {"line": n, "character": at}, "end": {"line": n, "character": at + 6}},
                            }))
                        })
                        .collect();
                    write_message(
                        &mut to_client,
                        &json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": {"uri": uri, "diagnostics": diagnostics}}),
                    );
                }
                Some("textDocument/completion") => write_message(
                    &mut to_client,
                    &reply(json!({"isIncomplete": false, "items": [
                        {"label": "hello", "detail": "fn(&str) -> String"},
                        {"label": "help", "insertText": "help()"},
                    ]})),
                ),
                Some("textDocument/hover") => write_message(
                    &mut to_client,
                    &reply(
                        json!({"contents": {"kind": "markdown", "value": "```rust\npub fn hello(name: &str) -> String\n```\n---\nA greeting for `name`."}}),
                    ),
                ),
                Some("textDocument/definition") => write_message(
                    &mut to_client,
                    &reply(
                        json!([{"uri": uri_of(&root.join("src/greeting.rs")), "range": {"start": {"line": 1, "character": 7}, "end": {"line": 1, "character": 12}}}]),
                    ),
                ),
                Some("shutdown") => write_message(&mut to_client, &reply(Value::Null)),
                Some("exit") => return,
                _ => {}
            }
        }
    });
    (from_server, to_server)
}
