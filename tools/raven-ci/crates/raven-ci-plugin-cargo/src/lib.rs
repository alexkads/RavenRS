//! The plugin `cargo` of Raven CI: the step `cargo`, each error and warning with its file and
//! line, and the compilation of a job kept for its next build.
//!
//! # Its role
//!
//! A Rust build in a `sh` works, but its log is a wall of text and every build compiles from
//! nothing. This plugin runs `cargo` knowing what it says: each diagnostic goes to the log as the
//! compiler renders it and is kept, with its file and line, as an attachment of the build; and the
//! builds of one job share one target folder, so the second build finds what the first compiled.
//!
//! # How it works
//!
//! - **The step** is a node of kind [`CARGO`] with the parameter `command`, the words after
//!   `cargo` (`{ cargo = "clippy -p site -- -D warnings" }` in a `.raven-ci.toml`). For `build`,
//!   `check`, `clippy`, `test`, `bench`, `doc`, `run` and `rustc`, it adds
//!   `--message-format=json-diagnostic-rendered-ansi` before a `--`: each line of JSON that is a
//!   diagnostic goes to the log rendered, the other JSON lines are left out, and every other line
//!   (the output of tests, `Compiling…`) goes as it came. It runs as `sh` runs a program, through
//!   `raven_ci_plugin_shell::watch`: in the build's folder, with its variables, stopped when the
//!   build is cancelled or runs out of time.
//! - **The diagnostics** (level, code, message, file, line, column) of the errors and warnings
//!   go to `.raven-ci/cargo-diagnostics.jsonl` in the build's folder, one line each, with the
//!   step. The publisher `cargo` reads them at the end of the build and keeps the attachment
//!   `cargo-diagnostics.json`, of kind `cargo`, with the counts in its summary. The format
//!   `.raven-ci.toml` adds the publisher by itself when a pipeline has a step `cargo`.
//! - **One target per job.** `CARGO_TARGET_DIR` is a folder of the plugin per job
//!   (`targets/<job>`), held by a lock on a file while a step uses it, so two builds of one job
//!   do not compile into it at once; the lock is not handed to the programs cargo starts. A build
//!   whose variables already say `CARGO_TARGET_DIR`, or the setting `shared_target = false`,
//!   keeps cargo's own.
//! - **`ravenrs-xcache`**, with `xcache = true`, is the `RUSTC_WRAPPER` (with
//!   `CARGO_INCREMENTAL=0`), as `scripts/check.sh` does. Without the program, a line in the log
//!   and the build goes on.
//! - **Refused**: with no `cargo` on the agent the step fails with "falta o cargo neste agente";
//!   a cargo that fails says its code and its first error.
//!
//! # Settings
//!
//! | Setting | Default | What it does |
//! |---|---|---|
//! | `shared_target` | `true` | one `CARGO_TARGET_DIR` per job, kept between builds |
//! | `xcache` | `false` | `ravenrs-xcache` as the `RUSTC_WRAPPER` |
//! | `xcache_program` | `ravenrs-xcache` | the program, by name in the `PATH` or by its path |
//! | `program` | `cargo` | the program run, by name in the `PATH` or by its path |

mod diagnostics;

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{
    Attachment, BuildInfo, BuildScope, CiError, CiPlugin, Host, Permission, Publisher,
};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use ravenrs_flows::{Catalog, NodeContext, NodeError, NodeKind, NodeSpec, Ports, Reconciled};
use ravenrs_process::{End, Spec};
use serde_json::{Value, json};

pub use diagnostics::{DIAGNOSTICS_FILE, Diagnostic, Level};

/// The kind of the step.
pub const CARGO: &str = "cargo";
/// The kind of the attachment the publisher keeps.
pub const ATTACHMENT_KIND: &str = "cargo";
/// The name of the attachment.
pub const ATTACHMENT_NAME: &str = "cargo-diagnostics.json";
/// The subcommands that take `--message-format`.
const JSON_COMMANDS: &[&str] = &["build", "check", "clippy", "test", "bench", "doc", "run", "rustc"];
/// What makes cargo say its diagnostics as JSON, each with its rendered text.
const MESSAGE_FORMAT: &str = "--message-format=json-diagnostic-rendered-ansi";
/// How often a step waiting for the target of its job looks again.
const LOOK_EVERY_MS: u64 = 200;

/// The plugin `cargo`.
#[derive(Default)]
pub struct CargoPlugin {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

impl std::fmt::Debug for CargoPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CargoPlugin").finish_non_exhaustive()
    }
}

impl CargoPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for CargoPlugin {
    fn name(&self) -> &str {
        "cargo"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Process.capability())
            .capability(Permission::Disk.capability())
    }
}

impl CiPlugin for CargoPlugin {
    fn summary(&self) -> String {
        "O passo cargo: erros e avisos com arquivo e linha, guardados no build, e o alvo de cada job entre builds."
            .to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(
            EntityInfo::new("CargoSettings")
                .field(FieldInfo::new("shared_target", FieldKind::Boolean))
                .field(FieldInfo::new("xcache", FieldKind::Boolean))
                .field(FieldInfo::new("xcache_program", FieldKind::Text))
                .field(FieldInfo::new("program", FieldKind::Text)),
        )
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o cargo dos passos cargo")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Cargo { host: self.host.clone() });
    }

    fn publishers(&self) -> Vec<Arc<dyn Publisher>> {
        vec![Arc::new(CargoPublisher)]
    }
}

/// The words of `text`, split at blanks, where a part between quotes (`'a b'`, `"a b"`) is one.
pub fn words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in text.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// The arguments of `cargo` for `command`: its words, with the JSON of diagnostics before a `--`
/// when the subcommand takes it and the words do not choose a format already.
pub fn arguments(command: &str) -> Vec<String> {
    let mut args = words(command);
    let takes = args.first().is_some_and(|sub| JSON_COMMANDS.contains(&sub.as_str()));
    let chosen = args.iter().any(|a| a.starts_with("--message-format"));
    if takes && !chosen {
        let at = args.iter().position(|a| a == "--").unwrap_or(args.len());
        args.insert(at, MESSAGE_FORMAT.to_owned());
    }
    args
}

/// `job` as a path under the folder of targets: each part of its name kept to letters, digits,
/// `-`, `_` and `.`, never `..`.
fn job_folder(job: &str) -> PathBuf {
    job.split('/')
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .map(|part| {
            part.chars()
                .map(|c| if c.is_alphanumeric() || "-_.".contains(c) { c } else { '_' })
                .collect::<String>()
        })
        .collect()
}

/// Where `program` is: itself when it is a path, or found in the folders of `path`.
fn find_program(program: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let given = Path::new(program);
    if given.components().count() > 1 {
        return given.is_file().then(|| given.to_path_buf());
    }
    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{program}.exe"), program.to_owned()]
    } else {
        vec![program.to_owned()]
    };
    std::env::split_paths(path?)
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|p| p.is_file())
}

/// The step `cargo`. See the [crate](crate).
struct Cargo {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

impl Cargo {
    fn host(&self) -> Result<Arc<dyn Host>, NodeError> {
        self.host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| NodeError::new("o plugin cargo está desligado"))
    }

    /// The target of `job`, made, and its lock taken; waits while another build holds it.
    fn target(
        &self,
        cx: &NodeContext<'_>,
        host: &dyn Host,
        job: &str,
        log: &dyn Fn(&str),
    ) -> Result<(PathBuf, File), NodeError> {
        let dir =
            host.data_dir().map_err(|e| NodeError::new(e.to_string()))?.join("targets").join(job_folder(job));
        std::fs::create_dir_all(&dir)
            .map_err(|e| NodeError::new(format!("o alvo {}: {e}", dir.display())))?;
        let path = dir.join(".raven-ci-cargo.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| NodeError::new(format!("a trava {}: {e}", path.display())))?;
        let mut said = false;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok((dir, file)),
                Err(TryLockError::WouldBlock) => {
                    if !said {
                        log("esperando o alvo do job, que outro build está usando");
                        said = true;
                    }
                    cx.sleep(LOOK_EVERY_MS)?;
                }
                Err(TryLockError::Error(e)) => {
                    return Err(NodeError::new(format!("a trava {}: {e}", path.display())));
                }
            }
        }
    }
}

fn setting_bool(settings: &Value, name: &str, default: bool) -> bool {
    settings.get(name).and_then(Value::as_bool).unwrap_or(default)
}

fn setting_text<'a>(settings: &'a Value, name: &str, default: &'a str) -> &'a str {
    settings.get(name).and_then(Value::as_str).filter(|t| !t.trim().is_empty()).unwrap_or(default)
}

impl NodeKind for Cargo {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(CARGO, 1).title("Cargo").input("main").output("main").requires("command")
    }

    /// A compilation whose worker died in the middle runs again: cargo picks up what it kept.
    fn reconcile(&self, _: &NodeContext<'_>) -> Reconciled {
        Reconciled::NotDone
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let host = self.host()?;
        let settings = host.settings();
        let step = cx.node().id.clone();
        let log = |line: &str| build.log(&step, line);
        let command = cx
            .parameter("command")
            .and_then(Value::as_str)
            .ok_or_else(|| NodeError::new("o passo cargo precisa do comando, em texto, como \"test\""))?;
        let args = arguments(command);
        if args.is_empty() {
            return Err(NodeError::new("o passo cargo precisa do comando, como \"test\" ou \"clippy\""));
        }
        let environment = build.environment();
        let path = environment.get("PATH").map(std::ffi::OsString::from).or_else(|| std::env::var_os("PATH"));
        let program = setting_text(&settings, "program", "cargo");
        let Some(cargo) = find_program(program, path.as_deref()) else {
            return Err(NodeError::new(format!("falta o cargo neste agente: {program} não está no PATH")));
        };

        let context = build.context();
        let mut spec =
            Spec::new(cargo.to_string_lossy().into_owned()).args(args.clone()).dir(&context.workspace);
        for (name, value) in environment {
            spec = spec.env(name, value);
        }
        // Held while the step runs, let go when it ends however it ends.
        let mut _held: Option<File> = None;
        if setting_bool(&settings, "shared_target", true) && !environment.contains_key("CARGO_TARGET_DIR") {
            let (dir, lock) = self.target(cx, host.as_ref(), &context.job, &log)?;
            log(&format!("CARGO_TARGET_DIR do job: {}", dir.display()));
            spec = spec.env("CARGO_TARGET_DIR", dir.to_string_lossy());
            _held = Some(lock);
        }
        if setting_bool(&settings, "xcache", false) && !environment.contains_key("RUSTC_WRAPPER") {
            let wanted = setting_text(&settings, "xcache_program", "ravenrs-xcache");
            match find_program(wanted, path.as_deref()) {
                Some(xcache) => {
                    spec = spec.env("RUSTC_WRAPPER", xcache.to_string_lossy()).env("CARGO_INCREMENTAL", "0");
                }
                None => {
                    log(&format!("o ravenrs-xcache ({wanted}) não está neste agente: compilando sem o cache"))
                }
            }
        }
        if let Some(ms) = cx.node().timeout_ms {
            spec = spec.timeout(Duration::from_millis(ms));
        }

        let found = Mutex::new(Vec::new());
        let read = |line: &str| match diagnostics::read_line(line) {
            diagnostics::Line::Diagnostic(diagnostic, rendered) => {
                for text in rendered.lines() {
                    log(text);
                }
                found.lock().unwrap_or_else(PoisonError::into_inner).push(diagnostic);
            }
            diagnostics::Line::Rendered(rendered) => {
                for text in rendered.lines() {
                    log(text);
                }
            }
            diagnostics::Line::Quiet => {}
            diagnostics::Line::Text => log(line),
        };
        let watched = raven_ci_plugin_shell::watch(cx, spec, &read, &|| {})?;
        let found = found.into_inner().unwrap_or_else(PoisonError::into_inner);
        if let Err(e) = diagnostics::keep(&context.workspace, &step, &found) {
            log(&format!("não consegui guardar os diagnósticos: {e}"));
        }
        let errors = found.iter().filter(|d| d.level == Level::Error).count();
        let warnings = found.iter().filter(|d| d.level == Level::Warning).count();
        if errors + warnings > 0 {
            log(&format!("cargo {}: {errors} erro(s) e {warnings} aviso(s)", args[0]));
        }
        match watched.end {
            End::Code(code) if code != 0 => {
                let first = found
                    .iter()
                    .find(|d| d.level == Level::Error)
                    .map(|d| format!(": {}", d.short()))
                    .unwrap_or_else(|| {
                        let last = watched.last_error();
                        if last.is_empty() { String::new() } else { format!(": {last}") }
                    });
                let message = format!("o cargo terminou com o código {code}{first}");
                if cx.node().retry.is_some() {
                    Err(NodeError::retryable(message))
                } else {
                    Err(NodeError::new(message))
                }
            }
            end => {
                raven_ci_plugin_shell::ended(end, cx.node().retry.is_some()).map(|()| BuildScope::pass_on(cx))
            }
        }
    }
}

/// The publisher `cargo`: the diagnostics the steps `cargo` kept, as one attachment.
struct CargoPublisher;

impl Publisher for CargoPublisher {
    fn id(&self) -> &str {
        "cargo"
    }

    fn publish(
        &self,
        _build: &BuildInfo,
        _settings: &Value,
        workspace: &Path,
    ) -> Result<Vec<Attachment>, CiError> {
        let Some(found) = diagnostics::read(workspace).map_err(CiError::failed)? else {
            return Ok(Vec::new());
        };
        let errors = found.iter().filter(|d| d.level == Level::Error).count();
        let warnings = found.iter().filter(|d| d.level == Level::Warning).count();
        let steps: std::collections::BTreeSet<&str> = found.iter().map(|d| d.step.as_str()).collect();
        let bytes = serde_json::to_vec_pretty(&found).map_err(|e| CiError::failed(e.to_string()))?;
        let mut attachment = Attachment::new(ATTACHMENT_KIND, ATTACHMENT_NAME, "application/json", bytes);
        attachment.summary = json!({ "errors": errors, "warnings": warnings, "steps": steps.len() });
        Ok(vec![attachment])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_split_at_blanks_and_keep_what_is_between_quotes() {
        assert_eq!(words("test -p site  --features 'a b'"), ["test", "-p", "site", "--features", "a b"]);
        assert_eq!(words("clippy -- -D warnings"), ["clippy", "--", "-D", "warnings"]);
        assert_eq!(words("run -- \"\""), ["run", "--", ""]);
        assert!(words("   ").is_empty());
    }

    #[test]
    fn the_json_of_diagnostics_goes_before_the_double_dash_only_for_commands_that_take_it() {
        assert_eq!(arguments("clippy -- -D warnings"), ["clippy", MESSAGE_FORMAT, "--", "-D", "warnings"]);
        assert_eq!(arguments("test -p site"), ["test", "-p", "site", MESSAGE_FORMAT]);
        assert_eq!(arguments("fmt --check"), ["fmt", "--check"]);
        assert_eq!(arguments("build --message-format=short"), ["build", "--message-format=short"]);
    }

    #[test]
    fn a_job_is_a_folder_that_never_leaves_the_targets() {
        assert_eq!(job_folder("apps/site"), PathBuf::from("apps").join("site"));
        assert_eq!(job_folder("../../etc/x y"), PathBuf::from("etc").join("x_y"));
        assert_eq!(job_folder("site/PR-12"), PathBuf::from("site").join("PR-12"));
    }

    #[test]
    fn a_program_is_found_in_the_path_or_by_its_own_path() {
        let dir = std::env::temp_dir().join(format!("raven-ci-cargo-find-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("folder");
        let name = if cfg!(windows) { "falso.exe" } else { "falso" };
        std::fs::write(dir.join(name), b"").expect("file");
        let path = std::env::join_paths([&dir]).expect("path");
        assert!(find_program("falso", Some(&path)).is_some());
        assert!(find_program("outro", Some(&path)).is_none());
        assert_eq!(find_program(&dir.join(name).to_string_lossy(), None), Some(dir.join(name)));
        std::fs::remove_dir_all(&dir).expect("removed");
    }
}
