//! The messages of `cargo --message-format=json-diagnostic-rendered-ansi`, read line by line, and
//! the errors and warnings kept in the build's folder for the publisher.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where the diagnostics go, under the build's folder: one JSON per line, of every step `cargo`.
pub const DIAGNOSTICS_FILE: &str = ".raven-ci/cargo-diagnostics.jsonl";

/// How grave a diagnostic is. Only errors and warnings are kept; notes and help go to the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// It stops the build.
    Error,
    /// It does not.
    Warning,
}

/// One error or warning, with where it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The step that said it.
    pub step: String,
    /// How grave it is.
    pub level: Level,
    /// Its code, as `E0308` or `clippy::needless_return`, when it has one.
    pub code: Option<String>,
    /// What it says, in one line.
    pub message: String,
    /// The file of its main place, as cargo says it.
    pub file: Option<String>,
    /// Its line there, from 1.
    pub line: Option<u64>,
    /// Its column there, from 1.
    pub column: Option<u64>,
}

impl Diagnostic {
    /// `src/lib.rs:3:5: mismatched types`, for a message of one line.
    pub fn short(&self) -> String {
        match (&self.file, self.line, self.column) {
            (Some(file), Some(line), Some(column)) => format!("{file}:{line}:{column}: {}", self.message),
            (Some(file), Some(line), None) => format!("{file}:{line}: {}", self.message),
            _ => self.message.clone(),
        }
    }
}

/// What a line of cargo's output is.
pub(crate) enum Line {
    /// An error or a warning: kept, and its rendered text goes to the log.
    Diagnostic(Diagnostic, String),
    /// Another message of the compiler (a note, the count of warnings): its text goes to the log.
    Rendered(String),
    /// A message of cargo with nothing for people (an artifact built, a build script run).
    Quiet,
    /// Not a message of cargo: the line goes to the log as it is.
    Text,
}

/// Reads one line of cargo's output. The step's name is filled in by [`keep`].
pub(crate) fn read_line(line: &str) -> Line {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return Line::Text;
    }
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else { return Line::Text };
    let Some(reason) = value.get("reason").and_then(Value::as_str) else { return Line::Text };
    if reason != "compiler-message" {
        return Line::Quiet;
    }
    let Some(message) = value.get("message") else { return Line::Quiet };
    let rendered = message
        .get("rendered")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| message.get("message").and_then(Value::as_str).unwrap_or_default().to_owned());
    let level = match message.get("level").and_then(Value::as_str) {
        Some("error" | "error: internal compiler error") => Level::Error,
        Some("warning") => Level::Warning,
        _ => return Line::Rendered(rendered),
    };
    let text = message.get("message").and_then(Value::as_str).unwrap_or_default().to_owned();
    let spans = message.get("spans").and_then(Value::as_array).cloned().unwrap_or_default();
    let main = spans
        .iter()
        .find(|span| span.get("is_primary").and_then(Value::as_bool).unwrap_or(false))
        .or_else(|| spans.first());
    let diagnostic = Diagnostic {
        step: String::new(),
        level,
        code: message.pointer("/code/code").and_then(Value::as_str).map(str::to_owned),
        message: text,
        file: main.and_then(|s| s.get("file_name")).and_then(Value::as_str).map(str::to_owned),
        line: main.and_then(|s| s.get("line_start")).and_then(Value::as_u64),
        column: main.and_then(|s| s.get("column_start")).and_then(Value::as_u64),
    };
    Line::Diagnostic(diagnostic, rendered)
}

fn file_of(workspace: &Path) -> PathBuf {
    workspace.join(DIAGNOSTICS_FILE)
}

/// Adds the diagnostics `found` by `step` to the file of the build's folder `workspace`.
pub(crate) fn keep(workspace: &Path, step: &str, found: &[Diagnostic]) -> Result<(), String> {
    let path = file_of(workspace);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    for diagnostic in found {
        let mut one = diagnostic.clone();
        one.step = step.to_owned();
        let line = serde_json::to_string(&one).map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// The diagnostics kept in `workspace`, or none when no step `cargo` ran there.
pub(crate) fn read(workspace: &Path) -> Result<Option<Vec<Diagnostic>>, String> {
    let path = file_of(workspace);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut found = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        found.push(serde_json::from_str(line).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    Ok(Some(found))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERROR: &str = r#"{"reason":"compiler-message","package_id":"x","manifest_path":"Cargo.toml","target":{},"message":{"rendered":"error[E0308]: mismatched types\n --> src/lib.rs:3:5\n","children":[],"code":{"code":"E0308","explanation":null},"level":"error","message":"mismatched types","spans":[{"byte_end":1,"byte_start":0,"column_end":6,"column_start":5,"file_name":"src/lib.rs","is_primary":true,"line_end":3,"line_start":3,"text":[]}]}}"#;

    #[test]
    fn an_error_of_the_compiler_is_kept_with_its_place_and_its_rendered_text() {
        let Line::Diagnostic(diagnostic, rendered) = read_line(ERROR) else { panic!("a diagnostic") };
        assert_eq!(diagnostic.level, Level::Error);
        assert_eq!(diagnostic.code.as_deref(), Some("E0308"));
        assert_eq!(diagnostic.short(), "src/lib.rs:3:5: mismatched types");
        assert!(rendered.starts_with("error[E0308]"));
    }

    #[test]
    fn other_lines_are_quiet_rendered_or_text() {
        assert!(matches!(read_line(r#"{"reason":"compiler-artifact","target":{}}"#), Line::Quiet));
        assert!(matches!(read_line("   Compiling site v0.1.0"), Line::Text));
        assert!(matches!(read_line("{ not json"), Line::Text));
        let note = r#"{"reason":"compiler-message","message":{"level":"note","message":"x","rendered":"note: x\n","spans":[]}}"#;
        assert!(matches!(read_line(note), Line::Rendered(text) if text == "note: x\n"));
    }

    #[test]
    fn what_is_kept_by_each_step_is_read_back_in_order() {
        let dir = std::env::temp_dir().join(format!("raven-ci-cargo-diag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(read(&dir).expect("read"), None);
        let Line::Diagnostic(one, _) = read_line(ERROR) else { panic!("a diagnostic") };
        keep(&dir, "e1-1", std::slice::from_ref(&one)).expect("kept");
        keep(&dir, "e2-1", &[one]).expect("kept");
        let back = read(&dir).expect("read").expect("some");
        let steps: Vec<&str> = back.iter().map(|d| d.step.as_str()).collect();
        assert_eq!(steps, ["e1-1", "e2-1"]);
        std::fs::remove_dir_all(&dir).expect("removed");
    }
}
