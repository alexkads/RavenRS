//! The rules of `ravenrs-lint`, and a check of one source text against them. Nothing is read from
//! or written to disk: the text comes in the call.

use std::path::Path;

use ravenrs_lint::{Config, Language, Lint, check_source};
use serde_json::{Value, json};

use crate::api::Index;

/// Every rule: its name, one line on what it protects and the rustdoc of its variant.
pub fn rules() -> Value {
    let index = Index::get();
    let documented = index
        .items
        .iter()
        .find(|item| item.krate == "ravenrs-lint" && item.name == "Lint" && item.kind == "enum");
    let rules: Vec<Value> = Lint::ALL
        .iter()
        .map(|lint| {
            let variant = format!("{lint:?}");
            let docs = documented
                .and_then(|item| item.variants.iter().find(|member| member.name == variant))
                .map(|member| member.docs.clone())
                .unwrap_or_default();
            json!({ "name": lint.name(), "protects": lint.summary(), "catches": docs })
        })
        .collect();
    json!({
        "total": rules.len(),
        "rules": rules,
        "every_rule": "deny unless [package.metadata.ravenrs-lint.lints] lowers it to warn or allow",
        "excuse": "// ravenrs-lint: allow(<rule>) <why>, on the same line or right above; allow-file(..) for the whole file. An exception without a reason is itself a finding",
        "run": "cargo ravenrs-lint in the project, or ravenrs_lint::assert_clean() in a test",
        "not_checked": "test code (#[test], #[cfg(test)]) and the tests/ directory",
    })
}

/// The findings of one file's text. `module_cycle` needs the whole crate, so it is not run here.
pub fn check(source: &str, file: Option<&str>, language: Option<&str>) -> Result<Value, String> {
    let mut config = Config::default();
    config.language = match language {
        None | Some("en") => Language::English,
        Some("pt-BR") => Language::PortugueseBrazil,
        Some(other) => return Err(format!("unknown language '{other}': use 'en' or 'pt-BR'")),
    };
    let file = file.unwrap_or("src/lib.rs");
    let diagnostics = check_source(Path::new(file), source, &config).map_err(|error| error.to_string())?;
    let findings: Vec<Value> = diagnostics
        .iter()
        .map(|d| {
            json!({
                "rule": d.lint.name(),
                "level": d.level.label(),
                "line": d.line,
                "column": d.column,
                "message": d.message,
                "help": d.help,
                "snippet": d.snippet.trim(),
            })
        })
        .collect();
    Ok(json!({
        "file": file,
        "clean": findings.is_empty(),
        "total": findings.len(),
        "findings": findings,
        "note": "module_cycle reads the whole crate: run cargo ravenrs-lint for it",
    }))
}
