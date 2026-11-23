//! The [`BranchSource`] `git`: the branches of any git server, and the pipeline of each one read
//! from the branch itself, with no API of a provider.
//!
//! # Its role
//!
//! The source `github` reads the pipeline of a branch through GitHub's API, which needs a token
//! for a private repository and leaves out every other server: Azure DevOps, Gitea, a mirror on
//! the machine. This one speaks git itself, so it works wherever `git clone` works, with the same
//! credential the job fetches its code with. It is also how a job of one branch reads its
//! pipeline from the repository (`pipeline_from = "scm"`), as Jenkins's "Pipeline script from
//! SCM" does.
//!
//! # How it works
//!
//! - **`discover`** is `git ls-remote --heads` on `url`, filtered by `branches` as the source
//!   `github` filters ([`wanted_branches`]). With `pull_requests = true`, the `refs/pull/N/head`
//!   the server publishes (GitHub, Gitea, a mirror of them) are heads `PR-N` too; a ref says no
//!   target, so their target is `target`, or the first branch named without `*`.
//! - **`file`** keeps one bare repository per address in the plugin's folder
//!   (`fontes/<hash>.git`), reused from scan to scan, fetches into it only the head
//!   (`--depth 1`) and reads the file with `git show <commit>:<file>`. When the ref no longer
//!   has the commit asked (the branch moved), the commit itself is fetched; when the server
//!   refuses that, the file is read at the tip and the log of the build says which commit it was.
//! - **The credential** is the job's, read through the host, sent as a header in `git`'s
//!   environment, as the [`Scm`](raven_ci_plugin_api::Scm) `git` does: never in an argument,
//!   never in the address kept on disk.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{
    BranchSource, CiError, Head, HeadKind, Host, Permission, branch_chosen, wanted_branches,
};
use ravenrs_git::Git;
use serde_json::Value;

use crate::{IDENTITY, credential, said};

/// The folder, in the plugin's own, where the bare repositories the sources read are kept.
pub const SOURCES_DIR: &str = "fontes";

/// The [`BranchSource`] `git`. See the [module](self).
pub(crate) struct GitBranches {
    pub(crate) host: Arc<Mutex<Option<Arc<dyn Host>>>>,
    /// One fetch into the bare repositories at a time: two scans of the same address would race
    /// on its refs.
    pub(crate) busy: Arc<Mutex<()>>,
}

fn text<'v>(settings: &'v Value, field: &str) -> Option<&'v str> {
    settings.get(field).and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty())
}

/// A name for the bare repository of `url` that a file system takes: FNV-1a, in hexadecimal.
fn folder_name(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}.git")
}

impl GitBranches {
    fn host(&self) -> Result<Arc<dyn Host>, CiError> {
        self.host
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| CiError::unavailable("o plugin git não está ligado"))
    }

    fn url(settings: &Value) -> Result<String, CiError> {
        text(settings, "url").map(str::to_owned).ok_or_else(|| {
            CiError::settings("branches.url", "diga o endereço do repositório (ou ponha-o em scm.url)")
        })
    }

    /// A `git` on `dir`, with the credential the settings name.
    fn git(&self, dir: PathBuf, settings: &Value) -> Result<Git, CiError> {
        let mut git = Git::at(dir).with_identity(IDENTITY.0, IDENTITY.1);
        if let Some(name) = text(settings, "credential") {
            let host = self.host()?;
            host.require(Permission::Network, "ler o repositório com credencial")?;
            git = git.with_credential(credential(&host.secret(name)?));
        }
        Ok(git)
    }

    /// The bare repository of `url`, made when it is not there yet.
    fn bare(&self, url: &str, settings: &Value) -> Result<Git, CiError> {
        let host = self.host()?;
        let dir = host.data_dir()?.join(SOURCES_DIR).join(folder_name(url));
        std::fs::create_dir_all(&dir)
            .map_err(|e| CiError::failed(format!("a pasta {}: {e}", dir.display())))?;
        let git = self.git(dir.clone(), settings)?;
        if !dir.join("HEAD").exists() {
            git.run(&["init", "--bare", "--quiet"]).map_err(said)?;
        }
        Ok(git)
    }
}

impl BranchSource for GitBranches {
    fn id(&self) -> &str {
        "git"
    }

    fn discover(&self, settings: &Value) -> Result<Vec<Head>, CiError> {
        let url = Self::url(settings)?;
        let wanted = wanted_branches(settings);
        // `ls-remote` needs no repository: it runs in the system's temporary folder.
        let git = self.git(std::env::temp_dir(), settings)?;
        let pulls = settings.get("pull_requests").and_then(Value::as_bool) == Some(true);
        let mut patterns = vec!["refs/heads/*"];
        if pulls {
            patterns.push("refs/pull/*/head");
        }
        let refs = git.ls_remote(&url, &patterns).map_err(said)?;
        let target = text(settings, "target")
            .map(str::to_owned)
            .or_else(|| wanted.iter().find(|pattern| !pattern.ends_with('*')).cloned())
            .unwrap_or_else(|| "main".to_owned());
        let mut heads = Vec::new();
        for (commit, name) in refs {
            if let Some(branch) = name.strip_prefix("refs/heads/") {
                if branch_chosen(branch, &wanted) {
                    heads.push(Head { name: branch.to_owned(), kind: HeadKind::Branch, commit });
                }
            } else if let Some(number) = name
                .strip_prefix("refs/pull/")
                .and_then(|rest| rest.strip_suffix("/head"))
                .and_then(|n| n.parse::<u64>().ok())
            {
                heads.push(Head {
                    name: format!("PR-{number}"),
                    kind: HeadKind::PullRequest { number, target: target.clone(), from_fork: false },
                    commit,
                });
            }
        }
        heads.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(heads)
    }

    fn file(&self, settings: &Value, head: &Head, path: &str) -> Result<Option<String>, CiError> {
        let url = Self::url(settings)?;
        let path = path.trim_start_matches('/');
        let _one = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        let git = self.bare(&url, settings)?;
        let source = match &head.kind {
            HeadKind::PullRequest { number, .. } => format!("refs/pull/{number}/head"),
            _ => format!("refs/heads/{}", head.name),
        };
        let local = format!("refs/raven-ci/{}", head.name);
        let by_ref = git
            .run(&["fetch", "--no-tags", "--depth=1", "--", &url, &format!("+{source}:{local}")])
            .map_err(said);
        let has = |rev: &str| git.rev_parse(rev).is_ok();
        // The commit found, when this repository has it or can fetch it; else the tip just fetched.
        let at_commit = !head.commit.is_empty()
            && (has(&head.commit)
                || (git.run(&["fetch", "--no-tags", "--depth=1", "--", &url, &head.commit]).is_ok()
                    && has(&head.commit)));
        let at = if at_commit {
            head.commit.clone()
        } else {
            by_ref?;
            local
        };
        let found = git.run(&["ls-tree", "--name-only", &at, "--", path]).map_err(said)?;
        if found.is_empty() {
            return Ok(None);
        }
        let lines = git.run(&["show", &format!("{at}:{path}")]).map_err(said)?;
        let mut text = lines.join("\n");
        text.push('\n');
        Ok(Some(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_of_an_address_is_the_same_every_time_and_differs_between_addresses() {
        let a = folder_name("https://x/a.git");
        assert_eq!(a, folder_name("https://x/a.git"));
        assert_ne!(a, folder_name("https://x/b.git"));
        assert!(a.ends_with(".git") && a.len() == 20, "{a}");
    }
}
