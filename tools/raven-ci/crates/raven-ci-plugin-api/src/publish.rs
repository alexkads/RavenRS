//! What comes out of a build: the [`Publisher`], [`StatusReporter`] and [`Notifier`] points.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::CiError;

/// Where a build is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BuildStatus {
    /// Waiting in the queue.
    #[default]
    Queued,
    /// Running.
    Running,
    /// It ended and every step passed.
    Succeeded,
    /// A step failed.
    Failed,
    /// Someone cancelled it.
    Cancelled,
    /// It took longer than its limit.
    TimedOut,
    /// The agent that ran it went quiet past its grace time, and the server ended it: no one
    /// cancelled it and no step failed.
    Interrupted,
}

impl BuildStatus {
    /// Whether it ended.
    pub fn ended(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }

    /// In words for people.
    pub fn words(self) -> &'static str {
        match self {
            Self::Queued => "na fila",
            Self::Running => "rodando",
            Self::Succeeded => "passou",
            Self::Failed => "falhou",
            Self::Cancelled => "cancelado",
            Self::TimedOut => "passou do prazo",
            Self::Interrupted => "interrompido",
        }
    }
}

/// A build, as the points that come after it see it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildInfo {
    /// The job, by its path.
    pub job: String,
    /// Its number in the job.
    pub number: u64,
    /// Where it is.
    pub status: BuildStatus,
    /// Its page, when the server has an address.
    pub url: Option<String>,
    /// The values of its parameters.
    pub parameters: Map<String, Value>,
    /// The commit it built, when it fetched code.
    pub commit: Option<String>,
}

/// Something a build leaves: an artifact, a test result, a summary. The core keeps the bytes
/// with the build and forgets them with it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// What kind it is, as the publisher names it: `artifact`, `junit`.
    pub kind: String,
    /// Its name, as a path: `target/report.html`.
    pub name: String,
    /// Its type: `text/html`.
    pub mime: String,
    /// Its contents.
    pub bytes: Vec<u8>,
    /// What a screen shows without reading the bytes: for test results, the counts.
    pub summary: Value,
}

impl Attachment {
    /// The attachment `name` of `kind`, holding `bytes` of the type `mime`.
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        mime: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        Self { kind: kind.into(), name: name.into(), mime: mime.into(), bytes, summary: Value::Null }
    }

    /// With `summary`.
    pub fn summary(mut self, summary: Value) -> Self {
        self.summary = summary;
        self
    }
}

/// A way of taking things from a build when its steps end: artifacts, test results, images.
///
/// **Its role.** What a build leaves is not in the core: each kind of thing comes from a
/// publisher, which says what it found as [`Attachment`]s the core keeps with the build.
///
/// **How it works.** When the steps of a build end, failed or not, the core calls
/// [`publish`](Self::publish) for each publisher the pipeline names, with its settings and the
/// workspace, before the workspace is cleaned. A failure is written to the build's log and does
/// not change its status.
///
/// **How to write one.** Read what the settings point to (a pattern of files, a report) and
/// return it. Put what a screen shows at a glance in [`Attachment::summary`].
pub trait Publisher: Send + Sync {
    /// What a pipeline names it by: `junit`.
    fn id(&self) -> &str;

    /// What it takes from `build`, by `settings`, out of `workspace`.
    fn publish(
        &self,
        build: &BuildInfo,
        settings: &Value,
        workspace: &Path,
    ) -> Result<Vec<Attachment>, CiError>;
}

/// The state of a commit at a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitState {
    /// Building.
    Pending,
    /// It passed.
    Success,
    /// It failed.
    Failure,
    /// It could not be built.
    Error,
}

/// The status of a commit, as a provider shows it on the pull request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitStatus {
    /// The repository at the provider: `owner/name`.
    pub repository: String,
    /// The commit.
    pub commit: String,
    /// What it is about, telling it apart from other checks: `raven-ci/build`.
    pub context: String,
    /// The state.
    pub state: CommitState,
    /// A line for people.
    pub description: String,
    /// The page of the build.
    pub url: Option<String>,
}

/// A way of sending the status of a commit back to its provider.
///
/// **Its role.** The pull request shows whether the build passed. The core knows when a build
/// starts and ends; the reporter knows how the provider wants to hear it.
///
/// **How it works.** The core calls [`report`](Self::report) when a build that fetched a commit
/// starts (pending) and when it ends. A failure goes to the build's log.
///
/// **How to write one.** Send the status with the credential the settings name, read through
/// the host, and keep the context the settings give, so each job has its own check.
pub trait StatusReporter: Send + Sync {
    /// What a job names it by: `github`.
    fn id(&self) -> &str;

    /// Sends `status`, by `settings`.
    fn report(&self, status: &CommitStatus, settings: &Value) -> Result<(), CiError>;
}

/// What happened to a build that people may want to hear about.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BuildEvent {
    /// It started.
    Started(BuildInfo),
    /// It ended, in its status.
    Ended(BuildInfo),
}

/// A way of telling people about builds: e-mail, an outgoing webhook.
///
/// **Its role.** The core says what happened; a notifier says it to someone.
///
/// **How it works.** The core calls [`notify`](Self::notify) for each notifier a job names, with
/// its settings, on events the settings choose. A failure goes to the server's log.
///
/// **How to write one.** Decide from the settings whether the event is wanted (for example,
/// only failures), and send it. Do not wait long: the next build may be waiting.
pub trait Notifier: Send + Sync {
    /// What a job names it by: `email`.
    fn id(&self) -> &str;

    /// Tells about `event`, by `settings`.
    fn notify(&self, event: &BuildEvent, settings: &Value) -> Result<(), CiError>;
}

#[cfg(test)]
mod interrupted {
    use super::BuildStatus;

    #[test]
    fn an_interrupted_build_ended_and_says_so() {
        assert!(BuildStatus::Interrupted.ended());
        assert_eq!(BuildStatus::Interrupted.words(), "interrompido");
        assert_eq!(serde_json::to_value(BuildStatus::Interrupted).expect("JSON"), "interrupted");
    }
}
