//! Diagnostics written the way the compiler writes them, or as JSON for tools.

use std::collections::HashSet;
use std::fmt::Write as _;

use crate::{Diagnostic, Level, Lint};

/// The diagnostics as the compiler would print them, followed by a summary line when `summary`.
pub fn render(diagnostics: &[Diagnostic], summary: bool) -> String {
    let mut out = String::new();
    let mut explained: HashSet<Lint> = HashSet::new();
    for d in diagnostics {
        let gutter = d.line.to_string().len();
        let pad = " ".repeat(gutter);
        let _ = writeln!(out, "{}[ravenrs::{}]: {}", d.level.label(), d.lint.name(), d.message);
        let _ = writeln!(out, "{pad}--> {}:{}:{}", d.file.display(), d.line, d.column);
        let _ = writeln!(out, "{pad} |");
        let _ = writeln!(out, "{} | {}", d.line, d.snippet);
        let marks = "^".repeat(d.end_column.saturating_sub(d.column).max(1));
        let _ = writeln!(out, "{pad} | {}{marks}", " ".repeat(d.column.saturating_sub(1)));
        let _ = writeln!(out, "{pad} |");
        let _ = writeln!(out, "{pad} = help: {}", d.help);
        if explained.insert(d.lint) {
            let _ = writeln!(
                out,
                "{pad} = note: `ravenrs::{}` ({}) is {}; fix the code rather than silencing it",
                d.lint.name(),
                d.lint.summary(),
                match d.level {
                    Level::Deny => "denied",
                    _ => "a warning",
                },
            );
        }
        out.push('\n');
    }
    if summary && !diagnostics.is_empty() {
        let errors = diagnostics.iter().filter(|d| d.level == Level::Deny).count();
        let warnings = diagnostics.len() - errors;
        let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
        let _ = writeln!(
            out,
            "error: ravenrs-lint found {} and {}. Fix each one as its help says, then run it again.",
            plural(errors, "error"),
            plural(warnings, "warning"),
        );
    }
    out
}

/// One JSON object per line, for editors and agents.
pub fn render_json(diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    for d in diagnostics {
        let value = serde_json::json!({
            "lint": format!("ravenrs::{}", d.lint.name()),
            "level": d.level.label(),
            "file": d.file.display().to_string(),
            "line": d.line,
            "column": d.column,
            "end_column": d.end_column,
            "message": d.message,
            "help": d.help,
        });
        out.push_str(&value.to_string());
        out.push('\n');
    }
    out
}
