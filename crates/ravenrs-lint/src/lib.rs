//! Lints for applications built with [RavenRS](https://alexkads.github.io/RavenRS/).
//!
//! The compiler and Clippy know Rust, not the rules an interface must follow to stay right in every
//! theme, for screen readers and across rebuilds. `ravenrs-lint` reads the syntax tree of an
//! application's sources and reports those rules the way the compiler does:
//!
//! ```text
//! error[ravenrs::raw_color]: literal color in the interface
//!   --> src/ui.rs:12:21
//!    |
//! 12 |         .background(Color::rgb(0.1, 0.2, 0.3))
//!    |                     ^^^^^^^^^^
//!    |
//!    = help: use a `Semantic` role, such as `.background(Semantic::Control)`, so it stays right in every theme
//! ```
//!
//! # Running it
//!
//! From the command line, after `cargo install ravenrs-lint`:
//!
//! ```text
//! cargo ravenrs-lint            # every package of the workspace that depends on RavenRS
//! cargo ravenrs-lint --init     # recommended rustc and Clippy lints, the test below and AGENTS.md
//! ```
//!
//! From `cargo test`, so nothing passes while a rule is broken, with `ravenrs-lint` as a
//! dev-dependency and a file such as `tests/ravenrs_lint.rs`:
//!
//! ```no_run
//! #[test]
//! fn follows_the_ravenrs_rules() {
//!     ravenrs_lint::assert_clean();
//! }
//! ```
//!
//! # Configuring it
//!
//! Levels follow Cargo's `[lints]`: `allow`, `warn` or `deny`. They go in the package's or the
//! workspace's metadata, and the package wins:
//!
//! ```toml
//! [package.metadata.ravenrs-lint]
//! language = "pt-BR"            # how button captions are checked: "en" (default) or "pt-BR"
//! extra-verbs = ["Ok"]          # first words accepted as verbs besides the built-in ones
//!
//! [package.metadata.ravenrs-lint.lints]
//! off_grid_spacing = "deny"
//! ```
//!
//! A single place is excused with a comment that says why, on the same line or on its own line
//! right above:
//!
//! ```text
//! // ravenrs-lint: allow(raw_color) the church's logo keeps its brand color in every theme
//! ```
//!
//! `allow-file(...)` excuses the whole file. Test code (`#[test]`, `#[cfg(test)]`) is not checked.
//!
//! The rules are listed in [`Lint`].

mod config;
mod init;
mod report;
mod rules;
mod workspace;

use std::fmt;
use std::path::{Path, PathBuf};

pub use config::{Config, Language, Level};
pub use init::{InitReport, init, recommended_clippy, recommended_lints};
pub use report::{render, render_json};
pub use rules::Lint;
pub use workspace::{Package, packages};

/// One broken rule at one place in a source file.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    /// The rule.
    pub lint: Lint,
    /// How it is reported, after the configuration.
    pub level: Level,
    /// The file, as given to the check.
    pub file: PathBuf,
    /// Line, from 1.
    pub line: usize,
    /// Column of the first character, from 1.
    pub column: usize,
    /// Column after the last character on the same line, from 1.
    pub end_column: usize,
    /// What is wrong.
    pub message: String,
    /// How to fix it.
    pub help: String,
    /// The text of the line, for showing it.
    pub snippet: String,
}

/// Why a check could not run.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file or directory could not be read.
    Io(PathBuf, std::io::Error),
    /// A source file is not valid Rust.
    Parse(PathBuf, syn::Error),
    /// The configuration names something that does not exist.
    Config(String),
    /// `cargo metadata` failed or answered something unexpected.
    Cargo(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(path, error) => write!(f, "could not read {}: {error}", path.display()),
            Error::Parse(path, error) => {
                let at = error.span().start();
                write!(f, "{}:{}:{}: {error}", path.display(), at.line, at.column + 1)
            }
            Error::Config(message) => write!(f, "invalid ravenrs-lint configuration: {message}"),
            Error::Cargo(message) => write!(f, "cargo metadata: {message}"),
        }
    }
}

impl std::error::Error for Error {}

/// Checks one source file. `file` is only used in the diagnostics.
pub fn check_source(file: &Path, source: &str, config: &Config) -> Result<Vec<Diagnostic>, Error> {
    let syntax = syn::parse_file(source).map_err(|error| Error::Parse(file.to_owned(), error))?;
    let mut found = rules::check(&syntax, config, source.lines().count());
    found.extend(rules::check_allow_comments(source));
    let allowed = rules::Allowances::read(source);
    let lines: Vec<&str> = source.lines().collect();
    let mut diagnostics: Vec<Diagnostic> = found
        .into_iter()
        .filter(|finding| !allowed.covers(finding.lint, finding.line))
        .filter_map(|finding| {
            let level = config.level(finding.lint);
            (level != Level::Allow).then(|| Diagnostic {
                lint: finding.lint,
                level,
                file: file.to_owned(),
                line: finding.line,
                column: finding.column,
                end_column: finding.end_column,
                message: finding.message,
                help: finding.help,
                snippet: lines.get(finding.line.saturating_sub(1)).copied().unwrap_or_default().to_owned(),
            })
        })
        .collect();
    diagnostics.sort_by_key(|d| (d.line, d.column));
    Ok(diagnostics)
}

/// Checks every `.rs` file under `dir`, in a stable order. Paths in the diagnostics are relative
/// to `base` when they are inside it.
pub fn check_dir(dir: &Path, base: &Path, config: &Config) -> Result<Vec<Diagnostic>, Error> {
    let mut files = Vec::new();
    collect_rust_files(dir, &mut files)?;
    files.sort();
    let mut diagnostics = Vec::new();
    for path in files {
        let source = std::fs::read_to_string(&path).map_err(|error| Error::Io(path.clone(), error))?;
        let shown = path.strip_prefix(base).unwrap_or(&path);
        diagnostics.extend(check_source(shown, &source, config)?);
    }
    Ok(diagnostics)
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    let entries = std::fs::read_dir(dir).map_err(|error| Error::Io(dir.to_owned(), error))?;
    for entry in entries {
        let path = entry.map_err(|error| Error::Io(dir.to_owned(), error))?.path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

/// Checks the sources of one package: the directories of its library and binaries.
pub fn check_package(package: &Package) -> Result<Vec<Diagnostic>, Error> {
    let mut diagnostics = Vec::new();
    for dir in &package.source_dirs {
        diagnostics.extend(check_dir(dir, &package.root, &package.config)?);
    }
    Ok(diagnostics)
}

/// Checks the package whose tests are running and panics with every diagnostic, warnings
/// included, the way `cargo clippy -- -D warnings` fails. Call it from a `#[test]`.
///
/// # Panics
///
/// When a rule is broken, and when the package cannot be read.
#[allow(clippy::panic)]
pub fn assert_clean() {
    let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") else {
        panic!("ravenrs_lint::assert_clean runs from `cargo test`, which sets CARGO_MANIFEST_DIR");
    };
    let manifest = PathBuf::from(dir).join("Cargo.toml");
    let result = packages(&manifest).and_then(|found| {
        let same = |a: &Path, b: &Path| a == b || a.canonicalize().ok() == b.canonicalize().ok();
        let package = found
            .into_iter()
            .find(|p| same(&p.manifest, &manifest))
            .ok_or_else(|| Error::Cargo(format!("{} is not in its workspace", manifest.display())))?;
        check_package(&package)
    });
    match result {
        Ok(diagnostics) if diagnostics.is_empty() => {}
        Ok(diagnostics) => panic!("\n{}", render(&diagnostics, true)),
        Err(error) => panic!("{error}"),
    }
}
