//! When a build is asked for: the [`Trigger`] point, and what it asks the core.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::CiError;

/// A build asked for: of which job, with what, why and how soon.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildRequest {
    /// The job, by its path: `apps/site`.
    pub job: String,
    /// The values of its parameters, by name.
    pub parameters: Map<String, Value>,
    /// Why, in words: "pedido pela API", "push em main".
    pub cause: String,
    /// Who asked, when a person or a key did.
    pub actor: Option<String>,
    /// Builds of a higher priority leave the queue first.
    pub priority: i32,
}

impl BuildRequest {
    /// A build of `job` asked for because of `cause`.
    pub fn new(job: impl Into<String>, cause: impl Into<String>) -> Self {
        Self { job: job.into(), cause: cause.into(), ..Self::default() }
    }

    /// With the parameter `name` at `value`.
    pub fn parameter(mut self, name: impl Into<String>, value: Value) -> Self {
        self.parameters.insert(name.into(), value);
        self
    }

    /// Asked for by `actor`.
    pub fn by(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Leaving the queue before builds of a lower `priority`.
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// What the core gives a [`Trigger`] to ask for builds with.
pub trait Builds: Send + Sync {
    /// Asks for a build; its number. A build of the same job with the same parameters still in
    /// the queue takes the request instead, and its number is given.
    fn request(&self, request: BuildRequest) -> Result<u64, CiError>;
}

/// A way of asking for builds: by hand, by the API, on a schedule, by polling a repository, by a
/// webhook.
///
/// **Its role.** The core never asks for a build by itself: every build comes from a trigger
/// (or from a person, through a route or a command a plugin brings).
///
/// **How it works.** For each job whose pipeline names it, the core calls
/// [`watch`](Self::watch) with the job's settings for it and a [`Builds`] to ask with; when the
/// job changes or is removed, or the plugin is switched off, it calls [`unwatch`](Self::unwatch).
/// A trigger that waits (a schedule) keeps its own thread or timer and asks when the time comes.
///
/// **How to write one.** Check the settings in `watch` and refuse with
/// [`CiError::Settings`] what is wrong, so the job shows it. Keep the `Builds` per job, and drop
/// it in `unwatch`.
pub trait Trigger: Send + Sync {
    /// What a pipeline names it by: `cron`.
    fn id(&self) -> &str;

    /// Starts asking for builds of `job`, by `settings`.
    fn watch(&self, job: &str, settings: &Value, builds: Arc<dyn Builds>) -> Result<(), CiError>;

    /// Stops asking for builds of `job`.
    fn unwatch(&self, job: &str);
}
