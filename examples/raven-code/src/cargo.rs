//! Building and running the project with cargo, on a thread of its own, and reading what the
//! compiler says. The reading is apart from the running, so it is tested with no cargo at all.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use ravenrs_process::{End, Event, Spec};

use serde_json::Value;

/// What to ask cargo for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// `cargo build`.
    Build,
    /// `cargo run`, which takes what is typed in the terminal.
    Run,
    /// `cargo test`.
    Test,
    /// `cargo check`.
    Check,
    /// `cargo clippy`.
    Clippy,
}

impl Task {
    /// The word cargo takes.
    pub fn verb(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Run => "run",
            Self::Test => "test",
            Self::Check => "check",
            Self::Clippy => "clippy",
        }
    }

    /// Its name, for a menu and the status bar.
    pub fn name(self) -> &'static str {
        match self {
            Self::Build => "Build",
            Self::Run => "Run",
            Self::Test => "Test",
            Self::Check => "Check",
            Self::Clippy => "Clippy",
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
    /// How many tests passed and failed, from every `test result:` line of `cargo test`.
    pub tests: Option<(usize, usize)>,
    /// Whether it was stopped before it ended.
    pub stopped: bool,
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
                if let Some(counts) = test_result(&text) {
                    let (passed, failed) = self.tests.unwrap_or_default();
                    self.tests = Some((passed + counts.0, failed + counts.1));
                }
                self.output.push_str(&text);
                self.output.push('\n');
            }
        }
    }
}

/// How many passed and failed, from a line such as
/// `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
pub fn test_result(line: &str) -> Option<(usize, usize)> {
    let rest = line.trim().strip_prefix("test result:")?;
    let count = |word: &str| {
        rest.split(';').find_map(|part| {
            let part = part.trim().trim_start_matches("ok.").trim_start_matches("FAILED.").trim();
            part.strip_suffix(word)?.trim().parse::<usize>().ok()
        })
    };
    Some((count("passed")?, count("failed")?))
}

/// The binaries of the packages in a `cargo metadata --no-deps --format-version 1`, by name.
pub fn binaries_in(metadata: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(metadata) else { return Vec::new() };
    let mut names = Vec::new();
    for package in value["packages"].as_array().into_iter().flatten() {
        for target in package["targets"].as_array().into_iter().flatten() {
            let is_bin = target["kind"].as_array().is_some_and(|kinds| kinds.iter().any(|k| k == "bin"));
            if is_bin && let Some(name) = target["name"].as_str() {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// The binaries of the project in `root`, asking cargo.
pub fn binaries(root: &Path) -> io::Result<Vec<String>> {
    let out = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .stderr(Stdio::null())
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other("cargo metadata failed"));
    }
    Ok(binaries_in(&String::from_utf8_lossy(&out.stdout)))
}

/// A handle on what cargo is running: to stop it, and to type into the program.
#[derive(Clone, Default)]
pub struct Control {
    child: Arc<Mutex<Option<ravenrs_process::Control>>>,
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let running = self.child.lock().is_ok_and(|child| child.is_some());
        f.debug_struct("Control").field("running", &running).finish()
    }
}

impl Control {
    /// Stops what runs, cargo and the program it started. False when nothing runs.
    pub fn stop(&self) -> bool {
        let Ok(child) = self.child.lock() else { return false };
        let Some(child) = child.as_ref() else { return false };
        // Its input closes first, so a program that waits for a line ends on its own.
        child.stop();
        true
    }

    /// Sends a line to the program, as typed in a terminal. False when nothing takes it.
    pub fn send(&self, line: &str) -> bool {
        let Ok(child) = self.child.lock() else { return false };
        child.as_ref().is_some_and(|child| child.send(line))
    }
}

/// Runs cargo in `root` on a thread, writing what it says into `report` and calling `wake` after
/// each line, so the screen shows it as it comes. `bin` picks the binary `cargo run` runs, and
/// `control` stops it or types into it. It does nothing while another one runs.
pub fn start(
    root: PathBuf,
    task: Task,
    bin: Option<String>,
    report: Arc<Mutex<Report>>,
    control: Control,
    wake: impl Fn() + Send + Sync + 'static,
) {
    {
        let Ok(mut now) = report.lock() else { return };
        if now.running {
            return;
        }
        let mut shown = format!("$ cargo {}", task.verb());
        if let Some(bin) = &bin {
            shown.push_str(&format!(" --bin {bin}"));
        }
        *now = Report { output: format!("{shown}\n"), running: true, task: Some(task), ..Report::default() };
    }
    std::thread::spawn(move || {
        // Cargo needs the person's whole environment (its toolchains, its caches), as in a terminal.
        let mut spec =
            Spec::new("cargo").arg(task.verb()).arg("--message-format=json").dir(&root).inherit_env();
        if let (Task::Run, Some(bin)) = (task, &bin) {
            spec = spec.args(["--bin", bin]);
        }
        if task == Task::Run {
            spec = spec.stdin();
        }
        let running = match spec.spawn() {
            Ok(running) => running,
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
        if let Ok(mut held) = control.child.lock() {
            *held = Some(running.control());
        }
        let mut end = End::Code(-1);
        for event in running.events() {
            if let Ok(mut now) = report.lock() {
                match event {
                    // What the compiler says, as JSON.
                    Event::Out(line) => now.take(&line),
                    // What cargo itself says (Compiling…, Finished…), and the program's errors.
                    Event::Err(line) => {
                        now.output.push_str(&line);
                        now.output.push('\n');
                    }
                    Event::Ended(how) => end = how,
                }
            }
            wake();
        }
        if let Ok(mut held) = control.child.lock() {
            held.take();
        }
        if let Ok(mut now) = report.lock() {
            now.running = false;
            let ok = end.success();
            now.stopped = matches!(end, End::Stopped | End::TimedOut | End::Signal(_));
            if now.stopped {
                now.output.push_str("stopped\n");
            }
            now.success = Some(now.success.unwrap_or(ok) && ok);
        }
        wake();
    });
}
