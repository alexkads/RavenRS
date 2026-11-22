//! How an agent comes up: the [`AgentLauncher`] point, and [`AgentArgs`], what a launcher gives
//! the `raven-ci-agent` it starts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CiError;

/// What the core asks an [`AgentLauncher`] to bring up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaunchRequest {
    /// The build it is for, as `job#number`, when it is brought up for one build.
    pub build: Option<String>,
    /// The labels the agent has to have.
    pub labels: Vec<String>,
    /// The launcher's settings of the job or of the node: image, resources, machine.
    pub settings: Value,
    /// Where the agent connects to: the server's address.
    pub server: String,
    /// What the agent says to be let in, used once.
    pub token: String,
}

/// An agent a launcher brought up.
pub trait LaunchedAgent: Send + Sync {
    /// What it is called in the list of agents: the container's or the pod's name.
    fn name(&self) -> String;

    /// Whether it is still up.
    fn alive(&self) -> bool;

    /// Takes it down, with whatever it started.
    fn stop(&self);
}

/// A way of bringing agents up: on this machine, on another, in a container, in a pod.
///
/// **Its role.** An agent runs the steps of builds and connects to the server; the core does not
/// know how one starts.
///
/// **How it works.** When a build needs an agent with labels no agent connected has, the core
/// calls [`launch`](Self::launch) on the launcher the labels name. The agent connects out to
/// [`LaunchRequest::server`] with the token, so nothing has to be open where it runs. When the
/// build ends, or the agent passes its time, the core calls [`LaunchedAgent::stop`].
///
/// **How to write one.** Start the agent with the address and the token, and return at once; do
/// not wait for it to connect. `stop` must work even when the start failed halfway.
pub trait AgentLauncher: Send + Sync {
    /// What a job or a node names it by: `docker`.
    fn id(&self) -> &str;

    /// Brings up an agent for `request`.
    fn launch(&self, request: &LaunchRequest) -> Result<Box<dyn LaunchedAgent>, CiError>;
}

/// The variable that carries the agent's token, so it is never on a command line.
pub const TOKEN_VARIABLE: &str = "RAVEN_CI_AGENT_TOKEN";

/// What a launcher gives the `raven-ci-agent` it starts, and what the agent reads back: the
/// contract between the two (#893), in one place.
///
/// **Its role.** A launcher (a container, a pod, another machine) starts the agent with words
/// and an environment; the agent reads them. Both sides use this type, so they never drift
/// apart.
///
/// **How it works.** [`to_args`](Self::to_args) gives the words `--servidor`, `--nome`,
/// `--rotulos`, `--executores` and `--pasta`; [`env`](Self::env) gives the token in
/// [`TOKEN_VARIABLE`]. The token never goes on a command line, where `ps` shows it. Where a
/// launcher has to write the environment down (`docker inspect` shows it to whoever reaches
/// the engine; a pod's spec to whoever reads pods), the token is still of one use and lasts 10
/// minutes, so it is worth nothing once the agent is in. [`from_args`](Self::from_args) reads
/// the words and the variable back, and also takes `--token T` from a person starting an agent
/// by hand.
///
/// **How to use it.**
///
/// ```
/// use raven_ci_plugin_api::{AgentArgs, LaunchRequest};
///
/// let request = LaunchRequest {
///     build: Some("site#1".into()),
///     labels: vec!["build:site#1".into()],
///     settings: serde_json::json!({}),
///     server: "https://ci.casa".into(),
///     token: "tok".into(),
/// };
/// let args = AgentArgs::from_request(&request, "agente-1", 1, "/var/lib/raven-ci-agent");
/// let env: Vec<(String, String)> = args.env();
/// let back = AgentArgs::from_args(&args.to_args(), |name| {
///     env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
/// })?;
/// assert_eq!(back, args);
/// # Ok::<(), raven_ci_plugin_api::CiError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentArgs {
    /// Where the agent connects to.
    pub server: String,
    /// What it is called in the list of agents.
    pub name: String,
    /// What it says to be let in, used once.
    pub token: String,
    /// Its labels.
    pub labels: Vec<String>,
    /// How many builds it runs at once.
    pub executors: u64,
    /// Where it keeps its builds.
    pub folder: String,
}

impl AgentArgs {
    /// The arguments of an agent called `name` for `request`, running `executors` builds at once
    /// in `folder`.
    pub fn from_request(request: &LaunchRequest, name: &str, executors: u64, folder: &str) -> Self {
        Self {
            server: request.server.clone(),
            name: name.to_owned(),
            token: request.token.clone(),
            labels: request.labels.clone(),
            executors: executors.max(1),
            folder: folder.to_owned(),
        }
    }

    /// The words of `raven-ci-agent`, without the token: `--servidor S --nome N [--rotulos a,b]
    /// --executores E --pasta P`.
    pub fn to_args(&self) -> Vec<String> {
        let mut words =
            vec!["--servidor".to_owned(), self.server.clone(), "--nome".to_owned(), self.name.clone()];
        if !self.labels.is_empty() {
            words.extend(["--rotulos".to_owned(), self.labels.join(",")]);
        }
        words.extend([
            "--executores".to_owned(),
            self.executors.to_string(),
            "--pasta".to_owned(),
            self.folder.clone(),
        ]);
        words
    }

    /// The environment the agent gets: the token in [`TOKEN_VARIABLE`].
    pub fn env(&self) -> Vec<(String, String)> {
        vec![(TOKEN_VARIABLE.to_owned(), self.token.clone())]
    }

    /// The arguments read back from `args` and the variables `env` gives: the token from
    /// `--token` or [`TOKEN_VARIABLE`]. What is missing is refused by the name of its argument.
    pub fn from_args(args: &[String], env: impl Fn(&str) -> Option<String>) -> Result<Self, CiError> {
        let mut found: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
        let mut words = args.iter();
        while let Some(word) = words.next() {
            let key = match word.as_str() {
                "--servidor" | "--nome" | "--token" | "--rotulos" | "--executores" | "--pasta" => {
                    word.as_str()
                }
                other => return Err(CiError::settings(other, "o raven-ci-agent não conhece este argumento")),
            };
            let value = words.next().ok_or_else(|| CiError::settings(key, "falta o valor depois dele"))?;
            found.insert(key, value.clone());
        }
        let needed = |key: &str| {
            found
                .get(key)
                .filter(|v| !v.is_empty())
                .cloned()
                .ok_or_else(|| CiError::settings(key, "o raven-ci-agent precisa dele"))
        };
        let token = match found.get("--token").filter(|t| !t.is_empty()) {
            Some(token) => token.clone(),
            None => env(TOKEN_VARIABLE).filter(|t| !t.is_empty()).ok_or_else(|| {
                CiError::settings(
                    "--token",
                    format!("o raven-ci-agent precisa dele, ou da variável {TOKEN_VARIABLE}"),
                )
            })?,
        };
        let executors = match found.get("--executores") {
            Some(n) => {
                n.parse::<u64>().map_err(|_| CiError::settings("--executores", "um número inteiro"))?.max(1)
            }
            None => 1,
        };
        Ok(Self {
            server: needed("--servidor")?,
            name: needed("--nome")?,
            token,
            labels: found
                .get("--rotulos")
                .map(|l| l.split(',').map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect())
                .unwrap_or_default(),
            executors,
            folder: needed("--pasta")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> AgentArgs {
        let request = LaunchRequest {
            build: Some("site#1".into()),
            labels: vec!["build:site#1".into(), "linux".into()],
            settings: Value::Null,
            server: "https://ci.casa".into(),
            token: "tok-uso-unico".into(),
        };
        AgentArgs::from_request(&request, "agente-1", 2, "/var/lib/raven-ci-agent")
    }

    #[test]
    fn the_words_and_the_environment_read_back_to_the_same_arguments() {
        let args = args();
        let words = args.to_args();
        assert!(
            !words.iter().any(|w| w.contains("tok-uso-unico")),
            "the token is not on the command line: {words:?}"
        );
        let env = args.env();
        let back =
            AgentArgs::from_args(&words, |name| env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()))
                .expect("read back");
        assert_eq!(back, args);
    }

    #[test]
    fn a_token_on_the_command_line_is_taken_too_and_what_is_missing_is_named() {
        let mut words = args().to_args();
        words.extend(["--token".to_owned(), "tok-manual".to_owned()]);
        assert_eq!(AgentArgs::from_args(&words, |_| None).expect("read").token, "tok-manual");
        let without = AgentArgs::from_args(&args().to_args(), |_| None).expect_err("no token");
        assert!(
            without.to_string().contains("--token") && without.to_string().contains(TOKEN_VARIABLE),
            "{without}"
        );
        let words: Vec<String> = ["--servidor", "https://x", "--pasta", "/p"].map(str::to_owned).into();
        let missing = AgentArgs::from_args(&words, |_| Some("t".into())).expect_err("no name");
        assert!(missing.to_string().contains("--nome"), "{missing}");
        let odd: Vec<String> = ["--porta", "1"].map(str::to_owned).into();
        assert!(AgentArgs::from_args(&odd, |_| None).expect_err("unknown").to_string().contains("--porta"));
    }
}
