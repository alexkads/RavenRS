//! The plugin `webhook-github` of Raven CI: a push or a pull request on GitHub starts the build
//! at once, through the signed webhook of the repository.
//!
//! # Its role
//!
//! Without it, a job learns of a new commit only when the `poll` or the `scan` of the plugin
//! `cron` looks again. With it, GitHub calls Raven CI the moment something moves, and the
//! periodic look stays as the safety net for a delivery lost.
//!
//! # How it works
//!
//! - **The trigger** `github-webhook` goes on the job: `{"id": "github-webhook", "repository":
//!   "dono/repo", "secret": "github-webhook"}`, where `secret` names a credential of the vault
//!   holding the secret set on the webhook at GitHub. With `branch` (a pattern, or a list of them,
//!   each may end in `*`), the job is of one branch and a push to it asks for a build. Without it,
//!   the job is of many branches (plugin `github`) and every push and pull request asks for a scan,
//!   which makes, keeps and removes its jobs and builds what moved.
//! - **The route** `POST /hooks/github` is public: GitHub does not sign in. Before the
//!   body is read, its `X-Hub-Signature-256` is checked against the secret of each job watched, in
//!   constant time (`ravenrs_app_server::verify`). Only the jobs whose secret signed it, and whose
//!   `repository` is the event's, are touched: a secret of one repository moves nothing of
//!   another.
//! - **The events**: `ping` answers 200. `push` to `refs/heads/*` asks for a build or a scan (a
//!   push that deletes a branch asks for a scan, so its job goes away). `pull_request` that was
//!   `opened`, `synchronize`d, `reopened` or `closed` asks for a scan. A tag, another action or
//!   another event answers 202, ignored, saying why.
//! - **Once per delivery**: the same `X-GitHub-Delivery` sent again answers 200 and does nothing.
//! - **Scans run after the answer**, on a thread of their own: GitHub gives up after 10 seconds,
//!   and a scan asks GitHub for every branch and pull request.
//! - **Refused** is 401 with no detail of which part failed (no signature, a wrong one, a secret
//!   no job uses), and a line in the log of the server.
//!
//! # How to set it up on GitHub
//!
//! 1. `raven-ci credencial criar github-webhook --tipo token`, with a long random secret.
//! 2. On the repository, Settings → Webhooks → Add webhook: Payload URL
//!    `https://<servidor>/hooks/github`, Content type `application/json`, the same secret,
//!    and the events *Pushes* and *Pull requests*.
//! 3. On the job, the trigger `github-webhook` with the repository and the name of the credential.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{
    BuildRequest, Builds, CiError, CiPlugin, Host, HttpRequest, HttpResponse, Permission, Route, Trigger,
};
use ravenrs_app::{Module, ModuleManifest};
use serde_json::{Value, json};

/// The path of the route.
pub const PATH: &str = "/hooks/github";
/// The id of the trigger.
pub const TRIGGER: &str = "github-webhook";
/// The header with `sha256=` and the HMAC of the body.
pub const SIGNATURE_HEADER: &str = "x-hub-signature-256";
/// The header with the event.
pub const EVENT_HEADER: &str = "x-github-event";
/// The header with the id of the delivery.
pub const DELIVERY_HEADER: &str = "x-github-delivery";
/// How many deliveries are remembered, to do each one once.
const REMEMBERED: usize = 256;
/// The actions of a pull request that change what is built.
const PULL_REQUEST_ACTIONS: &[&str] = &["opened", "synchronize", "reopened", "closed"];

/// What a job watched says.
#[derive(Clone)]
struct Watch {
    repository: String,
    secret: String,
    /// The patterns of a job of one branch; empty for a job of many branches.
    branches: Vec<String>,
    builds: Arc<dyn Builds>,
}

#[derive(Default)]
struct State {
    host: Option<Arc<dyn Host>>,
    watched: BTreeMap<String, Watch>,
    deliveries: VecDeque<String>,
}

type Shared = Arc<Mutex<State>>;

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The plugin `webhook-github`.
#[derive(Default)]
pub struct WebhookGithubPlugin {
    shared: Shared,
}

impl std::fmt::Debug for WebhookGithubPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookGithubPlugin").finish_non_exhaustive()
    }
}

impl WebhookGithubPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for WebhookGithubPlugin {
    fn name(&self) -> &str {
        "webhook-github"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Secrets.capability())
    }
}

impl CiPlugin for WebhookGithubPlugin {
    fn summary(&self) -> String {
        "O webhook do GitHub: push e pull request assinados disparam o build ou a varredura na hora."
            .to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        lock(&self.shared).host = Some(host);
        Ok(())
    }

    fn stop(&self) {
        let mut state = lock(&self.shared);
        state.host = None;
        state.watched.clear();
        state.deliveries.clear();
    }

    fn triggers(&self) -> Vec<Arc<dyn Trigger>> {
        vec![Arc::new(GithubWebhook(self.shared.clone()))]
    }

    fn routes(&self) -> Vec<Route> {
        let shared = self.shared.clone();
        vec![Route::new("POST", PATH, move |request| receive(&shared, request)).public()]
    }
}

/// The trigger `github-webhook`.
struct GithubWebhook(Shared);

impl Trigger for GithubWebhook {
    fn id(&self) -> &str {
        TRIGGER
    }

    fn watch(&self, job: &str, settings: &Value, builds: Arc<dyn Builds>) -> Result<(), CiError> {
        let text = |name: &str| {
            settings
                .get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
        };
        let repository = text("repository")
            .ok_or_else(|| CiError::settings("repository", "diga o repositório do GitHub, como dono/repo"))?;
        if repository.split('/').filter(|part| !part.is_empty()).count() != 2 {
            return Err(CiError::settings("repository", format!("{repository:?} não é dono/repo")));
        }
        let secret = text("secret").ok_or_else(|| {
            CiError::settings("secret", "diga o nome da credencial com o segredo do webhook")
        })?;
        let branches = match settings.get("branch") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::String(one)) => vec![one.clone()],
            Some(Value::Array(many)) => {
                let texts: Vec<String> = many.iter().filter_map(Value::as_str).map(str::to_owned).collect();
                if texts.len() != many.len() || texts.is_empty() {
                    return Err(CiError::settings(
                        "branch",
                        "uma lista de textos, como [\"main\", \"release/*\"]",
                    ));
                }
                texts
            }
            Some(_) => return Err(CiError::settings("branch", "um texto ou uma lista de textos")),
        };
        lock(&self.0).watched.insert(job.to_owned(), Watch { repository, secret, branches, builds });
        Ok(())
    }

    fn unwatch(&self, job: &str) {
        lock(&self.0).watched.remove(job);
    }
}

fn answer(status: u16, message: impl Into<String>) -> HttpResponse {
    HttpResponse::json(status, &json!({ "message": message.into() }))
}

fn refused(host: Option<&Arc<dyn Host>>, why: &str) -> HttpResponse {
    if let Some(host) = host {
        host.log(&format!("webhook do GitHub recusado: {why}"));
    }
    HttpResponse::json(401, &json!({ "error": "unauthorized", "message": "A notificação não foi aceita." }))
}

/// Whether `head` is one of `patterns`, where a pattern ending in `*` takes what starts with it.
fn matches_branch(head: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| match pattern.strip_suffix('*') {
        Some(prefix) => head.starts_with(prefix),
        None => head == pattern,
    })
}

/// What an event asks of a job.
enum Ask {
    Build(BuildRequest),
    Scan,
}

/// Answers `POST /hooks/github`.
fn receive(shared: &Shared, request: &HttpRequest) -> HttpResponse {
    let (host, watched) = {
        let state = lock(shared);
        (state.host.clone(), state.watched.clone())
    };
    let Some(signature) = request.headers.get(SIGNATURE_HEADER) else {
        return refused(host.as_ref(), "sem X-Hub-Signature-256");
    };
    // The jobs whose secret signed the body, before the body is read.
    let mut secrets: BTreeMap<String, bool> = BTreeMap::new();
    let mut signed: Vec<(String, Watch)> = Vec::new();
    for (job, watch) in watched {
        let ok = match secrets.get(&watch.secret) {
            Some(ok) => *ok,
            None => {
                let ok =
                    host.as_ref().and_then(|host| host.secret(&watch.secret).ok()).is_some_and(|secret| {
                        ravenrs_app_server::verify(secret.as_bytes(), &request.body, signature)
                    });
                secrets.insert(watch.secret.clone(), ok);
                ok
            }
        };
        if ok {
            signed.push((job, watch));
        }
    }
    if signed.is_empty() {
        return refused(host.as_ref(), "a assinatura não é de nenhum segredo dos jobs vigiados");
    }

    // Signed by a secret Raven CI knows: only now is the body read.
    let event = request.headers.get(EVENT_HEADER).map_or("", String::as_str);
    if event == "ping" {
        return HttpResponse::json(200, &json!({ "message": "pong" }));
    }
    let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
        return answer(400, "o corpo tem que ser JSON (Content type application/json no GitHub)");
    };
    let repository = body.pointer("/repository/full_name").and_then(Value::as_str).unwrap_or_default();
    let jobs: Vec<(String, Watch)> =
        signed.into_iter().filter(|(_, watch)| watch.repository.eq_ignore_ascii_case(repository)).collect();
    if jobs.is_empty() {
        return answer(202, format!("nenhum job com este segredo vigia o repositório {repository}"));
    }
    if let Some(delivery) = request.headers.get(DELIVERY_HEADER) {
        let mut state = lock(shared);
        if state.deliveries.contains(delivery) {
            return HttpResponse::json(
                200,
                &json!({ "message": "esta entrega já foi recebida", "duplicate": true }),
            );
        }
        state.deliveries.push_back(delivery.clone());
        if state.deliveries.len() > REMEMBERED {
            state.deliveries.pop_front();
        }
    }

    let mut asks: Vec<(String, Watch, Ask)> = Vec::new();
    let mut ignored: Vec<String> = Vec::new();
    match event {
        "push" => {
            let reference = body.get("ref").and_then(Value::as_str).unwrap_or_default();
            let Some(branch) = reference.strip_prefix("refs/heads/") else {
                return answer(202, format!("ignorado: {reference} não é uma branch"));
            };
            let deleted = body.get("deleted").and_then(Value::as_bool).unwrap_or(false);
            let commit = body.get("after").and_then(Value::as_str).unwrap_or_default();
            let short: String = commit.chars().take(7).collect();
            let pusher = body
                .pointer("/pusher/name")
                .or_else(|| body.pointer("/sender/login"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            for (job, watch) in jobs {
                if watch.branches.is_empty() {
                    asks.push((job, watch, Ask::Scan));
                } else if deleted {
                    ignored.push(format!("{job}: a branch {branch} foi apagada"));
                } else if matches_branch(branch, &watch.branches) {
                    let mut build = BuildRequest::new(&job, format!("push de {short} em {branch} (GitHub)"));
                    if let Some(pusher) = &pusher {
                        build = build.by(pusher.clone());
                    }
                    asks.push((job, watch, Ask::Build(build)));
                } else {
                    ignored.push(format!("{job}: não vigia a branch {branch}"));
                }
            }
        }
        "pull_request" => {
            let action = body.get("action").and_then(Value::as_str).unwrap_or_default();
            if !PULL_REQUEST_ACTIONS.contains(&action) {
                return answer(202, format!("ignorado: a ação {action} não muda o que se constrói"));
            }
            for (job, watch) in jobs {
                if watch.branches.is_empty() {
                    asks.push((job, watch, Ask::Scan));
                } else {
                    ignored.push(format!(
                        "{job}: é de uma branch, e pull requests são dos jobs de muitas branches"
                    ));
                }
            }
        }
        other => return answer(202, format!("ignorado: o evento {other:?} não dispara builds")),
    }

    let mut builds = Vec::new();
    let mut scans = Vec::new();
    for (job, watch, ask) in asks {
        match ask {
            Ask::Build(request) => match watch.builds.request(request) {
                Ok(number) => builds.push(json!({ "job": job, "number": number })),
                Err(e) => ignored.push(format!("{job}: {e}")),
            },
            Ask::Scan => {
                scans.push(job.clone());
                let log = host.clone();
                let builds = watch.builds.clone();
                let spawned =
                    std::thread::Builder::new().name("webhook-github-scan".to_owned()).spawn(move || {
                        if let Err(why) = builds.scan(&job)
                            && let Some(host) = log
                        {
                            host.log(&format!("a varredura de {job} pedida pelo webhook falhou: {why}"));
                        }
                    });
                if let Err(e) = spawned {
                    ignored.push(format!("a varredura não começou: {e}"));
                }
            }
        }
    }
    let status = if builds.is_empty() && scans.is_empty() {
        202
    } else if scans.is_empty() {
        200
    } else {
        202
    };
    HttpResponse::json(status, &json!({ "builds": builds, "scans": scans, "ignored": ignored }))
}
