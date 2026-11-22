//! The plugin `agent-local` of Raven CI: a build agent on the server's machine.
//!
//! # Its role
//!
//! A server that runs its builds itself runs them as its own user, next to its vault, its
//! database and its keys. With `agent-local`, the builds run in a `raven-ci-agent` on the same
//! machine, as another user when the template says one, and the server runs with `--executores 0`:
//! it does not compile, and a build reads none of its files.
//!
//! # How it works
//!
//! It is an [`AgentLauncher`] whose id is `agent-local`. When a build with a label has no agent and
//! a template of agents with that label names `agent-local`, the core asks it to bring one up:
//!
//! - it starts the agent's program (the template's `programa`, or `raven-ci-agent` beside the
//!   program that runs the server) through `ravenrs-process`, with the server's address, the name,
//!   the labels, the executors and the limits as arguments, and the token used once in
//!   `RAVEN_CI_AGENT_TOKEN`, so it never shows in the list of processes;
//! - in the folder `pasta` of the template, or `agentes/<name>` in the plugin's own folder;
//! - as the user `usuario` of the template, when it says one (`ravenrs_process::Spec::user`: as
//!   root the user is switched; otherwise `sudo -n`, which needs a rule for it);
//! - each line the agent writes goes to the server's log with its name.
//!
//! The agent connects back to the server and gets in with the token. Stopping it stops the agent
//! with what it started.
//!
//! # How to use it
//!
//! ```text
//! raven-ci agentes modelo local --lancador agent-local --rotulos local --executores 2 \
//!     --config '{"usuario": "ci", "nucleos": 4, "memoria": "8G"}'
//! raven-ci servir --executores 0
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_core::agents::{ChildAgent, TOKEN_VAR, agent_arguments, launch_name, launch_text};
use raven_ci_plugin_api::{AgentLauncher, CiError, CiPlugin, Host, LaunchRequest, LaunchedAgent, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use ravenrs_process::Spec;

/// The id of the launcher.
pub const LAUNCHER: &str = "agent-local";

/// The name of the agent's program.
pub const PROGRAM: &str = if cfg!(windows) { "raven-ci-agent.exe" } else { "raven-ci-agent" };

/// The plugin `agent-local`.
#[derive(Default)]
pub struct AgentLocalPlugin {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

impl std::fmt::Debug for AgentLocalPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentLocalPlugin").finish_non_exhaustive()
    }
}

impl AgentLocalPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for AgentLocalPlugin {
    fn name(&self) -> &str {
        LAUNCHER
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Process.capability())
            .capability(Permission::Disk.capability())
    }
}

impl CiPlugin for AgentLocalPlugin {
    fn summary(&self) -> String {
        "Sobe agentes de build na máquina do servidor, como outro usuário quando o modelo diz.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(EntityInfo::new("AgentLocalSettings").field(FieldInfo::new("programa", FieldKind::Text)))
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "subir o programa dos agentes")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn agent_launchers(&self) -> Vec<Arc<dyn AgentLauncher>> {
        vec![Arc::new(Local { host: self.host.clone() })]
    }
}

/// The launcher `agent-local`. See the [crate](crate).
struct Local {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

/// The agent's program: the template's, the plugin's, or the one beside the server's.
fn program(request: &LaunchRequest, host: &dyn Host) -> Result<PathBuf, CiError> {
    if let Some(program) = launch_text(request, "programa") {
        return Ok(PathBuf::from(program));
    }
    if let Some(program) = host.settings().get("programa").and_then(|p| p.as_str()).filter(|p| !p.is_empty())
    {
        return Ok(PathBuf::from(program));
    }
    let server =
        std::env::current_exe().map_err(|e| CiError::failed(format!("o programa do servidor: {e}")))?;
    let beside = server.with_file_name(PROGRAM);
    if beside.is_file() {
        Ok(beside)
    } else {
        Err(CiError::unavailable(format!(
            "não achei {} ao lado do servidor; diga onde ele está em programa",
            beside.display()
        )))
    }
}

impl AgentLauncher for Local {
    fn id(&self) -> &str {
        LAUNCHER
    }

    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, CiError> {
        let host = self
            .host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| CiError::unavailable("o plugin agent-local está desligado"))?;
        let name = launch_name(request);
        let dir = match launch_text(request, "pasta") {
            Some(dir) => PathBuf::from(dir),
            None => host.data_dir()?.join("agentes").join(&name),
        };
        std::fs::create_dir_all(&dir).map_err(|e| CiError::failed(format!("{}: {e}", dir.display())))?;
        let program = program(request, host.as_ref())?;
        let mut spec = Spec::new(program)
            .args(agent_arguments(request, &dir.display().to_string()))
            .env(TOKEN_VAR, request.token.clone());
        if let Some(user) = launch_text(request, "usuario") {
            spec = spec.user(user);
        }
        let log = host.clone();
        let agent = ChildAgent::start(&name, spec, move |line| log.log(line))?;
        Ok(Box::new(agent))
    }
}
