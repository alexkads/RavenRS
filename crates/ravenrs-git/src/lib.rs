//! Git for RavenRS services, over the `git` installed on the machine, run with `ravenrs-process`:
//! arguments with no shell, only the environment it needs, a time limit.
//!
//! * [`Git::clone`] with a depth and a branch, and [`Git::at`] for a folder that already is one.
//! * [`Git::fetch`] with refspecs, and [`Git::fetch_pull_request`] for `refs/pull/N/head`.
//! * [`Git::checkout`], [`Git::rev_parse`], [`Git::head`] and [`Git::ls_remote`].
//! * [`Git::merge`], which says [`Merge::Merged`] or names the files of a [`Merge::Conflict`],
//!   and leaves the tree clean when it does not merge.
//! * [`Credential`]: the token travels as `http.extraHeader` in the child's environment
//!   (`GIT_CONFIG_COUNT`), never as an argument, which anyone on the machine can read with `ps`,
//!   nor in the remote's address, which `git` writes to `.git/config`.
//!
//! Every command runs without the user's or the system's git configuration, so a signing key or a
//! hook set up for someone's own work does not change what a build does, and `git` never stops to
//! ask for a password.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ravenrs_process::{End, ProcessError, Spec};

/// How long a command may take unless told otherwise.
const LIMIT: Duration = Duration::from_secs(600);
/// How many lines of what `git` said go into an error.
const ERROR_LINES: usize = 12;

/// Why a command did not do what was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GitError {
    /// `git` is not installed, or not where it was said to be.
    NotInstalled(String),
    /// It ran and failed: the command, its code, and the last lines it wrote to its errors.
    Failed {
        /// What was asked, as `fetch origin`.
        command: String,
        /// Its exit code, when it had one.
        code: Option<i32>,
        /// The last lines of what it said.
        error: String,
    },
    /// It passed its time limit and was stopped.
    TimedOut(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled(program) => write!(f, "{program} is not installed, or not in PATH"),
            Self::Failed { command, code: Some(code), error } => {
                write!(f, "git {command} failed ({code}): {error}")
            }
            Self::Failed { command, code: None, error } => write!(f, "git {command} failed: {error}"),
            Self::TimedOut(command) => write!(f, "git {command} did not finish in time"),
        }
    }
}

impl std::error::Error for GitError {}

/// What a merge did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Merge {
    /// It merged, into this commit.
    Merged(String),
    /// It did not: these files conflict. The merge was undone, so the tree is as before.
    Conflict(Vec<String>),
}

/// How to clone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CloneOptions {
    /// How many commits back; `None` brings the whole history.
    pub depth: Option<u32>,
    /// The branch to check out; `None` is the remote's own.
    pub branch: Option<String>,
}

impl CloneOptions {
    /// The whole history, on the remote's branch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Only the last `depth` commits.
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = Some(depth.max(1));
        self
    }

    /// On `branch`.
    pub fn branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }
}

/// A name and a secret for an HTTPS remote. It shows as `Credential(•••)`.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    header: String,
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credential(•••)")
    }
}

impl Credential {
    /// HTTP basic, as GitHub and Azure DevOps take a token: a user name (any, for a token) and it.
    pub fn basic(user: &str, token: &str) -> Self {
        Self { header: format!("Authorization: Basic {}", base64(format!("{user}:{token}").as_bytes())) }
    }

    /// A bearer token.
    pub fn bearer(token: &str) -> Self {
        Self { header: format!("Authorization: Bearer {token}") }
    }
}

/// Standard base64, with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let [a, b, c] = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c);
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A repository on disk, and how to run `git` in it.
#[derive(Clone, Debug)]
pub struct Git {
    program: OsString,
    dir: PathBuf,
    credential: Option<Credential>,
    identity: (String, String),
    limit: Duration,
}

fn failed(command: &str, error: impl fmt::Display) -> GitError {
    GitError::Failed { command: command.to_owned(), code: None, error: error.to_string() }
}

/// Where `git` finds no configuration: a file that is not there reads as empty.
fn nowhere() -> PathBuf {
    std::env::temp_dir().join("ravenrs-git-no-config")
}

impl Git {
    /// The repository in `dir`, run with the `git` in `PATH`.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self {
            program: OsString::from("git"),
            dir: dir.into(),
            credential: None,
            identity: ("RavenRS".to_owned(), "ravenrs@localhost".to_owned()),
            limit: LIMIT,
        }
    }

    /// Clones `url` into `dir`, which must not exist or be empty, and returns it.
    pub fn clone(url: &str, dir: impl Into<PathBuf>, options: &CloneOptions) -> Result<Self, GitError> {
        Self::at(dir).clone_with(url, options)
    }

    /// The same, with what this one was told (program, credential, identity, limit).
    pub fn clone_with(self, url: &str, options: &CloneOptions) -> Result<Self, GitError> {
        let mut args: Vec<OsString> = vec!["clone".into(), "--no-tags".into()];
        if let Some(depth) = options.depth {
            args.push(format!("--depth={depth}").into());
        }
        if let Some(branch) = &options.branch {
            args.push("--branch".into());
            args.push(branch.into());
        }
        // Absolute, since git runs in the folder above it.
        let target = if self.dir.is_absolute() {
            self.dir.clone()
        } else {
            std::env::current_dir().map_err(|e| failed("clone", e))?.join(&self.dir)
        };
        let parent = target.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        std::fs::create_dir_all(&parent).map_err(|e| failed("clone", e))?;
        args.push("--".into());
        args.push(url.into());
        args.push(target.as_os_str().to_owned());
        self.run_in(&parent, "clone", args)?;
        Ok(Self { dir: target, ..self })
    }

    /// Runs this `git` instead of the one in `PATH`.
    pub fn with_program(mut self, program: impl Into<OsString>) -> Self {
        self.program = program.into();
        self
    }

    /// Talks to HTTPS remotes with `credential`.
    pub fn with_credential(mut self, credential: Credential) -> Self {
        self.credential = Some(credential);
        self
    }

    /// Who merges, as git records it; `RavenRS <ravenrs@localhost>` unless told.
    pub fn with_identity(mut self, name: impl Into<String>, email: impl Into<String>) -> Self {
        self.identity = (name.into(), email.into());
        self
    }

    /// How long one command may take; 10 minutes unless told.
    pub fn with_limit(mut self, limit: Duration) -> Self {
        self.limit = limit;
        self
    }

    /// The folder of the repository.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn spec(&self, dir: &Path, args: Vec<OsString>) -> Spec {
        let none = nowhere();
        let mut spec = Spec::new(self.program.clone())
            .args(args)
            .dir(dir)
            .timeout(self.limit)
            .allow_env("SSH_AUTH_SOCK")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", none.to_string_lossy().into_owned())
            .env("GIT_AUTHOR_NAME", self.identity.0.clone())
            .env("GIT_AUTHOR_EMAIL", self.identity.1.clone())
            .env("GIT_COMMITTER_NAME", self.identity.0.clone())
            .env("GIT_COMMITTER_EMAIL", self.identity.1.clone());
        // The credential goes in the environment, which only the same user can read, and only for
        // this one command.
        if let Some(credential) = &self.credential {
            spec = spec
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "http.extraHeader")
                .env("GIT_CONFIG_VALUE_0", credential.header.clone());
        }
        spec
    }

    fn run_in(&self, dir: &Path, command: &str, args: Vec<OsString>) -> Result<Vec<String>, GitError> {
        let program = self.program.to_string_lossy().into_owned();
        let output = self.spec(dir, args).output().map_err(|error| match error {
            ProcessError::NotFound(_) => GitError::NotInstalled(program.clone()),
            ProcessError::TimedOut => GitError::TimedOut(command.to_owned()),
            ProcessError::Start(why) => {
                GitError::Failed { command: command.to_owned(), code: None, error: why }
            }
            other => GitError::Failed { command: command.to_owned(), code: None, error: other.to_string() },
        })?;
        match output.end {
            End::Code(0) => Ok(output.stdout),
            End::TimedOut => Err(GitError::TimedOut(command.to_owned())),
            end => {
                let lines = output.stderr.len().saturating_sub(ERROR_LINES);
                let error = output.stderr[lines..].join("\n");
                let code = match end {
                    End::Code(code) => Some(code),
                    _ => None,
                };
                Err(GitError::Failed { command: command.to_owned(), code, error })
            }
        }
    }

    /// Runs `git args…` in the repository and returns what it wrote, line by line.
    pub fn run(&self, args: &[&str]) -> Result<Vec<String>, GitError> {
        let command = args.first().copied().unwrap_or_default().to_owned();
        self.run_in(&self.dir, &command, args.iter().map(OsString::from).collect())
    }

    /// Fetches `refspecs` from `remote`.
    pub fn fetch(&self, remote: &str, refspecs: &[&str]) -> Result<(), GitError> {
        let mut args = vec!["fetch", "--no-tags", "--", remote];
        args.extend_from_slice(refspecs);
        self.run(&args).map(|_| ())
    }

    /// The refspec of pull request `number` on GitHub (and Azure DevOps, which keeps the same
    /// `refs/pull/N/head`), into `refs/remotes/<remote>/pr/N`.
    pub fn pull_request_ref(remote: &str, number: u64) -> String {
        format!("+refs/pull/{number}/head:refs/remotes/{remote}/pr/{number}")
    }

    /// Fetches pull request `number` from `remote`, as `<remote>/pr/N`.
    pub fn fetch_pull_request(&self, remote: &str, number: u64) -> Result<(), GitError> {
        self.fetch(remote, &[&Self::pull_request_ref(remote, number)])
    }

    /// Checks out `rev` with no branch, so nothing a build does moves a branch.
    pub fn checkout(&self, rev: &str) -> Result<(), GitError> {
        self.run(&["checkout", "--detach", "--force", rev]).map(|_| ())
    }

    /// The commit `rev` names.
    pub fn rev_parse(&self, rev: &str) -> Result<String, GitError> {
        let lines = self.run(&["rev-parse", "--verify", "--end-of-options", &format!("{rev}^{{commit}}")])?;
        Ok(lines.into_iter().next().unwrap_or_default().trim().to_owned())
    }

    /// The commit checked out.
    pub fn head(&self) -> Result<String, GitError> {
        self.rev_parse("HEAD")
    }

    /// The refs of `remote` that match `patterns` (all of them with none), as (commit, ref).
    pub fn ls_remote(&self, remote: &str, patterns: &[&str]) -> Result<Vec<(String, String)>, GitError> {
        let mut args = vec!["ls-remote", "--", remote];
        args.extend_from_slice(patterns);
        let lines = self.run(&args)?;
        Ok(lines
            .iter()
            .filter_map(|line| line.split_once('\t'))
            .map(|(commit, name)| (commit.trim().to_owned(), name.trim().to_owned()))
            .collect())
    }

    /// Merges `rev` into what is checked out, always as a merge commit with `message`. On a
    /// conflict it names the files and undoes the merge.
    pub fn merge(&self, rev: &str, message: &str) -> Result<Merge, GitError> {
        match self.run(&["merge", "--no-ff", "--no-edit", "-m", message, "--", rev]) {
            Ok(_) => Ok(Merge::Merged(self.head()?)),
            Err(failed @ GitError::Failed { .. }) => {
                let files = self.run(&["diff", "--name-only", "--diff-filter=U"])?;
                if files.is_empty() {
                    return Err(failed);
                }
                self.run(&["merge", "--abort"])?;
                Ok(Merge::Conflict(files))
            }
            Err(other) => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_the_standard_one() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"x-access-token:ghp_abc"), "eC1hY2Nlc3MtdG9rZW46Z2hwX2FiYw==");
    }

    #[test]
    fn a_credential_never_shows() {
        let credential = Credential::basic("x", "ghp_muito_secreto");
        assert_eq!(format!("{credential:?}"), "Credential(•••)");
        assert!(!format!("{:?}", Git::at(".").with_credential(credential)).contains("ghp_muito_secreto"));
    }
}
