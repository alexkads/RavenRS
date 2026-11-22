//! The plugin `github` of Raven CI: a repository on GitHub as a job of many branches, and the
//! status of each commit back on it.
//!
//! # Its role
//!
//! The multibranch job of the reference Jenkins builds the integration branch and every pull
//! request of the repository itself, never one of a fork, with the pull request merged onto its
//! target, and puts the status on the commit. This plugin is what knows GitHub in that: the
//! [`BranchSource`] the core's scan asks, and the [`StatusReporter`] the life of a build calls.
//!
//! # How it works
//!
//! - **The model job** says `{"branches": {"id": "github", "repository": "dono/repo",
//!   "credential": "github", "branches": ["main", "release/*"]}}` and its `scm` (`{"id": "git",
//!   "url": "https://github.com/dono/repo.git", "credential": "github"}`). `api` changes the address
//!   of the API (GitHub Enterprise, a test), `forks = true` takes pull requests of forks too, and
//!   `pull_requests = false` leaves pull requests out.
//! - **`discover`** lists the branches (each pattern may end in `*`) and the open pull requests,
//!   one page of 100 after another. A pull request is the head `PR-<number>`, with its target and
//!   whether it comes from a fork (its head is another repository, or one deleted).
//! - **`file`** reads a file of a head at its commit with the API of contents, as raw text: the
//!   pipeline of each branch comes from the branch.
//! - **The status** (`{"status": [{"id": "github", "repository": "dono/repo", "credential":
//!   "github", "context": "raven-ci"}]}` on the model) is `POST /repos/{o}/{r}/statuses/{sha}` with
//!   `pending`, `success`, `failure` or `error`, a line of description and the context.
//! - **The token** is the credential the settings name, read through the host, sent as a bearer;
//!   a credential `user:token` gives its token. With none, the public API is asked.
//! - **What fails** says what GitHub said: the status and its message.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{
    BranchSource, CiError, CiPlugin, CommitState, CommitStatus, Head, HeadKind, Host, Permission,
    StatusReporter,
};
use ravenrs_app::{Module, ModuleManifest};
use serde_json::{Value, json};

/// The API unless the settings say another.
pub const API: &str = "https://api.github.com";
/// How many items a page of the API brings.
const PAGE: usize = 100;
/// How many pages, at most, of branches or pull requests.
const MOST_PAGES: usize = 20;
/// The most a description of a status holds, as GitHub takes it.
const MOST_DESCRIPTION: usize = 140;

type Shared = Arc<Mutex<Option<Arc<dyn Host>>>>;

/// The plugin `github`.
#[derive(Default)]
pub struct GithubPlugin {
    host: Shared,
}

impl std::fmt::Debug for GithubPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubPlugin").finish_non_exhaustive()
    }
}

impl GithubPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for GithubPlugin {
    fn name(&self) -> &str {
        "github"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Network.capability())
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for GithubPlugin {
    fn summary(&self) -> String {
        "GitHub: branches e PRs sem fork num job de muitas branches, o pipeline do repositório e o status no commit."
            .to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Network, "falar com a API do GitHub")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn branch_sources(&self) -> Vec<Arc<dyn BranchSource>> {
        vec![Arc::new(Github { host: self.host.clone() })]
    }

    fn status_reporters(&self) -> Vec<Arc<dyn StatusReporter>> {
        vec![Arc::new(Github { host: self.host.clone() })]
    }
}

/// The branch source and the status reporter `github`.
struct Github {
    host: Shared,
}

fn text<'v>(settings: &'v Value, field: &str) -> Option<&'v str> {
    settings.get(field).and_then(Value::as_str).filter(|t| !t.trim().is_empty())
}

/// What is said to GitHub, and how.
struct Client {
    api: String,
    repository: String,
    token: Option<String>,
    agent: ureq::Agent,
}

impl Github {
    fn client(&self, settings: &Value) -> Result<Client, CiError> {
        let repository = text(settings, "repository")
            .filter(|r| r.split('/').count() == 2)
            .ok_or_else(|| CiError::settings("repository", "diga o repositório como dono/nome"))?
            .to_owned();
        let token = match text(settings, "credential") {
            Some(name) => {
                let host = self
                    .host
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone()
                    .ok_or_else(|| CiError::unavailable("o plugin github não está ligado"))?;
                let value = host.secret(name)?;
                Some(value.rsplit_once(':').map_or(value.clone(), |(_, token)| token.to_owned()))
            }
            None => None,
        };
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        let api = text(settings, "api").unwrap_or(API).trim_end_matches('/').to_owned();
        Ok(Client { api, repository, token, agent })
    }
}

impl Client {
    fn url(&self, path: &str) -> String {
        format!("{}/repos/{}{path}", self.api, self.repository)
    }

    /// `GET` of `path`, as JSON, or `None` when GitHub says there is nothing there.
    fn get(&self, path: &str, raw: bool) -> Result<Option<String>, CiError> {
        let accept = if raw { "application/vnd.github.raw+json" } else { "application/vnd.github+json" };
        let mut request = self
            .agent
            .get(&self.url(path))
            .header("Accept", accept)
            .header("User-Agent", "raven-ci")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let mut answer =
            request.call().map_err(|e| CiError::unavailable(format!("o GitHub não respondeu: {e}")))?;
        let status = answer.status().as_u16();
        let body = answer.body_mut().read_to_string().unwrap_or_default();
        match status {
            200 => Ok(Some(body)),
            404 => Ok(None),
            _ => Err(said(status, &body)),
        }
    }

    /// Every item of the list at `path`, page after page.
    fn list(&self, path: &str) -> Result<Vec<Value>, CiError> {
        let mut all = Vec::new();
        for page in 1..=MOST_PAGES {
            let joiner = if path.contains('?') { '&' } else { '?' };
            let body = self
                .get(&format!("{path}{joiner}per_page={PAGE}&page={page}"), false)?
                .ok_or_else(|| CiError::unavailable(format!("o GitHub não acha {}", self.repository)))?;
            let items: Vec<Value> = serde_json::from_str(&body)
                .map_err(|e| CiError::failed(format!("a resposta do GitHub: {e}")))?;
            let full = items.len() == PAGE;
            all.extend(items);
            if !full {
                break;
            }
        }
        Ok(all)
    }
}

/// What GitHub said when it said no, in words.
fn said(status: u16, body: &str) -> CiError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("message").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| body.chars().take(200).collect());
    CiError::failed(format!("o GitHub respondeu {status}: {message}"))
}

/// Whether `name` is one of `patterns`, where a pattern ending in `*` takes what starts with it.
fn chosen(name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    })
}

fn patterns(settings: &Value) -> Vec<String> {
    match settings.get("branches") {
        Some(Value::String(one)) => vec![one.clone()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        _ => vec!["main".to_owned()],
    }
}

impl BranchSource for Github {
    fn id(&self) -> &str {
        "github"
    }

    fn discover(&self, settings: &Value) -> Result<Vec<Head>, CiError> {
        let client = self.client(settings)?;
        let wanted = patterns(settings);
        let mut heads = Vec::new();
        for branch in client.list("/branches")? {
            let name = branch.get("name").and_then(Value::as_str).unwrap_or_default();
            let commit = branch.pointer("/commit/sha").and_then(Value::as_str).unwrap_or_default();
            if !name.is_empty() && !commit.is_empty() && chosen(name, &wanted) {
                heads.push(Head { name: name.to_owned(), kind: HeadKind::Branch, commit: commit.to_owned() });
            }
        }
        if settings.get("pull_requests").and_then(Value::as_bool) != Some(false) {
            let forks = settings.get("forks").and_then(Value::as_bool) == Some(true);
            for pull in client.list("/pulls?state=open")? {
                let Some(number) = pull.get("number").and_then(Value::as_u64) else { continue };
                let commit = pull.pointer("/head/sha").and_then(Value::as_str).unwrap_or_default();
                let target = pull.pointer("/base/ref").and_then(Value::as_str).unwrap_or_default();
                let from = pull.pointer("/head/repo/full_name").and_then(Value::as_str);
                let from_fork = from.is_none_or(|from| !from.eq_ignore_ascii_case(&client.repository));
                if commit.is_empty() || target.is_empty() || (from_fork && !forks) {
                    continue;
                }
                heads.push(Head {
                    name: format!("PR-{number}"),
                    kind: HeadKind::PullRequest { number, target: target.to_owned(), from_fork },
                    commit: commit.to_owned(),
                });
            }
        }
        Ok(heads)
    }

    fn file(&self, settings: &Value, head: &Head, path: &str) -> Result<Option<String>, CiError> {
        let client = self.client(settings)?;
        let path = path.trim_start_matches('/');
        client.get(&format!("/contents/{path}?ref={}", head.commit), true)
    }
}

impl StatusReporter for Github {
    fn id(&self) -> &str {
        "github"
    }

    fn report(&self, status: &CommitStatus, settings: &Value) -> Result<(), CiError> {
        let mut settings = settings.clone();
        if text(&settings, "repository").is_none()
            && let Value::Object(object) = &mut settings
        {
            object.insert("repository".to_owned(), json!(status.repository));
        }
        let client = self.client(&settings)?;
        let state = match status.state {
            CommitState::Pending => "pending",
            CommitState::Success => "success",
            CommitState::Failure => "failure",
            CommitState::Error => "error",
        };
        let description: String = status.description.chars().take(MOST_DESCRIPTION).collect();
        let mut body = json!({ "state": state, "context": status.context, "description": description });
        if let Some(url) = &status.url {
            body["target_url"] = json!(url);
        }
        let mut request = client
            .agent
            .post(&client.url(&format!("/statuses/{}", status.commit)))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "raven-ci")
            .header("Content-Type", "application/json");
        if let Some(token) = &client.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let mut answer = request
            .send(body.to_string())
            .map_err(|e| CiError::unavailable(format!("o GitHub não respondeu: {e}")))?;
        let code = answer.status().as_u16();
        if (200..300).contains(&code) {
            Ok(())
        } else {
            Err(said(code, &answer.body_mut().read_to_string().unwrap_or_default()))
        }
    }
}
