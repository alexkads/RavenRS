//! Scenario files, `.raventest`: a scenario as text, one block per line, kept in the app's own
//! `tests/e2e/` folder.
//!
//! ```text
//! scenario: Raven Finanças: lançar despesa
//! // A comment.
//! open Raven Finanças                          | Abrir o Raven Finanças
//! click Lançamentos                            | Ir aos lançamentos
//! type #field:description = Conserto da pia    | Digitar a descrição
//! check Lançamento salvo = Lançamento salvo    | O aviso diz que salvou
//! ```
//!
//! A line is a word, the target (`#id` for a test id, or the text shown; for `open`, the app's
//! name), `= text` when the block takes one, and the block's title after ` | `:
//!
//! | Word | Portuguese | Does |
//! |---|---|---|
//! | `open` | `abrir` | opens the app by name |
//! | `click` | `clicar` | clicks the target |
//! | `type` | `digitar` | types the text into the target |
//! | `fill` | `preencher` | puts the text in the field in place of what it has |
//! | `select` | `escolher` | picks the option (the text) in a list or combo box |
//! | `expand`, `collapse` | `expandir`, `recolher` | opens or closes the item |
//! | `scroll` | `rolar` | scrolls the target into sight |
//! | `drag` | `arrastar` | drags the target to the part after `=` (`drag #port:a:out = #port:b:in`) |
//! | `wait` | `esperar` | waits for the target, and for it to mention the text, if given |
//! | `check` | `conferir` | the target's value, or name, is exactly the text |
//! | `mention` | `mencionar` | the target's name or description contains the text |
//!
//! Every step waits a little for its target to be ready, and an action scrolls to a target out
//! of sight, so `wait` and `scroll` are only for what takes longer or must be seen. One file
//! serves any team; [`Scenario::write`] always writes the English words.
//!
//! Where the files live is the QA team's choice, and the format is the same either way:
//!
//! - **In the app**, in its `tests/e2e/`: the app embeds them with `include_str!` and hands them
//!   over in its [`crate::Hello`], so a tool such as Raven Test Studio (RTS) runs the tests the
//!   app brings, against the installed app too; [`check_folder`], in the app's own tests, keeps
//!   the folder and what the app embeds the same, and every file readable.
//! - **Apart from the app**, in a folder of the QA team, which the tool runs by path.

use std::fmt;

use crate::Command;

/// What a block does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Opens the app by name.
    Open,
    /// Clicks the target.
    Click,
    /// Types the text into the target.
    Type,
    /// The target's value, or its name, is exactly the text.
    Check,
    /// The target's name or description contains the text.
    Mention,
    /// Puts the text in the field in place of what it has.
    Fill,
    /// Picks the option (the text) in a list or a combo box.
    Select,
    /// Opens the item.
    Expand,
    /// Closes the item.
    Collapse,
    /// Scrolls the target into sight.
    Scroll,
    /// Waits for the target to show, and to mention the text when there is one.
    Wait,
    /// Drags the target to the part the text names.
    Drag,
}

impl Kind {
    /// The word the file writes, which is also the kind of a block in the Studio's diagram.
    pub fn word(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Click => "click",
            Self::Type => "type",
            Self::Check => "check",
            Self::Mention => "mention",
            Self::Fill => "fill",
            Self::Select => "select",
            Self::Expand => "expand",
            Self::Collapse => "collapse",
            Self::Scroll => "scroll",
            Self::Wait => "wait",
            Self::Drag => "drag",
        }
    }

    /// The kind a word stands for, in English or in Portuguese.
    pub fn read(word: &str) -> Option<Self> {
        Some(match word.to_lowercase().as_str() {
            "open" | "abrir" => Self::Open,
            "click" | "clicar" => Self::Click,
            "type" | "digitar" => Self::Type,
            "check" | "conferir" => Self::Check,
            "mention" | "mencionar" | "menciona" => Self::Mention,
            "fill" | "preencher" => Self::Fill,
            "select" | "escolher" => Self::Select,
            "expand" | "expandir" | "abrir-item" => Self::Expand,
            "collapse" | "recolher" => Self::Collapse,
            "scroll" | "rolar" => Self::Scroll,
            "wait" | "esperar" | "aguardar" => Self::Wait,
            "drag" | "arrastar" => Self::Drag,
            _ => return None,
        })
    }

    fn needs_text(self) -> bool {
        matches!(self, Self::Type | Self::Check | Self::Mention | Self::Fill | Self::Select | Self::Drag)
    }

    fn takes_text(self) -> bool {
        self.needs_text() || self == Self::Wait
    }
}

/// One block of a scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// What it does.
    pub kind: Kind,
    /// `#id` or the text shown; for [`Kind::Open`], the app's name.
    pub target: String,
    /// What to type or what to find, for the blocks that need it.
    pub text: Option<String>,
    /// How the block is called on screen and in reports.
    pub title: String,
}

impl Block {
    /// The target and the text as one line, `#id = text`, as the Studio's diagram writes it.
    pub fn line(&self) -> String {
        match &self.text {
            Some(text) => format!("{} = {text}", self.target),
            None => self.target.clone(),
        }
    }

    /// The command this block sends to the app; `None` for [`Kind::Open`], which the tool does.
    pub fn command(&self) -> Option<Command> {
        let (target, text) = (self.target.clone(), self.text.clone().unwrap_or_default());
        Some(match self.kind {
            Kind::Open => return None,
            Kind::Click => Command::Click { target },
            Kind::Type => Command::Type { target, text },
            Kind::Check => Command::Check { target, text },
            Kind::Mention => Command::Mention { target, text },
            Kind::Fill => Command::Fill { target, text },
            Kind::Select => Command::Select { target, text },
            Kind::Expand => Command::Expand { target },
            Kind::Collapse => Command::Collapse { target },
            Kind::Scroll => Command::Scroll { target },
            Kind::Wait => Command::Wait { target, text: self.text.clone() },
            Kind::Drag => Command::Drag { target, to: text },
        })
    }
}

/// A scenario read from a `.raventest` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    /// Its name, from the `scenario:` line.
    pub name: String,
    /// Its blocks, in order.
    pub blocks: Vec<Block>,
}

/// Why a scenario file could not be read: the line and what is wrong with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioError {
    /// The line, counting from 1.
    pub line: usize,
    /// What is wrong, in Portuguese, for whoever wrote the file.
    pub message: String,
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "linha {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ScenarioError {}

const WORDS: &str = "open, click, type, check, mention, fill, select, expand, collapse, scroll, wait, drag \
     (ou abrir, clicar, digitar, conferir, mencionar, preencher, escolher, expandir, recolher, rolar, esperar, arrastar)";

impl Scenario {
    /// Reads a `.raventest` file.
    pub fn parse(text: &str) -> Result<Self, ScenarioError> {
        let mut name = None;
        let mut blocks = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            let at = |message: String| ScenarioError { line: index + 1, message };
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            let header = ["scenario:", "cenario:", "cenário:"].iter().find_map(|head| {
                line.get(..head.len()).filter(|s| s.eq_ignore_ascii_case(head)).map(|_| head.len())
            });
            if let Some(len) = header {
                name = Some(line[len..].trim().to_owned());
                continue;
            }
            let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let kind =
                Kind::read(word).ok_or_else(|| at(format!("\"{word}\" não é um bloco; use {WORDS}")))?;
            let (body, title) = match rest.rsplit_once(" | ") {
                Some((body, title)) => (body.trim(), Some(title.trim())),
                None => (rest.trim(), None),
            };
            let (target, text) = match (kind, body.split_once(" = ")) {
                (Kind::Open, _) | (_, None) => (body.to_owned(), None),
                (_, Some(_)) if !kind.takes_text() => {
                    return Err(at(format!("{} não leva texto: só {} #alvo", kind.word(), kind.word())));
                }
                (_, Some((target, text))) => (target.trim().to_owned(), Some(text.trim().to_owned())),
            };
            if target.is_empty() {
                return Err(at(format!("{} sem alvo: diga #id ou o texto que aparece", kind.word())));
            }
            if kind.needs_text() && text.is_none() {
                return Err(at(format!(
                    "{} precisa do texto depois do alvo: {} #alvo = texto",
                    kind.word(),
                    kind.word()
                )));
            }
            let title = title.filter(|t| !t.is_empty()).map_or_else(|| line.to_owned(), str::to_owned);
            blocks.push(Block { kind, target, text, title });
        }
        let name = name.filter(|n| !n.is_empty()).ok_or(ScenarioError {
            line: 1,
            message: "falta o nome do cenário: a primeira linha é scenario: <nome>".to_owned(),
        })?;
        if blocks.first().map(|b| b.kind) != Some(Kind::Open) {
            return Err(ScenarioError {
                line: 1,
                message: "o cenário começa por um bloco open, que abre o app".to_owned(),
            });
        }
        Ok(Self { name, blocks })
    }

    /// The scenario as a `.raventest` file, with the English words.
    pub fn write(&self) -> String {
        let mut out = format!("scenario: {}\n", self.name);
        for block in &self.blocks {
            out.push_str(&format!("{} {} | {}\n", block.kind.word(), block.line(), block.title));
        }
        out
    }

    /// The app its first block opens.
    pub fn app(&self) -> &str {
        self.blocks.first().map_or("", |b| b.target.as_str())
    }
}

/// The folder of an app where its scenarios live, from the app's package folder.
pub const FOLDER: &str = "tests/e2e";
/// The extension of a scenario file.
pub const EXTENSION: &str = "raventest";

/// Checks an app's scenario folder against what the app embeds: every `.raventest` file of
/// `folder` is embedded (its text is in `embedded`), nothing embedded is missing from the
/// folder, and each one reads. Call it from the app's tests with
/// `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/e2e")`.
pub fn check_folder(folder: &std::path::Path, embedded: &[&str]) -> Result<Vec<Scenario>, String> {
    let entries = std::fs::read_dir(folder)
        .map_err(|e| format!("{}: {e}; os cenários do RTS ficam em {FOLDER}/", folder.display()))?;
    let mut files: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == EXTENSION))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("{} não tem nenhum arquivo .{EXTENSION}", folder.display()));
    }
    let mut problems = Vec::new();
    let mut scenarios = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        if !embedded.contains(&text.as_str()) {
            problems.push(format!("{} não está embutido no app (falta um include_str!)", file.display()));
        }
        match Scenario::parse(&text) {
            Ok(scenario) => scenarios.push(scenario),
            Err(e) => problems.push(format!("{}: {e}", file.display())),
        }
    }
    if embedded.len() > files.len() {
        problems.push(format!("o app embute {} cenários e a pasta tem {}", embedded.len(), files.len()));
    }
    if problems.is_empty() { Ok(scenarios) } else { Err(problems.join("\n")) }
}
