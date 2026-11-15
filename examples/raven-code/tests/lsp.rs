//! The language server client, against a fake server on pipes: no rust-analyzer needed.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use raven_code::cargo::Level;
use common::fake_server;
use raven_code::lsp::{Definition, Lsp, Position, column_of, path_of, position_of, uri_of};

#[test]
fn places_are_counted_as_the_protocol_counts_them() {
    // "é" is one UTF-16 unit, "🦀" is two.
    let text = "fn a() {}\nlet é🦀x = 1;";
    assert_eq!(position_of(text, 0), Position { line: 0, character: 0 });
    let x = text.chars().position(|c| c == 'x').unwrap();
    assert_eq!(position_of(text, x), Position { line: 1, character: 7 });
    assert_eq!(column_of(text, 1, 7), x - 10 + 1, "and back: the 7th character of its line, from 1");
    let root = PathBuf::from("/work/my project");
    let uri = uri_of(&root.join("src/main.rs"));
    assert_eq!(uri, "file:///work/my%20project/src/main.rs");
    assert_eq!(path_of(&uri), Some(root.join("src/main.rs")));
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn sample_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("raven-code-lsp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    raven_code::project::write_sample(&root).unwrap();
    root
}

#[test]
fn the_client_hears_problems_suggestions_tips_and_definitions() {
    let root = sample_root("client");
    let (reader, writer) = fake_server(&root);
    let lsp = Lsp::connect(reader, writer, &root, || {});
    until("the start", || lsp.is_ready());
    let text = "mod greeting;\n\nfn main() {\n    printn!(\"{}\", greeting::hello(\"Raven Code\"));\n}\n";
    lsp.open("src/main.rs", text);
    until("the problems", || lsp.diagnostics("src/main.rs").is_some());
    let found = lsp.diagnostics("src/main.rs").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].line, found[0].column, found[0].length), (4, 5, 6));
    assert_eq!(found[0].level, Level::Error);
    assert_eq!(found[0].message, "cannot find macro `printn` in this scope", "the first line only");
    // Fixed, the problem goes away.
    lsp.change("src/main.rs", &text.replace("printn!", "println!"));
    until("the fix", || lsp.diagnostics("src/main.rs").is_some_and(|d| d.is_empty()));

    lsp.complete("src/main.rs", 40);
    until("suggestions", || !lsp.completions().is_empty());
    let offered = lsp.completions();
    assert_eq!(offered[0].label, "hello");
    assert_eq!(offered[0].detail.as_deref(), Some("fn(&str) -> String"));
    assert_eq!(offered[1].insert, "help()");

    lsp.ask_hover("src/main.rs", 40);
    until("a tip", || lsp.hover().is_some());
    assert_eq!(lsp.hover().unwrap(), "pub fn hello(name: &str) -> String\nA greeting for `name`.");

    lsp.ask_definition("src/main.rs", 40);
    until("a definition", || {
        lsp.take_definition().is_some_and(|d| d == Definition { file: "src/greeting.rs".into(), line: 2, column: 8 })
    });
    assert!(lsp.take_definition().is_none(), "taken once");
}

/// With rust-analyzer installed (`rustup component add rust-analyzer`): the real server finds
/// the error in the sample. `cargo test -p raven-code --test lsp -- --ignored`.
#[test]
#[ignore = "needs rust-analyzer, which takes a while to read a project"]
fn rust_analyzer_finds_the_error_in_the_sample() {
    let root = sample_root("real");
    let text = std::fs::read_to_string(root.join("src/main.rs")).unwrap().replace("println!", "printn!");
    let lsp = Lsp::start(&root, || {}).expect("rust-analyzer is installed");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while !lsp.is_ready() {
        assert!(std::time::Instant::now() < deadline, "it started");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    lsp.open("src/main.rs", &text);
    while !lsp.diagnostics("src/main.rs").is_some_and(|d| !d.is_empty()) {
        assert!(std::time::Instant::now() < deadline, "it found the error");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(lsp.diagnostics("src/main.rs").unwrap()[0].line, 4);
}

#[test]
fn a_missing_rust_analyzer_is_said_to_be_not_found() {
    // Where it is installed this starts it, and dropping it stops it; where it is not, or is
    // only rustup's stand-in, the error says not found at once.
    let root = sample_root("which");
    if let Err(error) = Lsp::start(&root, || {}) {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}
