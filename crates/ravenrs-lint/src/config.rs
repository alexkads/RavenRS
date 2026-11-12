//! What a package says about the rules, read from `[package.metadata.ravenrs-lint]` and
//! `[workspace.metadata.ravenrs-lint]`.

use std::collections::HashMap;

use serde_json::Value;

use crate::{Error, Lint};

/// How a broken rule is reported, as in Cargo's `[lints]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Level {
    /// Not reported.
    Allow,
    /// Reported as a warning. `assert_clean` still fails.
    Warn,
    /// Reported as an error, and `cargo ravenrs-lint` exits with a failure.
    Deny,
}

impl Level {
    fn parse(text: &str) -> Option<Level> {
        match text {
            "allow" => Some(Level::Allow),
            "warn" => Some(Level::Warn),
            "deny" | "forbid" => Some(Level::Deny),
            _ => None,
        }
    }

    /// `"warning"` or `"error"`, as the compiler prints it.
    pub fn label(self) -> &'static str {
        match self {
            Level::Allow => "allowed",
            Level::Warn => "warning",
            Level::Deny => "error",
        }
    }
}

/// The language of the interface's text, for the checks that read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Language {
    /// English: captions start with an imperative verb from a built-in list.
    #[default]
    English,
    /// Brazilian Portuguese: captions start with an infinitive ("Salvar", "Cadastrar").
    PortugueseBrazil,
}

/// The configuration of one package.
#[derive(Clone, Debug)]
pub struct Config {
    levels: HashMap<Lint, Level>,
    /// The language of the interface.
    pub language: Language,
    /// First words of button captions accepted as verbs, besides the built-in ones.
    pub extra_verbs: Vec<String>,
    /// The longest a source file may be before `oversized_file`.
    pub max_file_lines: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            levels: HashMap::new(),
            language: Language::default(),
            extra_verbs: Vec::new(),
            max_file_lines: 600,
        }
    }
}

impl Config {
    /// The level of `lint`: `deny` unless the configuration lowered it.
    pub fn level(&self, lint: Lint) -> Level {
        self.levels.get(&lint).copied().unwrap_or(Level::Deny)
    }

    /// Sets the level of `lint`.
    pub fn set_level(&mut self, lint: Lint, level: Level) {
        self.levels.insert(lint, level);
    }

    /// Reads the workspace's table, then the package's over it. Both are the JSON that
    /// `cargo metadata` gives for `metadata.ravenrs-lint`.
    pub fn from_metadata(workspace: Option<&Value>, package: Option<&Value>) -> Result<Config, Error> {
        let mut config = Config::default();
        for table in [workspace, package].into_iter().flatten() {
            config.merge(table)?;
        }
        Ok(config)
    }

    fn merge(&mut self, table: &Value) -> Result<(), Error> {
        let Some(table) = table.as_object() else {
            return Err(Error::Config("[metadata.ravenrs-lint] must be a table".into()));
        };
        for (key, value) in table {
            match key.as_str() {
                "language" => {
                    self.language = match value.as_str() {
                        Some("en") => Language::English,
                        Some("pt-BR" | "pt") => Language::PortugueseBrazil,
                        _ => {
                            return Err(Error::Config(format!(
                                "language must be \"en\" or \"pt-BR\", not {value}"
                            )));
                        }
                    }
                }
                "extra-verbs" => {
                    let words = value.as_array().and_then(|items| {
                        items.iter().map(|w| w.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>()
                    });
                    self.extra_verbs =
                        words.ok_or_else(|| Error::Config("extra-verbs must be a list of words".into()))?;
                }
                "max-file-lines" => {
                    self.max_file_lines = value
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or_else(|| Error::Config("max-file-lines must be a whole number".into()))?;
                }
                "lints" => self.merge_levels(value)?,
                other => {
                    return Err(Error::Config(format!(
                        "unknown key `{other}`; the keys are language, extra-verbs, max-file-lines and lints"
                    )));
                }
            }
        }
        Ok(())
    }

    fn merge_levels(&mut self, value: &Value) -> Result<(), Error> {
        let Some(levels) = value.as_object() else {
            return Err(Error::Config("[metadata.ravenrs-lint.lints] must be a table".into()));
        };
        for (name, value) in levels {
            let lint = Lint::from_name(name).ok_or_else(|| {
                let known: Vec<&str> = Lint::ALL.iter().map(|l| l.name()).collect();
                Error::Config(format!("unknown rule `{name}`; the rules are {}", known.join(", ")))
            })?;
            // `"deny"`, or `{ level = "warn", reason = "..." }`: lowering a rule needs a reason.
            let (level, reason) = match value {
                Value::String(level) => (level.as_str(), None),
                Value::Object(table) => (
                    table.get("level").and_then(Value::as_str).unwrap_or_default(),
                    table.get("reason").and_then(Value::as_str).map(str::trim).filter(|r| !r.is_empty()),
                ),
                _ => ("", None),
            };
            let level = Level::parse(level).ok_or_else(|| {
                Error::Config(format!(
                    "`{name}` must be \"deny\" or {{ level = \"warn\", reason = \"...\" }}"
                ))
            })?;
            if level < Level::Deny && reason.is_none() {
                return Err(Error::Config(format!(
                    "`{name}` is lowered to {level:?} without a reason. Fix the code instead; when lowering the \
                     rule is right, write {name} = {{ level = \"{}\", reason = \"why\" }}",
                    match level {
                        Level::Allow => "allow",
                        _ => "warn",
                    }
                )));
            }
            self.levels.insert(lint, level);
        }
        Ok(())
    }
}
