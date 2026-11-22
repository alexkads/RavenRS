//! The launcher `docker`: an agent in a container of its own, one per build, taken down at the
//! end of the build or when it passes its time.
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

/// The image an agent runs in when its settings name none.
pub const AGENT_IMAGE: &str = "debian:stable-slim";

/// Where the agent's program is in its container.
pub const AGENT_IN_CONTAINER: &str = "/usr/local/bin/raven-ci-agent";

/// The label every container of an agent has, with its name.
pub const AGENT_LABEL: &str = "raven-ci.agent";

/// What a `docker run` of an agent is made of.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentPlan {
    /// The container's name, which is the agent's.
    pub name: String,
    /// The base image.
    pub image: String,
    /// The `raven-ci-agent` of Linux on the engine's machine, mounted read only.
    pub binary: String,
    /// The `args` of the settings, before the image.
    pub extra: Vec<String>,
    /// What the core asked.
    pub request: LaunchRequest,
    /// How many builds it runs at once.
    pub executors: u64,
}

impl AgentPlan {
    /// The plan of `request`, from its settings: `image`, `agent_binary` (needed), `args` and
    /// `executors`.
    pub fn read(request: &LaunchRequest, n: u64) -> Result<Self, CiError> {
        let settings = &request.settings;
        let binary = settings
            .get("agent_binary")
            .and_then(Value::as_str)
            .filter(|b| !b.trim().is_empty())
            .ok_or_else(|| {
                CiError::settings(
                    "agent_binary",
                    "diga onde está o raven-ci-agent de Linux na máquina do docker (ainda não há imagem com ele)",
                )
            })?
            .to_owned();
        let image = settings.get("image").and_then(Value::as_str).unwrap_or(AGENT_IMAGE).to_owned();
        let extra = match settings.get("args") {
            Some(Value::String(text)) => text.split_whitespace().map(str::to_owned).collect(),
            other => texts(other),
        };
        let executors = settings.get("executors").and_then(Value::as_u64).unwrap_or(1).max(1);
        let whose = request.build.as_deref().map_or_else(|| "livre".to_owned(), slug);
        Ok(Self {
            name: format!("raven-ci-agente-{whose}-{}-{n}", std::process::id()),
            image,
            binary,
            extra,
            request: request.clone(),
            executors,
        })
    }

    /// The arguments of `docker run`. The agent connects out to the server: nothing is
    /// published, and `host.docker.internal` is the engine's machine.
    pub fn args(&self) -> Vec<String> {
        let mut args: Vec<String> = ["run", "-d", "--rm", "--name", &self.name].map(str::to_owned).into();
        args.extend(["--label".to_owned(), format!("{AGENT_LABEL}={}", self.name)]);
        if let Some(build) = &self.request.build {
            args.extend(["--label".to_owned(), format!("raven-ci.agent-build={build}")]);
        }
        args.extend(["--add-host".to_owned(), "host.docker.internal:host-gateway".to_owned()]);
        args.extend(["-v".to_owned(), format!("{}:{AGENT_IN_CONTAINER}:ro", self.binary)]);
        // The token by name: its value is in the docker command's environment, never in its words.
        args.extend(["-e".to_owned(), TOKEN_VARIABLE.to_owned()]);
        args.extend(self.extra.iter().cloned());
        args.extend(["--entrypoint".to_owned(), AGENT_IN_CONTAINER.to_owned(), self.image.clone()]);
        args.extend(self.agent().to_args());
        args
    }

    /// What the agent gets, as the contract of #893 says.
    pub fn agent(&self) -> AgentArgs {
        AgentArgs::from_request(&self.request, &self.name, self.executors, "/var/lib/raven-ci-agent")
    }
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
    fn an_agent_without_its_binary_is_refused_saying_why() {
        let refused = AgentPlan::read(&request(json!({})), 0).expect_err("refused");
        assert!(refused.to_string().contains("agent_binary"), "{refused}");
    }

    #[test]
    fn the_agent_runs_its_binary_read_only_with_the_words_of_the_contract() {
        let plan =
            AgentPlan::read(&request(json!({ "agent_binary": "/opt/raven-ci-agent" })), 3).expect("a plan");
        assert!(plan.name.starts_with("raven-ci-agente-apps-site-12-"), "{}", plan.name);
        let args = plan.args();
        let has = |a: &str, b: &str| args.windows(2).any(|w| w[0] == a && w[1] == b);
        assert!(has("-v", "/opt/raven-ci-agent:/usr/local/bin/raven-ci-agent:ro"), "{args:?}");
        assert!(has("--label", &format!("raven-ci.agent={}", plan.name)), "{args:?}");
        assert!(has("--entrypoint", AGENT_IN_CONTAINER), "{args:?}");
        assert!(has("--servidor", "http://ci.casa:8080") && has("-e", TOKEN_VARIABLE), "{args:?}");
        assert!(!args.iter().any(|a| a.contains("tok-uso-unico")), "the token is not in the words: {args:?}");
        assert!(has("--rotulos", "build:apps/site#12,linux") && has("--executores", "1"), "{args:?}");
        let image_at = args.iter().position(|a| a == AGENT_IMAGE).expect("the default image");
        let server_at = args.iter().position(|a| a == "--servidor").expect("the server");
        assert!(image_at < server_at, "the agent's words come after the image");
        assert!(!args.iter().any(|a| a == "-p" || a.starts_with("--publish")), "nothing is published");
    }
}
