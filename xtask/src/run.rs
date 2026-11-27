//! Running a check: its steps one after the other, each timed, the first failure stopping it, and
//! the table of times at the end, passing or failing.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Why a check stopped: the command that failed and the code it ended with.
#[derive(Debug)]
pub struct Failed {
    /// What was run, as it would be typed.
    pub what: String,
    /// The process's exit code, or 1 when it could not start or was killed.
    pub code: i32,
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` failed with code {}", self.what, self.code)
    }
}

/// The Python the project's helper scripts run on: `python` on Windows, where `python3` is not
/// the installer's name, and `python3` elsewhere, where `python` may be missing or be Python 2.
pub const PYTHON: &str = if cfg!(windows) { "python" } else { "python3" };

/// What a step gives: done, or the failure that stops the check.
pub type Done = Result<(), Failed>;

/// A failure that is not a command: a selection that cannot be read, say.
pub fn failure(what: impl Into<String>) -> Failed {
    Failed { what: what.into(), code: 1 }
}

/// One check being run.
#[derive(Debug)]
pub struct Check {
    /// Its name: `dev`, `pr` or `release`, then what it found (`pr, ALL`).
    pub mode: String,
    /// The repository's root, where every command runs.
    pub root: PathBuf,
    /// The build directory, from `cargo metadata`.
    pub target: PathBuf,
    /// The variables every command gets on top of this process's: the compiler cache.
    env: Vec<(String, OsString)>,
    /// The step being run, its title and when it began.
    step: Option<(String, Instant)>,
    started: Instant,
    started_at: u64,
    /// Seconds, kind (step, sub or failed) and title, one line per step, for check-timings.py.
    timings: Vec<(u64, &'static str, String)>,
    build_state: &'static str,
    /// `--timings` for the clippy and test builds, with RAVENRS_CHECK_TIMINGS=1.
    pub cargo_timings: bool,
}

impl Check {
    /// A check named `mode`, run from the repository at `root`.
    pub fn new(mode: &str, root: &Path) -> Result<Check, Failed> {
        let target = target_directory(root)?;
        fs::create_dir_all(&target).map_err(|e| failure(format!("{}: {e}", target.display())))?;
        // Worktrees that share a build directory take each other's local crates (skill ravenrs-check).
        let mark = target.join(".ravenrs-checkout");
        let here = root.display().to_string();
        if let Ok(last) = fs::read_to_string(&mark)
            && last.trim() != here
        {
            eprintln!("\x1b[1;33mwarning: {} was last used by {}.\x1b[0m", target.display(), last.trim());
            eprintln!(
                "Worktrees that share a build directory reuse each other's local crates; give this one its own CARGO_TARGET_DIR."
            );
        }
        let _ = fs::write(&mark, format!("{here}\n"));
        // An empty build directory and a reused one take very different times: the table says which.
        let empty = fs::read_dir(target.join("debug/deps")).map_or(true, |mut dir| dir.next().is_none());
        let started_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Ok(Check {
            mode: mode.to_owned(),
            root: root.to_owned(),
            target,
            env: Vec::new(),
            step: None,
            started: Instant::now(),
            started_at,
            timings: Vec::new(),
            build_state: if empty { "empty" } else { "reused" },
            cargo_timings: std::env::var("RAVENRS_CHECK_TIMINGS").is_ok_and(|v| v == "1"),
        })
    }

    /// Sets a variable for every command that follows.
    pub fn set_env(&mut self, name: &str, value: impl Into<OsString>) {
        self.env.retain(|(known, _)| known != name);
        self.env.push((name.to_owned(), value.into()));
    }

    /// Whether a variable was set for the commands.
    pub fn has_env(&self, name: &str) -> bool {
        self.env.iter().any(|(known, _)| known == name)
    }

    /// Starts a step, closing the one before.
    pub fn step(&mut self, title: &str) {
        self.close("step");
        println!("\n\x1b[1m==> {title}\x1b[0m");
        self.step = Some((title.to_owned(), Instant::now()));
    }

    fn close(&mut self, kind: &'static str) {
        if let Some((title, began)) = self.step.take() {
            self.timings.push((began.elapsed().as_secs(), kind, title));
        }
    }

    /// A command, ready to run from the repository's root with the check's variables.
    pub fn command(&self, program: &str, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        command.args(args).current_dir(&self.root);
        for (name, value) in &self.env {
            command.env(name, value);
        }
        command
    }

    /// Runs a command; it fails the check unless it ends with code 0.
    pub fn run(&self, program: &str, args: &[&str]) -> Done {
        self.run_command(self.command(program, args), program, args)
    }

    /// Runs a command that was set up apart (with a variable of its own, say).
    pub fn run_command(&self, mut command: Command, program: &str, args: &[&str]) -> Done {
        let what = std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
        let status = command.status().map_err(|e| failure(format!("{what}: {e}")))?;
        if status.success() { Ok(()) } else { Err(Failed { what, code: status.code().unwrap_or(1) }) }
    }

    /// Runs a command whose output is read: what it wrote to stdout, if it ended with code 0.
    pub fn output(&self, program: &str, args: &[&str]) -> Result<String, Failed> {
        let what = std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
        let output = self.command(program, args).output().map_err(|e| failure(format!("{what}: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            std::io::stderr().write_all(&output.stderr).ok();
            Err(Failed { what, code: output.status.code().unwrap_or(1) })
        }
    }

    /// A part of the current step, timed on its own.
    pub fn substep(&mut self, title: &str, program: &str, args: &[&str]) -> Done {
        let began = Instant::now();
        let done = self.run(program, args);
        self.timings.push((began.elapsed().as_secs(), "sub", title.to_owned()));
        done
    }

    /// Ends the check: the table of times, the compiler cache's numbers, and the line that says how
    /// it went. Returns the process's exit code.
    pub fn finish(mut self, done: Done, passed: &str, cache: &crate::cache::Cache) -> i32 {
        let code = match &done {
            Ok(()) => 0,
            Err(failed) => failed.code.max(1),
        };
        self.close(if code == 0 { "step" } else { "failed" });
        let timings_dir = self.target.join("check-timings");
        let steps = timings_dir.join("current.tsv");
        let lines: String = self
            .timings
            .iter()
            .map(|(seconds, kind, title)| format!("{seconds}\t{kind}\t{title}\n"))
            .collect();
        let total = self.started.elapsed().as_secs();
        if fs::create_dir_all(&timings_dir).and_then(|()| fs::write(&steps, lines)).is_ok() {
            let mut report = self.command(
                PYTHON,
                &[
                    "scripts/check-timings.py",
                    "--total",
                    &total.to_string(),
                    "--since",
                    &self.started_at.to_string(),
                    "--status",
                    &code.to_string(),
                    "--mode",
                    &self.mode,
                    "--build",
                    self.build_state,
                    "--junit",
                    "target/nextest/default/junit.xml",
                ],
            );
            report.arg("--steps").arg(&steps).arg("--save").arg(timings_dir.join("last.txt"));
            if self.cargo_timings {
                report.arg("--cargo-timings").arg(self.target.join("cargo-timings"));
            }
            let _ = report.status();
        }
        cache.report(&self);
        match done {
            Ok(()) => println!("\n\x1b[1;32m{passed} in {}m{:02}s.\x1b[0m", total / 60, total % 60),
            Err(failed) => eprintln!("\n\x1b[1;31m{failed}.\x1b[0m"),
        }
        code
    }
}

/// The build directory Cargo uses for this workspace, as `cargo metadata` says.
fn target_directory(root: &Path) -> Result<PathBuf, Failed> {
    let args = ["metadata", "--format-version", "1", "--no-deps", "--offline"];
    let output = Command::new("cargo")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| failure(format!("cargo metadata: {e}")))?;
    if !output.status.success() {
        std::io::stderr().write_all(&output.stderr).ok();
        return Err(failure("cargo metadata"));
    }
    json_string(&String::from_utf8_lossy(&output.stdout), "target_directory")
        .map(PathBuf::from)
        .ok_or_else(|| failure("cargo metadata gives no target_directory"))
}

/// The string value of `key` in a JSON object, its escapes read. Enough for Cargo's metadata, which
/// is all this crate reads, without a JSON dependency to compile before any check can start.
pub fn json_string(json: &str, key: &str) -> Option<String> {
    let at = json.find(&format!("\"{key}\":\""))? + key.len() + 4;
    let mut value = String::new();
    let mut chars = json[at..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(value),
            '\\' => match chars.next()? {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    value.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => value.push(other),
            },
            c => value.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_string_of_cargo_metadata_is_read_with_its_escapes() {
        let json = r#"{"packages":[],"target_directory":"C:\\Users\\a \"b\"\u00e9","version":1}"#;
        assert_eq!(json_string(json, "target_directory").as_deref(), Some("C:\\Users\\a \"b\"é"));
        assert_eq!(json_string(json, "workspace_root"), None);
    }
}
