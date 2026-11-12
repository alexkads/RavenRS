//! `cargo ravenrs-lint --init`: the strict rustc and Clippy levels, the test that runs the rules,
//! and the instructions every coding agent reads.

use std::path::{Path, PathBuf};

use crate::{Error, Language};

const LINTS: &str = include_str!("../templates/lints.toml");
const CLIPPY: &str = include_str!("../templates/clippy.toml");
const TEST: &str = include_str!("../templates/test.rs");
const AGENTS_EN: &str = include_str!("../templates/agents.en.md");
const AGENTS_PT: &str = include_str!("../templates/agents.pt-BR.md");
const START: &str = "<!-- ravenrs-lint:start -->";
const END: &str = "<!-- ravenrs-lint:end -->";

/// Files the coding agents read: Codex and most others read `AGENTS.md`, Claude Code `CLAUDE.md`,
/// Gemini `GEMINI.md`, GitHub Copilot `.github/copilot-instructions.md`.
const AGENT_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "GEMINI.md", ".github/copilot-instructions.md"];

/// What `init` did.
#[derive(Debug, Default)]
pub struct InitReport {
    /// Files written or changed.
    pub changed: Vec<PathBuf>,
    /// Steps left to the person, with why.
    pub notes: Vec<String>,
}

/// Prepares the package in `dir` for the strict rules. Running it again updates the agents' block
/// and leaves the rest as it is.
pub fn init(dir: &Path, language: Language) -> Result<InitReport, Error> {
    let mut report = InitReport::default();
    let manifest_path = dir.join("Cargo.toml");
    let mut manifest = read(&manifest_path)?;
    let before = manifest.clone();

    if manifest.contains("[lints]") && manifest.contains("workspace = true") {
        report.notes.push(
            "Cargo.toml takes its lints from the workspace: copy the [lints.*] tables from \
             `cargo ravenrs-lint --print-lints` into [workspace.lints.*] of the root Cargo.toml"
                .into(),
        );
    } else if manifest.contains("[lints.") || manifest.contains("[lints]") {
        report.notes.push(
            "Cargo.toml already has [lints]; compare it with `cargo ravenrs-lint --print-lints` and raise \
             what is lower"
                .into(),
        );
    } else {
        manifest.push_str(LINTS);
    }

    if !manifest.contains("ravenrs-lint =") {
        let line = format!("ravenrs-lint = \"={}\"\n", env!("CARGO_PKG_VERSION"));
        if let Some(at) = manifest.find("[dev-dependencies]\n") {
            manifest.insert_str(at + "[dev-dependencies]\n".len(), &line);
        } else {
            manifest.push_str(&format!("\n[dev-dependencies]\n{line}"));
        }
    }

    if !manifest.contains("metadata.ravenrs-lint]") {
        let code = match language {
            Language::English => "en",
            Language::PortugueseBrazil => "pt-BR",
        };
        manifest.push_str(&format!(
            "\n# Every rule of ravenrs-lint is denied. Lowering one needs a reason:\n\
             # rule = {{ level = \"warn\", reason = \"...\" }} in [package.metadata.ravenrs-lint.lints].\n\
             [package.metadata.ravenrs-lint]\nlanguage = \"{code}\"\n"
        ));
    }
    if manifest != before {
        write(&manifest_path, &manifest, &mut report)?;
    }

    let clippy = dir.join("clippy.toml");
    if clippy.exists() {
        report
            .notes
            .push("clippy.toml already exists; compare it with `cargo ravenrs-lint --print-clippy`".into());
    } else {
        write(&clippy, CLIPPY, &mut report)?;
    }

    let test = dir.join("tests").join("ravenrs_lint.rs");
    if !test.exists() {
        write(&test, TEST, &mut report)?;
    }

    let block = match language {
        Language::English => AGENTS_EN,
        Language::PortugueseBrazil => AGENTS_PT,
    };
    for name in AGENT_FILES {
        let path = dir.join(name);
        let current = if path.exists() { read(&path)? } else { String::new() };
        let updated = with_block(&current, block);
        if updated != current {
            write(&path, &updated, &mut report)?;
        }
    }
    Ok(report)
}

/// The text with the agents' block replaced, or added at the end.
fn with_block(text: &str, block: &str) -> String {
    if let (Some(start), Some(end)) = (text.find(START), text.find(END))
        && start < end
    {
        let after = end + END.len();
        let mut out = String::with_capacity(text.len() + block.len());
        out.push_str(&text[..start]);
        out.push_str(block.trim_end());
        out.push_str(&text[after..]);
        return out;
    }
    let mut out = text.trim_end().to_owned();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(block);
    out
}

/// The `[lints]` tables `init` writes.
pub fn recommended_lints() -> &'static str {
    LINTS.trim_start()
}

/// The `clippy.toml` `init` writes.
pub fn recommended_clippy() -> &'static str {
    CLIPPY
}

fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|error| Error::Io(path.to_owned(), error))
}

fn write(path: &Path, text: &str, report: &mut InitReport) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| Error::Io(parent.to_owned(), error))?;
    }
    std::fs::write(path, text).map_err(|error| Error::Io(path.to_owned(), error))?;
    report.changed.push(path.to_owned());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_is_replaced_in_place() {
        let text = format!("# Notes\n\n{START}\nold\n{END}\n\nAfter.\n");
        let updated = with_block(&text, &format!("{START}\nnew\n{END}\n"));
        assert_eq!(updated, format!("# Notes\n\n{START}\nnew\n{END}\n\nAfter.\n"));
    }

    #[test]
    fn the_block_is_added_once() {
        let once = with_block("# Notes\n", AGENTS_EN);
        assert_eq!(with_block(&once, AGENTS_EN), once);
        assert!(once.starts_with("# Notes\n\n<!--"));
    }
}
