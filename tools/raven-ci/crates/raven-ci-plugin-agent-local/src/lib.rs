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
//!   the labels, the executors and the limits as arguments (the contract of
//!   `raven_ci_plugin_api::AgentArgs`), and the token used once on its input (`--token -`), so it
//!   shows in no list of processes and in no log of `sudo`;
//! - in a folder of its own, `<pasta>/<name>` under the template's `pasta`, or `agentes/<name>` in
//!   the plugin's own folder, removed when the agent is taken down (#938) unless it runs as
//!   another `usuario`;
//! - as the user `usuario` of the template, when it says one (`ravenrs_process::Spec::user`: as
//!   root the user is switched; otherwise `sudo -n`, which needs a rule for it and a server
//!   without `NoNewPrivileges`, which is refused saying so). The agent makes its folder itself, so
//!   a template with `usuario` says `pasta`, one that user writes in;
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

use raven_ci_core::agents::{ChildAgent, launch_args, launch_name, launch_text, limit_arguments};
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

/// How many bytes the files under `dir` hold.
fn size_of(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => size_of(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

fn make(dir: PathBuf) -> Result<PathBuf, CiError> {
    std::fs::create_dir_all(&dir).map_err(|e| CiError::failed(format!("{}: {e}", dir.display())))?;
    Ok(dir)
}

/// Whether this process may switch to another user. A service with `NoNewPrivileges=true` (the
/// unit of the RPM) cannot: `sudo` refuses at once, and the agent would never come up.
fn switching_allowed() -> Result<(), CiError> {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    if no_new_privileges(&status) {
        return Err(CiError::unavailable(
            "o servidor roda com NoNewPrivileges, e assim o sudo não troca de usuário; para um agente com \
             usuario, ponha NoNewPrivileges=false num drop-in (sudo systemctl edit raven-ci)",
        ));
    }
    Ok(())
}

/// The line `NoNewPrivs: 1` of `/proc/<pid>/status` in a process that is not root (Linux; elsewhere
/// the file is not there). Root switches the user itself, which the flag does not stop.
fn no_new_privileges(status: &str) -> bool {
    let field = |name: &str| status.lines().find_map(|line| line.strip_prefix(name)).map(str::trim);
    let root = field("Uid:").and_then(|uids| uids.split_whitespace().nth(1)) == Some("0");
    !root && field("NoNewPrivs:") == Some("1")
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
        let user = launch_text(request, "usuario");
        if user.is_some() {
            switching_allowed()?;
        }
        // A folder of its own under `pasta`: an agent that found the key of the one before in a
        // folder they shared came in under the old name, and the launcher never saw its own.
        let dir = match (launch_text(request, "pasta"), user) {
            // The agent makes its own folder: one the server's user made is not the other user's
            // to write in (the key of the agent goes there).
            (Some(dir), Some(_)) => PathBuf::from(dir).join(&name),
            (Some(dir), None) => make(PathBuf::from(dir).join(&name))?,
            (None, Some(user)) => {
                return Err(CiError::settings(
                    "pasta",
                    format!(
                        "o agente roda como {user} e não grava na pasta de dados do servidor; diga em pasta \
                         uma pasta que {user} grava"
                    ),
                ));
            }
            (None, None) => make(host.data_dir()?.join("agentes").join(&name))?,
        };
        let program = program(request, host.as_ref())?;
        let args = launch_args(request, &dir.display().to_string());
        // The token on the input (`--token -`), as agent-remote gives it: in the environment, `sudo`
        // writes it to the journal of the system with every launch (#803, test of the real world).
        let mut spec = Spec::new(program)
            .args(args.to_args())
            .args(["--token".to_owned(), "-".to_owned()])
            .args(limit_arguments(request))
            .stdin();
        if let Some(user) = user {
            spec = spec.user(user);
        }
        let log = host.clone();
        let mut agent = ChildAgent::start(&name, spec, move |line| log.log(line))?;
        // The folder of an agent this server's user runs goes with it when it is taken down (#938):
        // an agent of a template is not coming back under that name. One of another `usuario` is
        // not this user's to remove.
        if user.is_none() {
            let (gone, said, who) = (dir.clone(), host.clone(), name.clone());
            agent = agent.at_stop(move || {
                let bytes = size_of(&gone);
                if std::fs::remove_dir_all(&gone).is_ok() {
                    said.log(&format!(
                        "agente {who}: a pasta {} saiu com ele ({} MB liberados)",
                        gone.display(),
                        bytes / (1024 * 1024)
                    ));
                }
            });
        }
        agent.send(&request.token);
        Ok(Box::new(agent))
    }
}

#[cfg(test)]
mod tests {
    use super::no_new_privileges;

    #[test]
    fn the_flag_stops_only_a_user_that_is_not_root() {
        let service = "Name:\traven-ci\nUid:\t953\t953\t953\t953\nNoNewPrivs:\t1\n";
        assert!(no_new_privileges(service));
        assert!(!no_new_privileges(&service.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0")));
        assert!(!no_new_privileges("Uid:\t0\t0\t0\t0\nNoNewPrivs:\t1\n"));
        assert!(!no_new_privileges(""), "macOS and Windows have no /proc");
    }
}
