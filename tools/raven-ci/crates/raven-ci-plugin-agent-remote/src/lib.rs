//! The plugin `agent-remote` of Raven CI: a build agent on another machine, through SSH.
//!
//! # Its role
//!
//! A build that needs another system (a Mac for an app, a Linux with a GPU) runs in a
//! `raven-ci-agent` on that machine. This plugin brings the agent up there through the SSH that
//! is there already, and the agent connects back to the server: nothing else is opened on the
//! machine, and nothing is installed by the plugin but what the template says to run.
//!
//! # How it works
//!
//! It is an [`AgentLauncher`] whose id is `agent-remote`. Asked for an agent, it runs
//!
//! ```text
//! ssh -T -o BatchMode=yes -o StrictHostKeyChecking=accept-new [-p PORTA] [-i CHAVE] USUARIO@HOST -- \
//!     'PROGRAMA' '--servidor' 'URL' '--nome' 'NOME' … '--token' '-'
//! ```
//!
//! through `ravenrs-process`, and writes the token used once on the input of `ssh`, which the agent
//! reads from its own (`--token -`): it shows in no list of processes on either machine. With
//! `BatchMode`, a machine that asks for a password fails at once instead of waiting.
//!
//! - `host`, `usuario`, `porta`: where to.
//! - `programa`: the agent's program there (`raven-ci-agent` in its `PATH` unless said); `pasta`, its
//!   folder there (`raven-ci-agente/<name>` in the user's home unless said).
//! - `chave`: a credential `ssh-key` of the vault, by name. Its key goes to a file of the plugin's,
//!   readable only by its owner, and is removed when the agent stops. Without it, `ssh` uses the
//!   keys of the server's user.
//! - `ssh`: the program; `ssh`, which Linux, macOS and Windows 10+ have, unless said.
//! - `sistema`: the other machine's shell, `posix` (the default) or `windows` (#906). The OpenSSH of
//!   Windows hands the line to `cmd.exe`, which reads no single quotes: with `windows`, each word
//!   goes in double quotes with a `"` inside doubled, the program is `raven-ci-agent.exe` and the
//!   folder `%USERPROFILE%\raven-ci-agente\<name>` unless said.
//!
//! The server's address the agent connects to is the one `raven-ci servir --url-agentes URL` says:
//! it has to be reachable from the other machine. Stopping the agent ends the SSH session, and with
//! it the agent.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_core::agents::{ChildAgent, launch_args, launch_name, launch_text, limit_arguments};
use raven_ci_plugin_api::{AgentLauncher, CiError, CiPlugin, Host, LaunchRequest, LaunchedAgent, Permission};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_process::Spec;

/// The id of the launcher.
pub const LAUNCHER: &str = "agent-remote";

/// The plugin `agent-remote`.
#[derive(Default)]
pub struct AgentRemotePlugin {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

impl std::fmt::Debug for AgentRemotePlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRemotePlugin").finish_non_exhaustive()
    }
}

impl AgentRemotePlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for AgentRemotePlugin {
    fn name(&self) -> &str {
        LAUNCHER
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Process.capability())
            .capability(Permission::Network.capability())
            .capability(Permission::Secrets.capability())
            .capability(Permission::Disk.capability())
    }
}

impl CiPlugin for AgentRemotePlugin {
    fn summary(&self) -> String {
        "Sobe agentes de build em outra máquina por SSH; o agente conecta de volta ao servidor.".to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o ssh que sobe os agentes")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn agent_launchers(&self) -> Vec<Arc<dyn AgentLauncher>> {
        vec![Arc::new(Remote { host: self.host.clone() })]
    }
}

/// `text` as a POSIX shell reads it as one word.
pub fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// `text` as `cmd.exe` and the programs of Windows read it as one word: in double quotes, with a
/// `"` inside doubled (#906). `%VARIAVEL%` is still expanded, as the default folder wants.
pub fn quoted_cmd(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The shell of the other machine, as the template's `sistema` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    /// `sh`, `bash`, `zsh`: Linux and macOS. The default.
    Posix,
    /// `cmd.exe`, where the OpenSSH of Windows runs a command.
    Windows,
}

impl System {
    /// The system of `request`, refused when it names one there is not.
    pub fn of(request: &LaunchRequest) -> Result<Self, CiError> {
        match launch_text(request, "sistema") {
            None | Some("posix") => Ok(Self::Posix),
            Some("windows") => Ok(Self::Windows),
            Some(other) => Err(CiError::settings("sistema", format!("{other} não existe; use posix ou windows"))),
        }
    }

    /// `word` quoted for this shell.
    pub fn quote(self, word: &str) -> String {
        match self {
            Self::Posix => quoted(word),
            Self::Windows => quoted_cmd(word),
        }
    }

    /// The agent's program unless the template says it.
    pub fn program(self) -> &'static str {
        match self {
            Self::Posix => "raven-ci-agent",
            Self::Windows => "raven-ci-agent.exe",
        }
    }

    /// The agent's folder unless the template says it.
    pub fn folder(self, name: &str) -> String {
        match self {
            Self::Posix => format!("raven-ci-agente/{name}"),
            Self::Windows => format!("%USERPROFILE%\\raven-ci-agente\\{name}"),
        }
    }
}

/// The arguments of `ssh` for `request`, with the key in `key` when there is one: its options, the
/// destination and the agent's command line, every word quoted for the other machine's shell.
pub fn ssh_arguments(request: &LaunchRequest, key: Option<&str>) -> Result<Vec<String>, CiError> {
    let host =
        launch_text(request, "host").ok_or_else(|| CiError::settings("host", "o modelo diz a máquina"))?;
    let mut args: Vec<String> =
        ["-T", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new"].map(str::to_owned).to_vec();
    let port = match request.settings.get("porta") {
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(serde_json::Value::String(text)) if !text.is_empty() => Some(text.clone()),
        _ => None,
    };
    if let Some(port) = port {
        args.extend(["-p".to_owned(), port]);
    }
    if let Some(key) = key {
        args.extend(["-i".to_owned(), key.to_owned()]);
    }
    args.push(match launch_text(request, "usuario") {
        Some(user) => format!("{user}@{host}"),
        None => host.to_owned(),
    });
    args.push("--".to_owned());
    let system = System::of(request)?;
    let name = launch_name(request);
    let dir = launch_text(request, "pasta").map_or_else(|| system.folder(&name), str::to_owned);
    let mut words = vec![launch_text(request, "programa").unwrap_or(system.program()).to_owned()];
    words.extend(launch_args(request, &dir).to_args());
    // The token on the input, which `ssh` carries and no variable of the other machine shows.
    words.extend(["--token".to_owned(), "-".to_owned()]);
    words.extend(limit_arguments(request));
    args.push(words.iter().map(|w| system.quote(w)).collect::<Vec<_>>().join(" "));
    Ok(args)
}

/// The launcher `agent-remote`. See the [crate](crate).
struct Remote {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

impl Remote {
    /// The key of the credential `name`, in a file only its owner reads.
    fn key_file(host: &dyn Host, name: &str, agent: &str) -> Result<PathBuf, CiError> {
        let key = host.secret(name)?;
        let file = host.data_dir()?.join(format!("chave-{agent}"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut out = options.open(&file).map_err(|e| CiError::failed(format!("{}: {e}", file.display())))?;
        let text = if key.ends_with('\n') { key } else { format!("{key}\n") };
        std::io::Write::write_all(&mut out, text.as_bytes()).map_err(|e| CiError::failed(e.to_string()))?;
        Ok(file)
    }
}

impl AgentLauncher for Remote {
    fn id(&self) -> &str {
        LAUNCHER
    }

    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, CiError> {
        let host = self
            .host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| CiError::unavailable("o plugin agent-remote está desligado"))?;
        let name = launch_name(request);
        let key = match launch_text(request, "chave") {
            Some(credential) => Some(Self::key_file(host.as_ref(), credential, &name)?),
            None => None,
        };
        let args = ssh_arguments(request, key.as_ref().map(|k| k.display().to_string()).as_deref())?;
        let ssh = launch_text(request, "ssh").unwrap_or("ssh");
        let log = host.clone();
        let agent = ChildAgent::start(&name, Spec::new(ssh).args(args).stdin(), move |line| log.log(line));
        let agent = match agent {
            Ok(agent) => agent,
            Err(e) => {
                if let Some(key) = &key {
                    let _ = std::fs::remove_file(key);
                }
                return Err(e);
            }
        };
        agent.send(&request.token);
        let agent = match key {
            Some(key) => agent.at_stop(move || {
                let _ = std::fs::remove_file(key);
            }),
            None => agent,
        };
        Ok(Box::new(agent))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn request(settings: serde_json::Value) -> LaunchRequest {
        LaunchRequest {
            build: None,
            labels: vec!["mac".into(), "arm".into()],
            settings,
            server: "https://ci.local".into(),
            token: "rva_segredo".into(),
        }
    }

    #[test]
    fn the_command_of_ssh_has_the_machine_the_key_and_the_agent_quoted_with_no_token() {
        let asked = request(json!({
            "name": "mac-1", "executors": 2, "host": "mac-mini.local", "usuario": "ci", "porta": 2222,
            "programa": "/opt/raven ci/raven-ci-agent", "memoria": "8G"
        }));
        let args = ssh_arguments(&asked, Some("/dados/chave-mac-1")).expect("arguments");
        assert_eq!(
            args[..11],
            [
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-p",
                "2222",
                "-i",
                "/dados/chave-mac-1",
                "ci@mac-mini.local",
                "--"
            ]
        );
        assert_eq!(
            args[11],
            "'/opt/raven ci/raven-ci-agent' '--servidor' 'https://ci.local' '--nome' 'mac-1' '--rotulos' \
             'mac,arm' '--executores' '2' '--pasta' 'raven-ci-agente/mac-1' '--token' '-' '--memoria' '8G'"
        );
        assert!(!args.iter().any(|a| a.contains("rva_segredo")), "the token goes on the input");
    }

    #[test]
    fn on_windows_every_word_goes_in_double_quotes_with_the_exe_and_the_profile_folder() {
        let asked = request(json!({ "name": "win-1", "host": "win11", "sistema": "windows" }));
        let args = ssh_arguments(&asked, None).expect("arguments");
        assert_eq!(
            args.last().map(String::as_str),
            Some(
                "\"raven-ci-agent.exe\" \"--servidor\" \"https://ci.local\" \"--nome\" \"win-1\" \"--rotulos\" \
                 \"mac,arm\" \"--executores\" \"1\" \"--pasta\" \"%USERPROFILE%\\raven-ci-agente\\win-1\" \
                 \"--token\" \"-\""
            )
        );
        let spaced = request(json!({ "name": "w", "host": "h", "sistema": "windows", "programa": "C:\\Program Files\\Raven \"CI\"\\a.exe" }));
        let line = ssh_arguments(&spaced, None).expect("arguments");
        assert!(line.last().expect("the line").starts_with("\"C:\\Program Files\\Raven \"\"CI\"\"\\a.exe\""), "{line:?}");
        assert!(ssh_arguments(&request(json!({ "name": "x", "host": "h", "sistema": "dos" })), None).is_err());
    }

    #[test]
    fn a_template_with_no_machine_is_refused_and_quotes_survive() {
        assert!(ssh_arguments(&request(json!({ "name": "x" })), None).is_err());
        assert_eq!(quoted("it's"), "'it'\\''s'");
    }
}
