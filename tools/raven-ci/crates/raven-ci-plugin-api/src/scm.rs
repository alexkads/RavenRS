//! Fetching code and finding what there is to build: the [`Scm`] and [`BranchSource`] points.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CiError;

/// What to fetch from a repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reference {
    /// The tip of a branch.
    Branch(String),
    /// A pull request, by number, with the branch it goes into.
    PullRequest {
        /// Its number at the provider.
        number: u64,
        /// The branch it goes into.
        target: String,
    },
    /// One commit.
    Commit(String),
}

/// What a build asks an [`Scm`] for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutRequest {
    /// The repository, as the provider gives it to clone.
    pub url: String,
    /// What to fetch.
    pub reference: Reference,
    /// The folder of the build, where the code goes.
    pub workspace: PathBuf,
    /// How many commits of history to fetch; all when `None`.
    pub depth: Option<u32>,
    /// The credential to fetch with, by its name, read through the host.
    pub credential: Option<String>,
    /// For a pull request: build it merged onto its target, as it would be once merged.
    pub merge_onto_target: bool,
}

/// What an [`Scm`] fetched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkout {
    /// The commit in the workspace: the merge, when the pull request was merged onto its target.
    pub commit: String,
    /// The branch, when it was one.
    pub branch: Option<String>,
    /// The commit of the target it was merged onto, for a pull request merged.
    pub merged_onto: Option<String>,
}

/// A way of fetching code: git, or another system.
///
/// **Its role.** The core knows no version control. A build that needs code asks the `Scm` its
/// job names (by [`id`](Self::id)) to put it in the workspace.
///
/// **How it works.** The core calls [`checkout`](Self::checkout) on the build's thread, before
/// the first step, with the folder of the build; what it writes to `log` goes to the log of the
/// build, with secrets hidden. A failure ends the build as failed with the error as the reason.
///
/// **How to write one.** Fetch, check out what [`Reference`] says and, for a pull request with
/// `merge_onto_target`, merge it onto its target with no commit of its own; say what is in the
/// workspace in the [`Checkout`]. Read the credential through the host the plugin got in
/// `start`, never from the environment.
pub trait Scm: Send + Sync {
    /// What a job names it by: `git`.
    fn id(&self) -> &str;

    /// Puts what `request` says in its workspace.
    fn checkout(&self, request: &CheckoutRequest, log: &dyn Fn(&str)) -> Result<Checkout, CiError>;
}

/// What a head found by a [`BranchSource`] is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeadKind {
    /// A branch.
    Branch,
    /// A pull request.
    PullRequest {
        /// Its number.
        number: u64,
        /// The branch it goes into.
        target: String,
        /// It comes from someone else's copy of the repository.
        from_fork: bool,
    },
}

/// Something there is to build: a branch or a pull request, at a commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Head {
    /// The name its job takes: `main`, `PR-12`.
    pub name: String,
    /// What it is.
    pub kind: HeadKind,
    /// The commit at its tip.
    pub commit: String,
}

/// A way of finding the branches and pull requests of a repository, for jobs of many branches.
///
/// **Its role.** A job of many branches is a folder with one job for each head. The core asks
/// the source what heads there are and makes, keeps or removes the jobs to match.
///
/// **How it works.** A job whose settings say `{"branches": {"id": "github", ...}}` is the model of
/// a job of many branches. When a trigger asks ([`Builds::scan`](crate::Builds::scan): a schedule,
/// a webhook), the core calls [`discover`](Self::discover) with those settings and keeps one job
/// per head, `<job>/<head>`, with the model's pipeline or the one [`file`](Self::file) reads from
/// the head. A head whose commit changed gets a build; a head that is gone has its job removed
/// after the days the model says.
///
/// **How to write one.** List what the provider has and leave out what the settings filter out
/// (forks, branches not chosen). Return every head that passes, not only the ones that changed:
/// the core compares.
pub trait BranchSource: Send + Sync {
    /// What a job names it by: `github`.
    fn id(&self) -> &str;

    /// The heads there are now, by the job's `settings`.
    fn discover(&self, settings: &Value) -> Result<Vec<Head>, CiError>;

    /// The file at `path` of `head`, at its commit, when there is one: how the pipeline of each
    /// branch comes from the branch itself, as a Jenkinsfile does. `None` by default, and when the
    /// head has no such file; the job's own pipeline is used then.
    fn file(&self, settings: &Value, head: &Head, path: &str) -> Result<Option<String>, CiError> {
        let _ = (settings, head, path);
        Ok(None)
    }
}
