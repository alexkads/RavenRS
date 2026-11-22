//! The plugin `git` of Raven CI: the code of a build, fetched before its first step.
//!
//! # Its role
//!
//! A build tests what is in a repository: the tip of a branch, one commit, or a pull request as it
//! would be once merged. The core knows no version control; it asks the [`Scm`] the job names in
//! its settings (`settings.scm.id = "git"`) to put the code in the build's folder, before the
//! first step, and gives the commit to the steps as `GIT_COMMIT`.
//!
//! # How it works
//!
//! Everything goes through `ravenrs-git`, which runs the `git` of the machine with no shell, none
//! of the user's git configuration and a time limit:
//!
//! - **A branch**: `clone --branch B` (with `--depth D` when the job asks), in the build's folder.
//! - **A commit**: a clone, then `checkout --detach` of the commit; when a shallow history does not
//!   have it, the commit is fetched and checked out again.
//! - **A pull request**: a clone of its target, then `refs/pull/N/head` fetched apart from the branches
//!   (`Git::pull_request_head`), so a branch named `pr` does not stop it.
//!   With `merge_onto_target`, it is merged onto the target with a merge commit of Raven CI's own,
//!   and the [`Checkout`] says the commit of the target in `merged_onto`; the target's history is
//!   then fetched whole, since a merge needs the commit both sides come from. A conflict fails the
//!   build naming the files. Without it, the pull request's own commit is checked out.
//! - **The credential** is read through the host by its name. A value `user:token` is used as it
//!   is, and a value with no `:` is taken as a token. It travels as a header in `git`'s
//!   environment, never as an argument nor in the remote's address, and the log hides it.
//! - **The log** of the build says each thing done: cloning, fetching the pull request, merging.
//!
//! # How to use it
//!
//! ```json
//! { "scm": { "id": "git", "url": "https://github.com/dono/repo.git",
//!            "reference": { "pull_request": { "number": 12, "target": "main" } },
//!            "merge_onto_target": true, "depth": 50, "credential": "github-token" } }
//! ```

use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{Checkout, CheckoutRequest, CiError, CiPlugin, Host, Permission, Reference, Scm};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_git::{CloneOptions, Credential, Git, GitError, Merge};

/// The name and e-mail of the merge commits Raven CI makes.
pub const IDENTITY: (&str, &str) = ("Raven CI", "raven-ci@localhost");

/// The plugin `git`.
#[derive(Default)]
pub struct GitPlugin {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

impl std::fmt::Debug for GitPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitPlugin").finish_non_exhaustive()
    }
}

impl GitPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for GitPlugin {
    fn name(&self) -> &str {
        "git"
    }

    fn manifest(&self) -> ModuleManifest {
        let mut manifest = ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default();
        for permission in [Permission::Process, Permission::Network, Permission::Secrets] {
            manifest = manifest.capability(permission.capability());
        }
        manifest
    }
}

impl CiPlugin for GitPlugin {
    fn summary(&self) -> String {
        "Busca o código do build: uma branch, um commit ou o PR mesclado no destino.".to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o git")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn scms(&self) -> Vec<Arc<dyn Scm>> {
        vec![Arc::new(GitScm { host: self.host.clone() })]
    }
}

/// The [`Scm`] `git`. See the [crate](crate).
struct GitScm {
    host: Arc<Mutex<Option<Arc<dyn Host>>>>,
}

/// A failure of `git`, in words for the build.
fn said(e: GitError) -> CiError {
    CiError::failed(e.to_string())
}

impl GitScm {
    fn host(&self) -> Result<Arc<dyn Host>, CiError> {
        self.host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| CiError::unavailable("o plugin git não está ligado"))
    }

    /// The credential `name`, read through the host.
    fn credential(&self, name: &str) -> Result<Credential, CiError> {
        let host = self.host()?;
        host.require(Permission::Network, "buscar código com credencial")?;
        Ok(credential(&host.secret(name)?))
    }

    /// A `git` on the build's folder, with the credential the request names.
    fn git(&self, request: &CheckoutRequest) -> Result<Git, CiError> {
        let mut git = Git::at(&request.workspace).with_identity(IDENTITY.0, IDENTITY.1);
        if let Some(name) = &request.credential {
            git = git.with_credential(self.credential(name)?);
        }
        Ok(git)
    }
}

impl Scm for GitScm {
    fn id(&self) -> &str {
        "git"
    }

    fn checkout(&self, request: &CheckoutRequest, log: &dyn Fn(&str)) -> Result<Checkout, CiError> {
        let git = self.git(request)?;
        let source = match &request.reference {
            Reference::PullRequest { number, .. } => format!("refs/pull/{number}/head"),
            _ => String::new(),
        };
        checkout_with(git, request, &source, log)
    }
}

/// The credential of git for a secret `value` read through the host: `user:token` as it is, and a
/// value with no `:` as a token (the user is `raven-ci`, which GitHub and Azure DevOps ignore).
pub fn credential(value: &str) -> Credential {
    match value.split_once(':') {
        Some((user, token)) => Credential::basic(user, token),
        None => Credential::basic("raven-ci", value),
    }
}

/// Puts what `request` says in its workspace with `git`, a [`Git`] on the workspace with the
/// credential and identity already set: what the `Scm` `git` does, for another `Scm` that only
/// differs in where a pull request comes from (`raven-ci-plugin-azure-devops`).
///
/// `pull_source` is the ref of the remote the pull request is fetched from: `refs/pull/N/head` on
/// GitHub, the source branch (`refs/heads/feature`) on Azure DevOps, which publishes only
/// `refs/pull/N/merge`. It is fetched to [`Git::pull_request_head`], apart from the branches.
/// Unused for a branch or a commit.
pub fn checkout_with(
    git: Git,
    request: &CheckoutRequest,
    pull_source: &str,
    log: &dyn Fn(&str),
) -> Result<Checkout, CiError> {
    let depth_words = request.depth.map_or_else(String::new, |d| format!(", últimos {d} commits"));
    match &request.reference {
        Reference::Branch(branch) => {
            log(&format!("clonando {} na branch {branch}{depth_words}", request.url));
            let mut options = CloneOptions::new().branch(branch.clone());
            if let Some(depth) = request.depth {
                options = options.depth(depth);
            }
            let git = git.clone_with(&request.url, &options).map_err(said)?;
            let commit = git.head().map_err(said)?;
            Ok(Checkout { commit, branch: Some(branch.clone()), merged_onto: None })
        }
        Reference::Commit(commit) => {
            log(&format!("clonando {} no commit {commit}{depth_words}", request.url));
            let mut options = CloneOptions::new();
            if let Some(depth) = request.depth {
                options = options.depth(depth);
            }
            let git = git.clone_with(&request.url, &options).map_err(said)?;
            if git.checkout(commit).is_err() {
                log(&format!("o commit {commit} não veio no clone; buscando"));
                git.fetch("origin", &[commit]).map_err(said)?;
                git.checkout(commit).map_err(said)?;
            }
            let commit = git.head().map_err(said)?;
            Ok(Checkout { commit, branch: None, merged_onto: None })
        }
        Reference::PullRequest { number, target } => {
            let mut options = CloneOptions::new().branch(target.clone());
            // A merge needs the commit both sides come from: the target's history comes whole.
            if let (Some(depth), false) = (request.depth, request.merge_onto_target) {
                options = options.depth(depth);
            }
            log(&format!("clonando {} na branch {target}, destino do PR {number}", request.url));
            let git = git.clone_with(&request.url, &options).map_err(said)?;
            log(&format!("buscando o PR {number}"));
            let refspec = format!("+{pull_source}:{}", Git::pull_request_head("origin", *number));
            git.fetch("origin", &[&refspec]).map_err(said)?;
            let head = Git::pull_request_head("origin", *number);
            if request.merge_onto_target {
                let onto = git.head().map_err(said)?;
                log(&format!("mesclando o PR {number} em {target} ({onto})"));
                match git
                    .merge(&head, &format!("Merge do PR {number} em {target} (Raven CI)"))
                    .map_err(said)?
                {
                    Merge::Merged(commit) => {
                        Ok(Checkout { commit, branch: None, merged_onto: Some(onto) })
                    }
                    Merge::Conflict(files) => Err(CiError::failed(format!(
                        "o PR {number} conflita com {target} em: {}",
                        files.join(", ")
                    ))),
                }
            } else {
                git.checkout(&head).map_err(said)?;
                let commit = git.head().map_err(said)?;
                Ok(Checkout { commit, branch: None, merged_onto: None })
            }
        }
        other => Err(CiError::failed(format!("o git não sabe buscar {other:?}"))),
    }
}
