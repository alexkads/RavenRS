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
//! - when the build is cancelled or the step's time limit (`timeout_ms` of its node) passes, the
//!   program is stopped with every program it started.
//!
//! It ends well when the program ends with code 0, and fails with "o comando terminou com o
//! código N" otherwise. It passes on the items it got, so the next step finds its build.
//!
//! # How to use it
//!
//! A format makes a node of kind [`SH`] with the parameters `script` and, when it wants, `nice`.
//! The plugin needs the permission `ci.process`.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{BuildScope, CiError, CiPlugin, Host, Permission};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_flows::{Catalog, NodeContext, NodeError, NodeKind, NodeSpec, Ports};
use ravenrs_process::{End, Event, Spec};
use serde_json::Value;

/// The kind of the step.
pub const SH: &str = "sh";

/// How often a running step looks at whether it was cancelled or ran out of time.
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// The plugin `shell`.
#[derive(Default)]
pub struct ShellPlugin {
    host: Mutex<Option<Arc<dyn Host>>>,
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
        catalog.add(Sh);
    }
}

/// The step `sh`. See the [crate](crate).
#[derive(Clone, Copy, Debug, Default)]
pub struct Sh;

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
        if let Some(ms) = cx.node().timeout_ms {
            // A second guard: the run stops waiting at its deadline, and this stops the program.
            spec = spec.timeout(Duration::from_millis(ms));
        }
        let running = spec.spawn().map_err(|e| NodeError::new(format!("o comando não começou: {e}")))?;
        let control = running.control();
        let mut stopped: Option<NodeError> = None;
        let end = loop {
            match running.next_within(LOOK_EVERY) {
                Some(Event::Out(line) | Event::Err(line)) => log(&line),
                Some(Event::Ended(end)) => break end,
                None => {}
            }
            if stopped.is_none()
                && let Err(why) = cx.check()
            {
                log(&format!("parando o comando: {}", why.message));
                control.stop();
                stopped = Some(why);
            }
        };
        if let Some(why) = stopped {
            return Err(why);
        }
        match end {
            End::Code(0) => Ok(BuildScope::pass_on(cx)),
            End::Code(code) => Err(NodeError::new(format!("o comando terminou com o código {code}"))),
            End::Signal(signal) => Err(NodeError::new(format!("o comando foi morto pelo sinal {signal}"))),
            End::TimedOut => Err(NodeError::timed_out("o comando passou do prazo e foi parado")),
            End::Stopped => Err(NodeError::cancelled("o comando foi parado")),
        }
    }
}
