//! Building and running the project with cargo, on a thread of its own, and reading what the
//! compiler says. The reading is apart from the running, so it is tested with no cargo at all.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::Value;

/// What to ask cargo for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// `cargo build`.
    Build,
    /// `cargo run`.
    Run,
}

impl Task {
    fn verb(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Run => "run",
        }
    }
}

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// It stops the build.
    Error,
    /// It does not.
    Warning,
}

/// Something the compiler found, at a place in a file of the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The file, from the project's folder, such as `src/main.rs`.
    pub file: String,
    /// From 1.
    pub line: usize,
    /// From 1, in characters.
    pub column: usize,
    /// How many characters it covers on its line, at least 1.
    pub length: usize,
    /// Error or warning.
    pub level: Level,
    /// What it says, such as "cannot find macro `printn` in this scope".
    pub message: String,
}

/// What one line of cargo's output is.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// A message of the compiler: the text it would print, and where in the code, when it says.
    Compiler {
        /// What it prints, with its arrows and line numbers.
        rendered: String,
        /// Where it is, when it is an error or a warning at a place in the code.
        diagnostic: Option<Diagnostic>,
    },
    /// The build ended, and whether it worked.
    Finished(bool),
    /// Another message of cargo, which shows nothing.
    Quiet,
    /// Text that is not a message: what the program itself prints, when it runs.
    Text(String),
}

/// Reads one line of `cargo build --message-format=json` (or `run`).
pub fn read_line(line: &str) -> Line {
    if !line.starts_with('{') {
        return Line::Text(line.to_owned());
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else { return Line::Text(line.to_owned()) };
    match value.get("reason").and_then(Value::as_str) {
        Some("compiler-message") => {
            let message = &value["message"];
            let rendered = message["rendered"].as_str().unwrap_or_default().trim_end().to_owned();
            Line::Compiler { rendered, diagnostic: diagnostic_of(message) }
        }
        Some("build-finished") => Line::Finished(value["success"].as_bool().unwrap_or(false)),
        Some(_) => Line::Quiet,
        None => Line::Text(line.to_owned()),
    }
}

fn diagnostic_of(message: &Value) -> Option<Diagnostic> {
    let level = match message["level"].as_str()? {
        "error" => Level::Error,
        "warning" => Level::Warning,
        _ => return None,
    };
    let spans = message["spans"].as_array()?;
    let span = spans.iter().find(|span| span["is_primary"].as_bool() == Some(true))?;
    let number = |key: &str| span[key].as_u64().map(|n| n as usize);
    let (line, column) = (number("line_start")?, number("column_start")?);
    let length = match (number("line_end"), number("column_end")) {
        (Some(end), Some(column_end)) if end == line => column_end.saturating_sub(column),
        _ => 1,
    };
    Some(Diagnostic {
        file: span["file_name"].as_str()?.replace('\\', "/"),
        line,
        column,
        length: length.max(1),
        level,
        message: message["message"].as_str().unwrap_or_default().to_owned(),
    })
}

/// What a build or a run has said so far, shared with the thread that runs it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Everything printed, as a terminal would show it.
    pub output: String,
    /// What the compiler found.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether cargo is still at it.
    pub running: bool,
    /// How the build ended, once it has.
    pub success: Option<bool>,
    /// What it is doing, for the status bar.
    pub task: Option<Task>,
}

impl Report {
    /// How many errors and warnings, in a few words, such as "1 error, 2 warnings".
    pub fn summary(&self) -> String {
        let count = |level| self.diagnostics.iter().filter(|d| d.level == level).count();
        let words =
            |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
        format!(
            "{}, {}",
            words(count(Level::Error), "error", "errors"),
            words(count(Level::Warning), "warning", "warnings")
        )
    }

    /// Takes in one line of cargo's output.
    pub fn take(&mut self, line: &str) {
        match read_line(line) {
            Line::Compiler { rendered, diagnostic } => {
                if !rendered.is_empty() {
                    self.output.push_str(&rendered);
                    self.output.push('\n');
                }
                if let Some(diagnostic) = diagnostic {
                    self.diagnostics.push(diagnostic);
                }
            }
            Line::Finished(success) => self.success = Some(success),
            Line::Quiet => {}
            Line::Text(text) => {
                self.output.push_str(&text);
                self.output.push('\n');
            }
        }
    }
}

/// Runs cargo in `root` on a thread, writing what it says into `report` and calling `wake` after
/// each line, so the screen shows it as it comes. It does nothing while another one runs.
pub fn start(root: PathBuf, task: Task, report: Arc<Mutex<Report>>, wake: impl Fn() + Send + Sync + 'static) {
    {
        let Ok(mut now) = report.lock() else { return };
        if now.running {
            return;
        }
        *now = Report {
            output: format!("$ cargo {}\n", task.verb()),
            running: true,
            task: Some(task),
            ..Report::default()
        };
    }
    let wake = Arc::new(wake);
    std::thread::spawn(move || {
        let child = Command::new("cargo")
            .arg(task.verb())
            .arg("--message-format=json")
            .current_dir(&root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) => {
                if let Ok(mut now) = report.lock() {
                    now.output.push_str(&format!("could not start cargo: {error}\n"));
                    now.running = false;
                    now.success = Some(false);
                }
                wake();
                return;
            }
        };
        // What cargo itself says (Compiling…, Finished…) comes on the other stream.
        let errors = child.stderr.take().map(|stderr| {
            let (report, wake) = (report.clone(), wake.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Ok(mut now) = report.lock() {
                        now.output.push_str(&line);
                        now.output.push('\n');
                    }
                    wake();
                }
            })
        });
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(mut now) = report.lock() {
                    now.take(&line);
                }
                wake();
            }
        }
        if let Some(errors) = errors {
            let _ = errors.join();
        }
        let status = child.wait();
        if let Ok(mut now) = report.lock() {
            now.running = false;
            let ok = status.map(|status| status.success()).unwrap_or(false);
            now.success = Some(now.success.unwrap_or(ok) && ok);
        }
        wake();
    });
}
