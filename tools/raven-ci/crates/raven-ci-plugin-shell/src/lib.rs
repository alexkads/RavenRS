//! The plugin `shell` of Raven CI: the step `sh`.
//!
//! # Its role
//!
//! Most of what a pipeline does is a command: build, test, pack. The core runs no program; this
//! plugin brings the kind of step that does, so a pipeline with no `shell` switched on is refused
//! with "falta o plugin shell".
//!
//! # How it works
//!
//! A step `sh` runs its `script` through `ravenrs-process`:
//!
//! - with `sh -e -c` on Linux and macOS (it stops at the first command that fails) and `cmd /C`
//!   on Windows, never through a shell the step did not ask for;
//! - in the folder of the build, with the variables of the build ([`BuildScope::environment`])
//!   and the few basic ones of `ravenrs-process` (`PATH`, `HOME`…): nothing else of the server's
//!   environment reaches it;
//! - each line it writes, on its output or its errors, goes to the build's log as it comes, with
//!   the secrets hidden by the log;
//! - with `nice`, it runs under `nice -n N` on Linux and macOS; on Windows the setting is said in
//!   the log and left;
//! - with `credentials` (#922), each variable gets a credential of the vault, read through the
//!   host when the step starts, as Jenkins's `withCredentials` does: `{ "TOKEN": "nome" }` puts the
//!   value the credential gives plugins, `{ "USUARIO": "nome:user" }` one field of it, and
//!   `{ "KUBECONFIG": { "file": "nome", "name": "config" } }` writes it to a file only its owner
//!   reads, in [`BuildContext::secrets_dir`](raven_ci_plugin_api::BuildContext::secrets_dir), and
//!   puts the file's path. The value goes only into the environment of this command, never into
//!   its words; the log hides it and every value the credential hides; the file goes when the
//!   step ends, failed or cancelled too. On an agent the value comes from the server (`segredo`),
//!   which audits it;
//! - when the build is cancelled or the step's time limit (`timeout_ms` of its node) passes, the
//!   program is stopped with every program it started;
//! - when it ends on its own, what it left running in the background (`sleep 600 &`, a server of
//!   `sccache` started by the build) is stopped too, at once, and the log names it (#931): nothing
//!   of a build outlives it, nor keeps the mount namespace or the `/tmp` of a server that was
//!   restarted since. With `keep_daemons = true` it stays, and the log says so. A daemon that
//!   leaves the process group itself (`setsid`) is not reached.
//!
//! It ends well when the program ends with code 0, and fails with "o comando terminou com o
//! código N" otherwise: a failure worth trying again when its node has a `retry`, so the run
//! tries it again. It passes on the items it got, so the next step finds its build.
//!
//! # How to use it
//!
//! A format makes a node of kind [`SH`] with the parameters `script` and, when it wants, `nice`.
//! The plugin needs the permission `ci.process`.
//!
//! A step of another plugin that runs a program (`container` of `docker`, `kubectl` of
//! `kubernetes`) watches it with [`watch`], the same loop `sh` uses: each line to the log, the
//! program stopped when the build is cancelled or runs out of time, and the last lines of its
//! errors kept for the message of a failure.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{BuildScope, CiError, CiPlugin, Host, Permission};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_flows::{Catalog, NodeContext, NodeError, NodeKind, NodeSpec, Ports, Reconciled};
use ravenrs_process::{End, Event, Spec};
use serde_json::Value;

/// The kind of the step.
pub const SH: &str = "sh";

/// How often a running step looks at whether it was cancelled or ran out of time.
const LOOK_EVERY: Duration = Duration::from_millis(50);

type Shared = Arc<Mutex<Option<Arc<dyn Host>>>>;

/// The plugin `shell`.
#[derive(Default)]
pub struct ShellPlugin {
    host: Shared,
}

impl std::fmt::Debug for ShellPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellPlugin").finish_non_exhaustive()
    }
}

impl ShellPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for ShellPlugin {
    fn name(&self) -> &str {
        "shell"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Process.capability())
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for ShellPlugin {
    fn summary(&self) -> String {
        "O passo sh: roda comandos na pasta do build, com o log linha a linha.".to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar os comandos dos passos sh")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn steps(&self, catalog: &mut Catalog) {
        catalog.add(Sh { host: self.host.clone() });
    }
}

/// How many of the last lines of its errors [`watch`] keeps.
pub const KEPT_ERRORS: usize = 20;

/// What [`watch`] saw of a program that ended on its own or was stopped by its own time limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watched {
    /// How it ended.
    pub end: End,
    /// The last lines of its errors, up to [`KEPT_ERRORS`], in order: what a failure says.
    pub errors: Vec<String>,
    /// What it left running in the background and was stopped when it ended, by name, when its
    /// spec said [`Spec::stop_leftovers`].
    pub leftovers: Vec<String>,
}

impl Watched {
    /// The last line of its errors that says something, or an empty text.
    pub fn last_error(&self) -> &str {
        self.errors.iter().rev().map(|line| line.trim()).find(|line| !line.is_empty()).unwrap_or_default()
    }
}

/// Runs `spec` for the node of `cx`, giving each line it writes to `log` as it comes, and
/// watching the build: when it is cancelled or the node passes its time limit, `on_stop` runs
/// first (what stops what the program started elsewhere: `docker rm -f` of its container) and
/// then the program is stopped with every program it started, and the reason is the error.
///
/// It fails when the program does not start; otherwise it gives how it ended, with the last lines
/// of its errors.
pub fn watch(
    cx: &NodeContext<'_>,
    spec: Spec,
    log: &dyn Fn(&str),
    on_stop: &dyn Fn(),
) -> Result<Watched, NodeError> {
    let running = spec.spawn().map_err(|e| NodeError::new(format!("o comando não começou: {e}")))?;
    let control = running.control();
    let mut errors: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut stopped: Option<NodeError> = None;
    let end = loop {
        match running.next_within(LOOK_EVERY) {
            Some(Event::Out(line)) => log(&line),
            Some(Event::Err(line)) => {
                log(&line);
                if errors.len() == KEPT_ERRORS {
                    errors.pop_front();
                }
                errors.push_back(line);
            }
            Some(Event::Ended(end)) => break end,
            None => {}
        }
        if stopped.is_none()
            && let Err(why) = cx.check()
        {
            log(&format!("parando o comando: {}", why.message));
            on_stop();
            control.stop();
            stopped = Some(why);
        }
    };
    match stopped {
        Some(why) => Err(why),
        None => Ok(Watched { end, errors: errors.into(), leftovers: control.leftovers() }),
    }
}

/// How `end` reads as the end of a step: code 0 is fine; any other code fails with "o comando
/// terminou com o código N", worth trying again when `retry` (the node has a `retry`, as in a
/// `retry` of a Jenkinsfile); a time limit and a stop fail as such.
pub fn ended(end: End, retry: bool) -> Result<(), NodeError> {
    match end {
        End::Code(0) => Ok(()),
        End::Code(code) if retry => {
            Err(NodeError::retryable(format!("o comando terminou com o código {code}")))
        }
        End::Code(code) => Err(NodeError::new(format!("o comando terminou com o código {code}"))),
        End::Signal(signal) => Err(NodeError::new(format!("o comando foi morto pelo sinal {signal}"))),
        End::TimedOut => Err(NodeError::timed_out("o comando passou do prazo e foi parado")),
        End::Stopped => Err(NodeError::cancelled("o comando foi parado")),
    }
}

/// The step `sh`. See the [crate](crate). Made with [`Default`], outside the plugin (a test of
/// its own), it reads no credential.
#[derive(Clone, Default)]
pub struct Sh {
    host: Shared,
}

impl std::fmt::Debug for Sh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sh").finish_non_exhaustive()
    }
}

/// The secret files of one step, removed when it ends, however it ends.
struct SecretFiles(Option<PathBuf>);

impl Drop for SecretFiles {
    fn drop(&mut self) {
        if let Some(dir) = &self.0 {
            let _ = std::fs::remove_dir_all(dir);
            // The folder of the build's secrets, when no other step has one there now.
            if let Some(above) = dir.parent() {
                let _ = std::fs::remove_dir(above);
            }
        }
    }
}

/// A name of a variable: letters, digits and `_`, not starting with a digit.
fn variable(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Writes `content` to `path`, readable by its owner only.
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(content.as_bytes())
}

impl Sh {
    /// The variables the step's `credentials` ask, read through the host, and the secret files
    /// written for it; every value is hidden by the log from here on.
    fn credentials(
        &self,
        asked: &Value,
        build: &BuildScope,
        step: &str,
    ) -> Result<(Vec<(String, String)>, SecretFiles), NodeError> {
        let Some(asked) = asked.as_object().filter(|asked| !asked.is_empty()) else {
            return Ok((Vec::new(), SecretFiles(None)));
        };
        let host = self
            .host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| NodeError::new("o passo sh só lê credenciais com o plugin shell ligado"))?;
        let mut variables = Vec::new();
        let mut files = SecretFiles(None);
        for (name, wanted) in asked {
            if !variable(name) {
                return Err(NodeError::new(format!(
                    "credentials: {name:?} não é nome de variável (letras, números e _)"
                )));
            }
            let (credential, file) = match wanted {
                Value::String(credential) => (credential.as_str(), None),
                Value::Object(table) => match table.get("file").and_then(Value::as_str) {
                    Some(credential) => {
                        let file_name = table.get("name").and_then(Value::as_str).unwrap_or(name.as_str());
                        (credential, Some(file_name))
                    }
                    None => {
                        return Err(NodeError::new(format!(
                            "credentials.{name}: diga a credencial (\"nome\") ou o arquivo ({{ file = \"nome\" }})"
                        )));
                    }
                },
                _ => return Err(NodeError::new(format!("credentials.{name}: diga o nome da credencial"))),
            };
            let value =
                host.secret(credential).map_err(|e| NodeError::new(format!("credentials.{name}: {e}")))?;
            let value = match file {
                None => value,
                Some(file_name) => {
                    if file_name.is_empty() || file_name.contains(['/', '\\']) || file_name.starts_with("..")
                    {
                        return Err(NodeError::new(format!(
                            "credentials.{name}: o nome do arquivo {file_name:?} não serve"
                        )));
                    }
                    let dir = build.context().secrets_dir().join(step);
                    if files.0.is_none() {
                        std::fs::create_dir_all(&dir).map_err(|e| {
                            NodeError::new(format!("a pasta dos segredos {}: {e}", dir.display()))
                        })?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt as _;
                            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
                        }
                        files.0 = Some(dir.clone());
                    }
                    let path = dir.join(file_name);
                    write_private(&path, &value).map_err(|e| {
                        NodeError::new(format!("credentials.{name}: o arquivo {}: {e}", path.display()))
                    })?;
                    build.log(step, &format!("credencial {credential} no arquivo {}", path.display()));
                    path.display().to_string()
                }
            };
            variables.push((name.clone(), value));
        }
        Ok((variables, files))
    }
}

/// The program and its words that run `script`, with `nice` when asked and possible.
fn command(script: &str, nice: Option<i64>, log: &dyn Fn(&str)) -> Spec {
    if cfg!(windows) {
        if nice.is_some() {
            log("nice não existe no Windows: o comando roda na prioridade normal");
        }
        return Spec::new("cmd").args(["/C", script]);
    }
    match nice {
        Some(level) => Spec::new("nice").args(["-n", &level.to_string(), "sh", "-e", "-c", script]),
        None => Spec::new("sh").args(["-e", "-c", script]),
    }
}

impl NodeKind for Sh {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(SH, 1).title("Comando").input("main").output("main").requires("script")
    }

    /// A command whose worker died in the middle (an agent that fell) runs again: the build's
    /// folder kept what it wrote, and a build command is made to be run again.
    fn reconcile(&self, _: &NodeContext<'_>) -> Reconciled {
        Reconciled::NotDone
    }

    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let build = BuildScope::of(cx)?;
        let step = cx.node().id.clone();
        let script = cx
            .parameter("script")
            .and_then(Value::as_str)
            .ok_or_else(|| NodeError::new("o passo sh precisa do script, em texto"))?;
        let nice = cx.parameter("nice").and_then(Value::as_i64);
        let log = |line: &str| build.log(&step, line);
        let mut spec = command(script, nice, &log).dir(&build.context().workspace);
        for (name, value) in build.environment() {
            spec = spec.env(name, value);
        }
        let asked = cx.parameter("credentials").cloned().unwrap_or(Value::Null);
        // The files stay until this function returns: after the program, however it ended.
        let (variables, _files) =
            self.credentials(&asked, &build, &step).inspect_err(|why| log(&why.message))?;
        for (name, value) in variables {
            spec = spec.env(name, value);
        }
        if let Some(ms) = cx.node().timeout_ms {
            // A second guard: the run stops waiting at its deadline, and this stops the program.
            spec = spec.timeout(Duration::from_millis(ms));
        }
        // What the command leaves in the background goes with it (#931), unless the step keeps it.
        let keep = cx.parameter("keep_daemons").and_then(Value::as_bool) == Some(true);
        if !keep {
            spec = spec.stop_leftovers();
        }
        let watched = watch(cx, spec, &log, &|| {})?;
        if !watched.leftovers.is_empty() {
            log(&format!(
                "o comando deixou {} processo(s) rodando ({}); parados, como todo passo faz (keep_daemons = true os deixa)",
                watched.leftovers.len(),
                watched.leftovers.join(", ")
            ));
        } else if keep {
            log("keep_daemons: o que o comando deixou rodando fica vivo depois do passo");
        }
        ended(watched.end, cx.node().retry.is_some()).map(|()| BuildScope::pass_on(cx))
    }
}
