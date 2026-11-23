//! The launcher `docker`: an agent in a container of its own, one per build, taken down at the
//! end of the build or when it passes its time.
//!
//! - **The image** is the agent's own, [`AGENT_IMAGE`] (Fedora with the RPM of Raven CI, `git`
//!   and a user that is not root, made by `pacote/docker/gerar-imagem-agente.sh`), which has the
//!   agent in it. Another image needs `agent_binary`: a `raven-ci-agent` of Linux on the engine's
//!   machine, mounted read only, built for a C library that image has.
//! - **The way to the server**: a server that listens only on its loopback (the RPM's
//!   `127.0.0.1:8090`) is not reached from a bridge network, so on Linux the container then shares
//!   the host's network (`--network host`), and elsewhere (Docker Desktop) it is told
//!   `host.docker.internal` for `127.0.0.1`. A `network` in the settings, or `--network` in `args`,
//!   says otherwise.
//! - **Why it ended**: the container is not run with `--rm`, so when the agent dies its exit code
//!   and its last lines are still there to be read ([`LaunchedAgent::last_words`]); it is removed
//!   when the core takes it down, right after.
//!
//! The agent gets what [`AgentArgs`] says: the words, and the token in `RAVEN_CI_AGENT_TOKEN`,
//! passed by name (`-e RAVEN_CI_AGENT_TOKEN`) with its value in the environment of the `docker`
//! command, never in its words. `docker inspect` still shows it to whoever reaches the engine;
//! it is of one use and lasts 10 minutes, so it is worth nothing once the agent is in.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{AgentArgs, AgentLauncher, CiError, LaunchRequest, LaunchedAgent, TOKEN_VARIABLE};
use ravenrs_process::End;
use serde_json::Value;

use crate::engine::{Engine, slug};
use crate::{State, texts};

/// The image an agent runs in when its settings name none: the agent's own, with the agent, `git`
/// and a user that is not root (`pacote/docker/Dockerfile.agente`).
pub const AGENT_IMAGE: &str = "raven-ci-agente:latest";

/// Where the agent's program is in [`AGENT_IMAGE`], as the RPM installs it.
pub const AGENT_IN_IMAGE: &str = "/usr/bin/raven-ci-agent";

/// Where `agent_binary` is mounted in a container of another image.
pub const AGENT_IN_CONTAINER: &str = "/usr/local/bin/raven-ci-agent";

/// How many of the last lines of a container that ended its last words bring.
const LAST_LINES: &str = "12";

/// The label every container of an agent has, with its name.
pub const AGENT_LABEL: &str = "raven-ci.agent";

/// What a `docker run` of an agent is made of.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentPlan {
    /// The container's name, which is the agent's.
    pub name: String,
    /// The base image.
    pub image: String,
    /// The `raven-ci-agent` of Linux on the engine's machine, mounted read only; `None` for an
    /// image that has it ([`AGENT_IMAGE`]).
    pub binary: Option<String>,
    /// The address of the server the agent is given: the request's, or `host.docker.internal` in
    /// place of the loopback.
    pub server: String,
    /// The network of the container, when it is not docker's own: `host` for a server on the
    /// loopback of a Linux engine.
    pub network: Option<String>,
    /// The `args` of the settings, before the image.
    pub extra: Vec<String>,
    /// What the core asked.
    pub request: LaunchRequest,
    /// How many builds it runs at once.
    pub executors: u64,
}

impl AgentPlan {
    /// The plan of `request`, from its settings: `image`, `agent_binary` (needed for an image
    /// other than [`AGENT_IMAGE`]), `network`, `args` and `executors`.
    pub fn read(request: &LaunchRequest, n: u64) -> Result<Self, CiError> {
        Self::read_on(request, n, cfg!(target_os = "linux"))
    }

    /// The same, saying whether the engine runs on Linux, where the host's network is the
    /// machine's: what decides how a container reaches a server on the loopback.
    pub fn read_on(request: &LaunchRequest, n: u64, linux: bool) -> Result<Self, CiError> {
        let settings = &request.settings;
        let binary = settings
            .get("agent_binary")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .map(str::to_owned);
        let image = settings.get("image").and_then(Value::as_str).unwrap_or(AGENT_IMAGE).to_owned();
        if binary.is_none() && image != AGENT_IMAGE {
            return Err(CiError::settings(
                "agent_binary",
                format!(
                    "a imagem {image} não tem o agente: use a imagem do agente ({AGENT_IMAGE}, feita por pacote/docker/gerar-imagem-agente.sh) ou diga em agent_binary onde está um raven-ci-agent de Linux que rode nela"
                ),
            ));
        }
        let extra: Vec<String> = match settings.get("args") {
            Some(Value::String(text)) => text.split_whitespace().map(str::to_owned).collect(),
            other => texts(other),
        };
        let said_network = settings.get("network").and_then(Value::as_str).map(str::to_owned);
        let in_args = extra.iter().any(|a| a == "--network" || a == "--net" || a.starts_with("--network=") || a.starts_with("--net="));
        let loopback = on_loopback(&request.server);
        let (network, server) = match (said_network, in_args, loopback, linux) {
            (Some(network), _, _, _) => (Some(network), request.server.clone()),
            (None, true, _, _) | (None, false, false, _) => (None, request.server.clone()),
            (None, false, true, true) => (Some("host".to_owned()), request.server.clone()),
            (None, false, true, false) => (None, to_docker_host(&request.server)),
        };
        let executors = settings.get("executors").and_then(Value::as_u64).unwrap_or(1).max(1);
        let whose = request.build.as_deref().map_or_else(|| "livre".to_owned(), slug);
        Ok(Self {
            name: format!("raven-ci-agente-{whose}-{}-{n}", std::process::id()),
            image,
            binary,
            server,
            network,
            extra,
            request: request.clone(),
            executors,
        })
    }

    /// The arguments of `docker run`. The agent connects out to the server: nothing is
    /// published. Not `--rm`: the container stays once it ends, for its last words, until the
    /// core takes it down.
    pub fn args(&self) -> Vec<String> {
        let mut args: Vec<String> = ["run", "-d", "--name", &self.name].map(str::to_owned).into();
        args.extend(["--label".to_owned(), format!("{AGENT_LABEL}={}", self.name)]);
        if let Some(build) = &self.request.build {
            args.extend(["--label".to_owned(), format!("raven-ci.agent-build={build}")]);
        }
        match &self.network {
            Some(network) => args.extend(["--network".to_owned(), network.clone()]),
            // `host.docker.internal` is the engine's machine; docker refuses it with the host's network.
            None => args.extend(["--add-host".to_owned(), "host.docker.internal:host-gateway".to_owned()]),
        }
        let entry = match &self.binary {
            Some(binary) => {
                args.extend(["-v".to_owned(), format!("{binary}:{AGENT_IN_CONTAINER}:ro")]);
                AGENT_IN_CONTAINER
            }
            None => AGENT_IN_IMAGE,
        };
        // The token by name: its value is in the docker command's environment, never in its words.
        args.extend(["-e".to_owned(), TOKEN_VARIABLE.to_owned()]);
        args.extend(self.extra.iter().cloned());
        args.extend(["--entrypoint".to_owned(), entry.to_owned(), self.image.clone()]);
        args.extend(self.agent().to_args());
        args
    }

    /// What the agent gets, as the contract of #893 says, with the address it reaches the
    /// server by.
    pub fn agent(&self) -> AgentArgs {
        let mut request = self.request.clone();
        request.server.clone_from(&self.server);
        AgentArgs::from_request(&request, &self.name, self.executors, "/var/lib/raven-ci-agent")
    }
}

/// Whether `url` is of this machine's loopback: `127.x`, `localhost` or `[::1]`.
fn on_loopback(url: &str) -> bool {
    let host = host_of(url);
    host == "localhost" || host == "[::1]" || host == "::1" || host.starts_with("127.")
}

/// The host of `url`, with no scheme, port nor path.
fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.split(['/', '?']).next().unwrap_or(rest);
    if rest.starts_with('[') {
        return rest.split_inclusive(']').next().unwrap_or(rest);
    }
    rest.rsplit_once(':').map_or(rest, |(host, _)| host)
}

/// `url` with `host.docker.internal` in place of its host: the engine's machine, as a container
/// of Docker Desktop reaches it.
fn to_docker_host(url: &str) -> String {
    let host = host_of(url);
    url.replacen(host, "host.docker.internal", 1)
}

/// The launcher `docker`. See the [crate](crate).
#[derive(Debug)]
pub struct DockerLauncher(pub Arc<State>);

/// An agent in a container.
#[derive(Debug)]
pub struct ContainerAgent {
    name: String,
    engine: Engine,
    stopped: Arc<Mutex<bool>>,
}

impl ContainerAgent {
    fn remove(engine: &Engine, name: &str) {
        let _ = engine.ask(["rm", "-f", "-v", name]);
    }
}

/// What a container that ended says of itself: its exit code and its last lines.
fn last_words_of(engine: &Engine, name: &str) -> Option<String> {
    let code = engine.lines(["inspect", "--format", "{{.State.ExitCode}}", name]).into_iter().next()?;
    let output = engine.command(["logs", "--tail", LAST_LINES, name]).output().ok()?;
    let lines: Vec<String> = output
        .stdout
        .iter()
        .chain(output.stderr.iter())
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    let said = if lines.is_empty() { String::new() } else { format!(": {}", lines.join(" · ")) };
    Some(format!("o contêiner saiu com o código {code}{said}"))
}

impl LaunchedAgent for ContainerAgent {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn alive(&self) -> bool {
        self.engine
            .lines(["inspect", "--format", "{{.State.Running}}", &self.name])
            .first()
            .is_some_and(|state| state == "true")
    }

    fn stop(&self) {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner) = true;
        Self::remove(&self.engine, &self.name);
    }

    fn last_words(&self) -> Option<String> {
        last_words_of(&self.engine, &self.name)
    }
}

impl AgentLauncher for DockerLauncher {
    fn id(&self) -> &str {
        "docker"
    }

    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, CiError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let engine = self.0.engine().map_err(|e| CiError::unavailable(e.message))?;
        let plan = AgentPlan::read(request, NEXT.fetch_add(1, Ordering::SeqCst))?;
        let output = plan
            .agent()
            .env()
            .into_iter()
            .fold(engine.command(plan.args()), |spec, (name, value)| spec.env(name, value))
            .timeout(Duration::from_secs(600))
            .output()
            .map_err(|e| CiError::failed(format!("o {} não rodou: {e}", engine.name())))?;
        if output.end != End::Code(0) {
            // Whatever came up halfway goes.
            ContainerAgent::remove(&engine, &plan.name);
            let why =
                output.stderr.iter().rev().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or_default();
            return Err(CiError::failed(format!("o agente em contêiner não subiu ({}): {why}", plan.image)));
        }
        let agent = ContainerAgent { name: plan.name, engine, stopped: Arc::default() };
        if let Some(limit) = request.settings.get("timeout_s").and_then(Value::as_u64).filter(|s| *s > 0) {
            // Past its time the agent goes, whether or not the core remembered it.
            let (engine, name, stopped) = (agent.engine.clone(), agent.name.clone(), agent.stopped.clone());
            let _ = std::thread::Builder::new().name(format!("prazo-{name}")).spawn(move || {
                std::thread::sleep(Duration::from_secs(limit));
                if !*stopped.lock().unwrap_or_else(PoisonError::into_inner) {
                    ContainerAgent::remove(&engine, &name);
                }
            });
        }
        Ok(Box::new(agent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(settings: Value) -> LaunchRequest {
        LaunchRequest {
            build: Some("apps/site#12".into()),
            labels: vec!["build:apps/site#12".into(), "linux".into()],
            settings,
            server: "http://ci.casa:8080".into(),
            token: "tok-uso-unico".into(),
        }
    }

    #[test]
    fn another_image_without_the_agent_is_refused_saying_what_to_do() {
        let refused = AgentPlan::read(&request(json!({ "image": "node:24-bookworm-slim" })), 0).expect_err("refused");
        let said = refused.to_string();
        assert!(said.contains("agent_binary") && said.contains(AGENT_IMAGE), "{said}");
    }

    /// The image of the agent has it: no binary to mount, its own entry, and no `--rm`, so its last
    /// words are there when it dies (#934).
    #[test]
    fn the_image_of_the_agent_runs_its_own_agent_and_is_not_removed_at_its_end() {
        let plan = AgentPlan::read_on(&request(json!({})), 1, true).expect("a plan");
        let args = plan.args();
        let has = |a: &str, b: &str| args.windows(2).any(|w| w[0] == a && w[1] == b);
        assert!(has("--entrypoint", AGENT_IN_IMAGE), "{args:?}");
        assert!(args.iter().any(|a| a == AGENT_IMAGE), "{args:?}");
        assert!(!args.iter().any(|a| a == "-v"), "nothing is mounted: {args:?}");
        assert!(!args.iter().any(|a| a == "--rm"), "{args:?}");
    }

    /// A server on the loopback is reached by the host's network on Linux and by
    /// `host.docker.internal` elsewhere; one on another address, by docker's own (#934).
    #[test]
    fn a_server_on_the_loopback_is_reached_from_the_container() {
        let mut on_loopback = request(json!({}));
        on_loopback.server = "http://127.0.0.1:8090".into();
        let linux = AgentPlan::read_on(&on_loopback, 0, true).expect("a plan").args();
        let has = |args: &[String], a: &str, b: &str| args.windows(2).any(|w| w[0] == a && w[1] == b);
        assert!(has(&linux, "--network", "host"), "{linux:?}");
        assert!(!linux.iter().any(|a| a == "--add-host"), "docker refuses it with the host's network");
        assert!(has(&linux, "--servidor", "http://127.0.0.1:8090"), "{linux:?}");

        let desktop = AgentPlan::read_on(&on_loopback, 0, false).expect("a plan").args();
        assert!(has(&desktop, "--servidor", "http://host.docker.internal:8090"), "{desktop:?}");
        assert!(!desktop.iter().any(|a| a == "--network"), "{desktop:?}");

        let elsewhere = AgentPlan::read_on(&request(json!({})), 0, true).expect("a plan").args();
        assert!(has(&elsewhere, "--servidor", "http://ci.casa:8080") && !elsewhere.iter().any(|a| a == "--network"));

        let mut said = on_loopback.clone();
        said.settings = json!({ "args": ["--network", "ci"] });
        let kept = AgentPlan::read_on(&said, 0, true).expect("a plan").args();
        assert_eq!(kept.iter().filter(|a| *a == "--network").count(), 1, "{kept:?}");
        assert!(has(&kept, "--network", "ci"), "{kept:?}");
        assert_eq!(host_of("http://[::1]:8090/x"), "[::1]");
        assert!(on_loopback_url("http://localhost:1"));
    }

    fn on_loopback_url(url: &str) -> bool {
        on_loopback(url)
    }

    #[test]
    fn the_agent_runs_its_binary_read_only_with_the_words_of_the_contract() {
        let plan = AgentPlan::read(
            &request(json!({ "agent_binary": "/opt/raven-ci-agent", "image": "debian:stable-slim" })),
            3,
        )
        .expect("a plan");
        assert!(plan.name.starts_with("raven-ci-agente-apps-site-12-"), "{}", plan.name);
        let args = plan.args();
        let has = |a: &str, b: &str| args.windows(2).any(|w| w[0] == a && w[1] == b);
        assert!(has("-v", "/opt/raven-ci-agent:/usr/local/bin/raven-ci-agent:ro"), "{args:?}");
        assert!(has("--label", &format!("raven-ci.agent={}", plan.name)), "{args:?}");
        assert!(has("--entrypoint", AGENT_IN_CONTAINER), "{args:?}");
        assert!(has("--servidor", "http://ci.casa:8080") && has("-e", TOKEN_VARIABLE), "{args:?}");
        assert!(!args.iter().any(|a| a.contains("tok-uso-unico")), "the token is not in the words: {args:?}");
        assert!(has("--rotulos", "build:apps/site#12,linux") && has("--executores", "1"), "{args:?}");
        let image_at = args.iter().position(|a| a == "debian:stable-slim").expect("the image");
        let server_at = args.iter().position(|a| a == "--servidor").expect("the server");
        assert!(image_at < server_at, "the agent's words come after the image");
        assert!(!args.iter().any(|a| a == "-p" || a.starts_with("--publish")), "nothing is published");
    }

    /// The token is for the name the core chose; the container keeps a name of its own (#803,
    /// test of the real world: the agent in a container was refused at the door, 401).
    #[test]
    fn the_agent_gets_in_by_the_name_of_its_token_not_by_the_container_s() {
        let plan = AgentPlan::read(
            &request(json!({ "agent_binary": "/opt/raven-ci-agent", "name": "docker-12-ab12cd" })),
            0,
        )
        .expect("a plan");
        let args = plan.args();
        assert!(args.windows(2).any(|w| w[0] == "--nome" && w[1] == "docker-12-ab12cd"), "{args:?}");
        assert!(args.windows(2).any(|w| w[0] == "--name" && w[1] == plan.name), "{args:?}");
    }
}
