//! How an agent comes up: the [`AgentLauncher`] point.

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
