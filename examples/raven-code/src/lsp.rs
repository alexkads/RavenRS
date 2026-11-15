//! Talking to a language server (rust-analyzer) over the Language Server Protocol: what is
//! wrong while typing, suggestions, tips and where things are defined. It speaks JSON-RPC over
//! any pair of streams, so tests talk to a fake server through pipes, with no rust-analyzer.
//!
//! Requests go out at once and answers come on a thread of their own; the screen reads the
//! last answer of each kind, and `wake` is called when one arrives.

use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::cargo::{Diagnostic, Level};

/// A place in a file, as the protocol counts it: the line from 0, and the column from 0 in
/// UTF-16 units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// From 0.
    pub line: usize,
    /// From 0, in UTF-16 units.
    pub character: usize,
}

/// Where character `at` of `text` is, as the protocol counts it.
pub fn position_of(text: &str, at: usize) -> Position {
    let (mut line, mut character) = (0, 0);
    for c in text.chars().take(at) {
        if c == '\n' {
            line += 1;
            character = 0;
        } else {
            character += c.len_utf16();
        }
    }
    Position { line, character }
}

/// The column, from 1 and in characters, of `character` UTF-16 units into line `line` (from 0)
/// of `text`: the other way around, for what the server says.
pub fn column_of(text: &str, line: usize, character: usize) -> usize {
    let Some(content) = text.split('\n').nth(line) else { return character + 1 };
    let mut units = 0;
    for (n, c) in content.chars().enumerate() {
        if units >= character {
            return n + 1;
        }
        units += c.len_utf16();
    }
    content.chars().count() + 1
}

/// The `file://` address of a path, as the protocol names files.
pub fn uri_of(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = if path.starts_with('/') { path } else { format!("/{path}") };
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~:".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

/// The path of a `file://` address.
pub fn path_of(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let mut bytes = Vec::with_capacity(rest.len());
    let raw = rest.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%'
            && i + 2 < raw.len()
            && let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&raw[i + 1..i + 3]).ok()?, 16)
        {
            bytes.push(byte);
            i += 3;
            continue;
        }
        bytes.push(raw[i]);
        i += 1;
    }
    let path = String::from_utf8(bytes).ok()?;
    // `/C:/x` on Windows is `C:/x`.
    let path = match path.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => path[1..].to_owned(),
        _ => path,
    };
    Some(PathBuf::from(path))
}

/// Something the server offers to put at the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// What the list shows.
    pub label: String,
    /// A type or a signature, when the server says.
    pub detail: Option<String>,
    /// What goes in.
    pub insert: String,
}

/// Where something is defined: a file of the project, from its folder, and the line and the
/// column, both from 1, the column in characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    /// From the project's folder.
    pub file: String,
    /// From 1.
    pub line: usize,
    /// From 1, in characters.
    pub column: usize,
}

/// What a request was, to read its answer.
#[derive(Clone, Copy, Debug)]
enum Asked {
    Initialize,
    Completion,
    Hover,
    Definition,
}

/// What the server has said, kept for the screen.
#[derive(Default)]
struct Heard {
    pending: HashMap<u64, Asked>,
    ready: bool,
    /// What is wrong, by file of the project, as the server last said.
    diagnostics: BTreeMap<String, Vec<Diagnostic>>,
    completions: Vec<Completion>,
    hover: Option<String>,
    definition: Option<Definition>,
    /// The text of each open file, to turn the server's columns into characters.
    texts: HashMap<String, String>,
    /// Set when the server stops answering.
    gone: bool,
}

/// A conversation with a language server about one project.
pub struct Lsp {
    root: PathBuf,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    heard: Arc<Mutex<Heard>>,
    next: AtomicU64,
    versions: Mutex<HashMap<String, i64>>,
    child: Option<Mutex<Child>>,
}

impl std::fmt::Debug for Lsp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lsp").field("root", &self.root).finish_non_exhaustive()
    }
}

impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.send(&json!({"jsonrpc": "2.0", "id": 0, "method": "shutdown"}));
        let _ = self.send(&json!({"jsonrpc": "2.0", "method": "exit"}));
        if let Some(child) = &self.child
            && let Ok(mut child) = child.lock()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Lsp {
    /// Starts rust-analyzer in `root`. An error when it is not installed, which includes the
    /// rustup stand-in that is there without the `rust-analyzer` component.
    pub fn start(root: &Path, wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let works = Command::new("rust-analyzer")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !works {
            return Err(io::Error::new(io::ErrorKind::NotFound, "rust-analyzer not found"));
        }
        let mut child = Command::new("rust-analyzer")
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(io::Error::other("rust-analyzer gave no streams"));
        };
        let mut lsp = Self::connect(stdout, stdin, root, wake);
        lsp.child = Some(Mutex::new(child));
        Ok(lsp)
    }

    /// Talks to a server through `reader` and `writer`, and starts the conversation.
    pub fn connect(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        root: &Path,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let lsp = Self {
            root: root.to_path_buf(),
            writer: Arc::new(Mutex::new(Box::new(writer))),
            heard: Arc::new(Mutex::new(Heard::default())),
            next: AtomicU64::new(1),
            versions: Mutex::new(HashMap::new()),
            child: None,
        };
        let (heard, writer, root) = (lsp.heard.clone(), lsp.writer.clone(), lsp.root.clone());
        std::thread::spawn(move || listen(BufReader::new(reader), &heard, &writer, &root, &wake));
        lsp.request(
            Asked::Initialize,
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": uri_of(&lsp.root),
                "capabilities": {
                    "textDocument": {
                        "completion": {"completionItem": {"snippetSupport": false}},
                        "hover": {"contentFormat": ["plaintext", "markdown"]},
                        "publishDiagnostics": {},
                        "definition": {}
                    }
                },
            }),
        );
        lsp
    }

    /// Whether the server answered the start of the conversation.
    pub fn is_ready(&self) -> bool {
        self.heard.lock().is_ok_and(|heard| heard.ready)
    }

    /// Whether the server stopped answering.
    pub fn is_gone(&self) -> bool {
        self.heard.lock().is_ok_and(|heard| heard.gone)
    }

    fn send(&self, message: &Value) -> io::Result<()> {
        let body = message.to_string();
        let mut writer = self.writer.lock().map_err(|_| io::Error::other("the writer broke"))?;
        write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len())?;
        writer.flush()
    }

    fn notify(&self, method: &str, params: Value) {
        let _ = self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn request(&self, asked: Asked, method: &str, params: Value) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut heard) = self.heard.lock() {
            heard.pending.insert(id, asked);
        }
        let _ = self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
    }

    fn uri(&self, path: &str) -> String {
        uri_of(&self.root.join(path))
    }

    fn document(&self, path: &str, at: usize) -> Value {
        let text = self.heard.lock().ok().and_then(|heard| heard.texts.get(path).cloned()).unwrap_or_default();
        let place = position_of(&text, at);
        json!({
            "textDocument": {"uri": self.uri(path)},
            "position": {"line": place.line, "character": place.character},
        })
    }

    /// Tells the server a file is open, with its text.
    pub fn open(&self, path: &str, text: &str) {
        if let Ok(mut versions) = self.versions.lock() {
            versions.insert(path.to_owned(), 1);
        }
        if let Ok(mut heard) = self.heard.lock() {
            heard.texts.insert(path.to_owned(), text.to_owned());
        }
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": self.uri(path), "languageId": "rust", "version": 1, "text": text}}),
        );
    }

    /// Tells the server the whole new text of an open file.
    pub fn change(&self, path: &str, text: &str) {
        let version = self.versions.lock().map_or(1, |mut versions| {
            let version = versions.entry(path.to_owned()).or_insert(1);
            *version += 1;
            *version
        });
        if let Ok(mut heard) = self.heard.lock() {
            heard.texts.insert(path.to_owned(), text.to_owned());
        }
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": self.uri(path), "version": version},
                "contentChanges": [{"text": text}],
            }),
        );
    }

    /// Tells the server a file was closed.
    pub fn close(&self, path: &str) {
        if let Ok(mut heard) = self.heard.lock() {
            heard.texts.remove(path);
        }
        self.notify("textDocument/didClose", json!({"textDocument": {"uri": self.uri(path)}}));
    }

    /// Asks for suggestions at character `at` of an open file; they come in
    /// [`completions`](Self::completions).
    pub fn complete(&self, path: &str, at: usize) {
        if let Ok(mut heard) = self.heard.lock() {
            heard.completions.clear();
        }
        self.request(Asked::Completion, "textDocument/completion", self.document(path, at));
    }

    /// Asks for a tip about character `at` of an open file; it comes in [`hover`](Self::hover).
    pub fn ask_hover(&self, path: &str, at: usize) {
        if let Ok(mut heard) = self.heard.lock() {
            heard.hover = None;
        }
        self.request(Asked::Hover, "textDocument/hover", self.document(path, at));
    }

    /// Asks where what is at character `at` of an open file is defined; the answer comes in
    /// [`take_definition`](Self::take_definition).
    pub fn ask_definition(&self, path: &str, at: usize) {
        self.request(Asked::Definition, "textDocument/definition", self.document(path, at));
    }

    /// What is wrong in a file of the project, as the server last said. `None` when it has
    /// said nothing about it yet.
    pub fn diagnostics(&self, path: &str) -> Option<Vec<Diagnostic>> {
        self.heard.lock().ok().and_then(|heard| heard.diagnostics.get(path).cloned())
    }

    /// The last suggestions.
    pub fn completions(&self) -> Vec<Completion> {
        self.heard.lock().map(|heard| heard.completions.clone()).unwrap_or_default()
    }

    /// The last tip.
    pub fn hover(&self) -> Option<String> {
        self.heard.lock().ok().and_then(|heard| heard.hover.clone())
    }

    /// Where the last thing asked about is defined, once: the screen goes there and forgets it.
    pub fn take_definition(&self) -> Option<Definition> {
        self.heard.lock().ok().and_then(|mut heard| heard.definition.take())
    }
}

/// Reads one message: its headers, then as many bytes as `Content-Length` says.
fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

/// Reads what the server says until it stops, keeping it in `heard`.
fn listen(
    mut reader: impl BufRead,
    heard: &Mutex<Heard>,
    writer: &Mutex<Box<dyn Write + Send>>,
    root: &Path,
    wake: &(dyn Fn() + Send + Sync),
) {
    while let Some(message) = read_message(&mut reader) {
        let method = message["method"].as_str();
        match (method, message.get("id")) {
            // A request of the server's own, such as for a progress bar: answered, and ignored.
            (Some(_), Some(id)) => {
                let body = json!({"jsonrpc": "2.0", "id": id, "result": null}).to_string();
                if let Ok(mut writer) = writer.lock() {
                    let _ = write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len());
                    let _ = writer.flush();
                }
            }
            (Some("textDocument/publishDiagnostics"), None) => {
                let params = &message["params"];
                let Some(path) = params["uri"].as_str().and_then(path_of) else { continue };
                let Ok(file) = path.strip_prefix(root) else { continue };
                let file = file.to_string_lossy().replace('\\', "/");
                if let Ok(mut heard) = heard.lock() {
                    let text = heard.texts.get(&file).cloned().unwrap_or_default();
                    let found = params["diagnostics"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|d| diagnostic_of(d, &file, &text))
                        .collect();
                    heard.diagnostics.insert(file, found);
                }
                wake();
            }
            (None, Some(id)) => {
                let Some(id) = id.as_u64() else { continue };
                if let Ok(mut heard) = heard.lock() {
                    let Some(asked) = heard.pending.remove(&id) else { continue };
                    let result = &message["result"];
                    match asked {
                        Asked::Initialize => heard.ready = true,
                        Asked::Completion => heard.completions = completions_of(result),
                        Asked::Hover => heard.hover = hover_of(result),
                        Asked::Definition => {
                            heard.definition = definition_of(result, root, &heard.texts);
                        }
                    }
                    if matches!(asked, Asked::Initialize) {
                        // The protocol wants this once the start was answered.
                        let body = json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}).to_string();
                        if let Ok(mut writer) = writer.lock() {
                            let _ = write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len());
                            let _ = writer.flush();
                        }
                    }
                }
                wake();
            }
            _ => {}
        }
    }
    if let Ok(mut heard) = heard.lock() {
        heard.gone = true;
    }
    wake();
}

fn diagnostic_of(d: &Value, file: &str, text: &str) -> Option<Diagnostic> {
    let level = match d["severity"].as_u64() {
        Some(1) => Level::Error,
        Some(2) => Level::Warning,
        _ => return None,
    };
    let start = &d["range"]["start"];
    let end = &d["range"]["end"];
    let line = start["line"].as_u64()? as usize;
    let column = column_of(text, line, start["character"].as_u64()? as usize);
    let length = if end["line"].as_u64() == Some(line as u64) {
        column_of(text, line, end["character"].as_u64()? as usize).saturating_sub(column)
    } else {
        1
    };
    Some(Diagnostic {
        file: file.to_owned(),
        line: line + 1,
        column,
        length: length.max(1),
        level,
        message: d["message"].as_str().unwrap_or_default().lines().next().unwrap_or_default().to_owned(),
    })
}

fn completions_of(result: &Value) -> Vec<Completion> {
    let items = result.as_array().or_else(|| result["items"].as_array());
    items
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let label = item["label"].as_str()?.trim().to_owned();
            let insert = item["textEdit"]["newText"]
                .as_str()
                .or_else(|| item["insertText"].as_str())
                .unwrap_or(&label)
                .to_owned();
            let detail = item["detail"].as_str().map(str::to_owned);
            Some(Completion { label, detail, insert })
        })
        .collect()
}

fn hover_of(result: &Value) -> Option<String> {
    let contents = &result["contents"];
    let text = match contents {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p.as_str().or_else(|| p["value"].as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => contents["value"].as_str()?.to_owned(),
    };
    // Markdown fences and rules are left out; the code and the words stay.
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("```") && line.trim() != "---")
        .collect();
    let text = lines.join("\n").trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn definition_of(result: &Value, root: &Path, texts: &HashMap<String, String>) -> Option<Definition> {
    let first = if result.is_array() { result.get(0)? } else { result };
    let uri = first["uri"].as_str().or_else(|| first["targetUri"].as_str())?;
    let range = if first["range"].is_object() { &first["range"] } else { &first["targetSelectionRange"] };
    let path = path_of(uri)?;
    let file = path.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    let line = range["start"]["line"].as_u64()? as usize;
    let character = range["start"]["character"].as_u64()? as usize;
    let text = texts.get(&file).cloned().or_else(|| std::fs::read_to_string(&path).ok()).unwrap_or_default();
    Some(Definition { column: column_of(&text, line, character), file, line: line + 1 })
}
