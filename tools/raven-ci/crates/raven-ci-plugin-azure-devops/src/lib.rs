//! The plugin `azure-devops` of Raven CI: the code of a repository of Azure DevOps fetched with a
//! PAT, and the status of the build back on its pull request.
//!
//! # Its role
//!
//! The second reference pipeline of #803 is a parameterized pipeline of an Azure DevOps
//! repository: it builds a branch or a pull request and the pull request shows whether it passed.
//! This plugin is what knows Azure DevOps in that: an [`Scm`] that fetches with the PAT kept in
//! the vault, and a [`StatusReporter`] that speaks the REST API of Azure DevOps.
//!
//! # How it works
//!
//! - **The address** is the one to clone: `https://dev.azure.com/org/projeto/_git/repo` (or
//!   `https://org.visualstudio.com/projeto/_git/repo`). What comes before `/<projeto>/_git/` is the
//!   collection, and the API is `<coleção>/<projeto>/_apis/git/repositories/<repo>`. The setting
//!   `api` of the plugin (or of a status) puts another collection in its place: Azure DevOps
//!   Server, or a test.
//! - **The PAT** is the credential the job names, read through the host. It goes as the password
//!   of HTTP basic, to git as a header in its environment (never in the address nor the command
//!   line) and to the API as the same header ([`ravenrs_git::Credential::http_authorization`]).
//!   A value `user:token` is used as it is.
//! - **The checkout** is the one of the plugin `git` ([`raven_ci_plugin_git::checkout_with`]): a
//!   branch, a commit, or a pull request merged onto its target. The only difference is where a
//!   pull request comes from: Azure DevOps publishes `refs/pull/N/merge` but no `refs/pull/N/head`,
//!   so the plugin asks the API for the pull request (`GET …/pullRequests/N`) and fetches its source
//!   branch. The commit built is remembered as the pull request's.
//! - **The status** (`{"status": [{"id": "azure-devops", "repository": "<endereço>",
//!   "credential": "azure", "genre": "raven-ci"}]}` on the job) goes to the pull request
//!   (`POST …/pullRequests/N/statuses`) when the commit is of a pull request this plugin fetched,
//!   or the entry says `pull_request = N`; else to the commit (`POST …/commits/<sha>/statuses`).
//!   The states are `pending`, `succeeded`, `failed` and `error`, with the context of the job as
//!   its name and `genre` as its genre: what a status policy of a branch asks for.
//! - **What fails** says what Azure DevOps said. A PAT it does not take is answered with a page
//!   to sign in (203) or 401; both say to look at the PAT.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{
    Checkout, CheckoutRequest, CiError, CiPlugin, CommitState, CommitStatus, Host, Permission, Reference, Scm,
    StatusReporter,
};
use raven_ci_plugin_git::{IDENTITY, checkout_with, credential};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use ravenrs_git::Git;
use serde_json::{Value, json};

/// The version of the REST API asked for.
pub const API_VERSION: &str = "7.1";
/// The genre of a status unless the settings say another.
pub const GENRE: &str = "raven-ci";
/// How many commits of pull requests are remembered for their statuses.
const REMEMBERED: usize = 256;

/// What the plugin keeps while it is on.
#[derive(Default)]
struct State {
    host: Option<Arc<dyn Host>>,
    /// The commits built of pull requests: `(commit, number)`, the newest last.
    pulls: VecDeque<(String, u64)>,
}

type Shared = Arc<Mutex<State>>;

fn lock(state: &Shared) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The plugin `azure-devops`.
#[derive(Default)]
pub struct AzureDevopsPlugin {
    state: Shared,
}

impl std::fmt::Debug for AzureDevopsPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureDevopsPlugin").finish_non_exhaustive()
    }
}

impl AzureDevopsPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for AzureDevopsPlugin {
    fn name(&self) -> &str {
        "azure-devops"
    }

    fn manifest(&self) -> ModuleManifest {
        let mut manifest = ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default();
        for permission in [Permission::Process, Permission::Network, Permission::Secrets] {
            manifest = manifest.capability(permission.capability());
        }
        manifest
    }
}

impl CiPlugin for AzureDevopsPlugin {
    fn summary(&self) -> String {
        "Azure DevOps: busca o código com o PAT, o PR mesclado no destino, e manda o status ao PR.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(EntityInfo::new("AzureDevopsSettings").field(FieldInfo::new("api", FieldKind::Text)))
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o git")?;
        host.require(Permission::Network, "falar com o Azure DevOps")?;
        lock(&self.state).host = Some(host);
        Ok(())
    }

    fn stop(&self) {
        let mut state = lock(&self.state);
        state.host = None;
        state.pulls.clear();
    }

    fn scms(&self) -> Vec<Arc<dyn Scm>> {
        vec![Arc::new(Azure { state: self.state.clone() })]
    }

    fn status_reporters(&self) -> Vec<Arc<dyn StatusReporter>> {
        vec![Arc::new(Azure { state: self.state.clone() })]
    }
}

/// A repository of Azure DevOps, read from the address to clone it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// The collection: `https://dev.azure.com/org`.
    pub collection: String,
    /// The project, as in the address (encoded).
    pub project: String,
    /// The repository, as in the address (encoded).
    pub name: String,
}

impl Repository {
    /// The repository of `url`, an address `…/<projeto>/_git/<repo>`. A user in the address
    /// (`https://org@dev.azure.com/…`, as Azure DevOps shows it) is left out.
    pub fn parse(url: &str) -> Result<Self, CiError> {
        let wrong = || {
            CiError::settings("url", format!("{url} não é um endereço do Azure DevOps (…/projeto/_git/repo)"))
        };
        let url = match url.split_once("://") {
            Some((scheme, rest)) => {
                let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
                let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
                format!("{scheme}://{authority}/{path}")
            }
            None => url.to_owned(),
        };
        let (before, after) = url.split_once("/_git/").ok_or_else(wrong)?;
        let name = after.split(['?', '#', '/']).next().unwrap_or_default();
        let (collection, project) = before.rsplit_once('/').ok_or_else(wrong)?;
        if name.is_empty() || project.is_empty() || collection.is_empty() {
            return Err(wrong());
        }
        Ok(Self { collection: collection.to_owned(), project: project.to_owned(), name: name.to_owned() })
    }

    /// The address of the API of the repository, with `api` in place of the collection when given.
    pub fn api(&self, api: Option<&str>) -> String {
        let collection = api.map_or(self.collection.as_str(), |a| a.trim_end_matches('/'));
        format!("{collection}/{}/_apis/git/repositories/{}", self.project, self.name)
    }
}

/// The [`Scm`] and the [`StatusReporter`] `azure-devops`. See the [crate](crate).
struct Azure {
    state: Shared,
}

fn text<'v>(settings: &'v Value, field: &str) -> Option<&'v str> {
    settings.get(field).and_then(Value::as_str).filter(|t| !t.trim().is_empty())
}

/// What is said to the API of one repository, and how.
struct Client {
    base: String,
    authorization: Option<String>,
    agent: ureq::Agent,
}

impl Client {
    fn new(base: String, authorization: Option<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        Self { base, authorization, agent }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}?api-version={API_VERSION}", self.base)
    }

    /// The JSON of `GET` of `path`.
    fn get(&self, path: &str) -> Result<Value, CiError> {
        let mut request = self.agent.get(&self.url(path)).header("Accept", "application/json");
        if let Some(authorization) = &self.authorization {
            request = request.header("Authorization", authorization);
        }
        let answer = request.call().map_err(|e| CiError::unavailable(format!("o Azure DevOps não respondeu: {e}")))?;
        read(answer)
    }

    /// `POST` of `body` to `path`.
    fn post(&self, path: &str, body: &Value) -> Result<Value, CiError> {
        let mut request = self
            .agent
            .post(&self.url(path))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(authorization) = &self.authorization {
            request = request.header("Authorization", authorization);
        }
        let answer = request
            .send(body.to_string())
            .map_err(|e| CiError::unavailable(format!("o Azure DevOps não respondeu: {e}")))?;
        read(answer)
    }
}

/// The JSON of an answer, or what Azure DevOps said when it said no.
fn read(mut answer: ureq::http::Response<ureq::Body>) -> Result<Value, CiError> {
    let status = answer.status().as_u16();
    let body = answer.body_mut().read_to_string().unwrap_or_default();
    // A PAT it does not take is answered with the page to sign in, as 203.
    if status == 203 || status == 401 {
        return Err(CiError::failed(format!(
            "o Azure DevOps respondeu {status}: não aceitou o PAT; confira a credencial e o escopo Code"
        )));
    }
    if !(200..300).contains(&status) {
        let message = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|value| value.get("message").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| body.chars().take(200).collect());
        return Err(CiError::failed(format!("o Azure DevOps respondeu {status}: {message}")));
    }
    Ok(serde_json::from_str(&body).unwrap_or(Value::Null))
}

impl Azure {
    fn host(&self) -> Result<Arc<dyn Host>, CiError> {
        lock(&self.state).host.clone().ok_or_else(|| CiError::unavailable("o plugin azure-devops não está ligado"))
    }

    /// The credential `name`, read through the host.
    fn credential(&self, name: &str) -> Result<ravenrs_git::Credential, CiError> {
        let host = self.host()?;
        host.require(Permission::Network, "falar com o Azure DevOps com credencial")?;
        Ok(credential(&host.secret(name)?))
    }

    /// The `api` of `settings`, else the plugin's.
    fn api(&self, settings: &Value) -> Result<Option<String>, CiError> {
        Ok(text(settings, "api").map(str::to_owned).or_else(|| {
            lock(&self.state).host.as_ref().and_then(|host| text(&host.settings(), "api").map(str::to_owned))
        }))
    }

    fn remember(&self, commit: &str, number: u64) {
        let mut state = lock(&self.state);
        state.pulls.retain(|(known, _)| known != commit);
        state.pulls.push_back((commit.to_owned(), number));
        while state.pulls.len() > REMEMBERED {
            state.pulls.pop_front();
        }
    }

    fn pull_of(&self, commit: &str) -> Option<u64> {
        lock(&self.state).pulls.iter().rev().find(|(known, _)| known == commit).map(|(_, number)| *number)
    }
}

impl Scm for Azure {
    fn id(&self) -> &str {
        "azure-devops"
    }

    fn checkout(&self, request: &CheckoutRequest, log: &dyn Fn(&str)) -> Result<Checkout, CiError> {
        let mut git = Git::at(&request.workspace).with_identity(IDENTITY.0, IDENTITY.1);
        let mut authorization = None;
        if let Some(name) = &request.credential {
            let credential = self.credential(name)?;
            authorization = Some(credential.http_authorization().to_owned());
            git = git.with_credential(credential);
        }
        let source = match &request.reference {
            Reference::PullRequest { number, .. } => {
                let repository = Repository::parse(&request.url)?;
                let client = Client::new(repository.api(self.api(&Value::Null)?.as_deref()), authorization);
                let pull = client.get(&format!("/pullRequests/{number}"))?;
                let source = pull
                    .get("sourceRefName")
                    .and_then(Value::as_str)
                    .ok_or_else(|| CiError::failed(format!("o Azure DevOps não disse de onde vem o PR {number}")))?
                    .to_owned();
                log(&format!("o PR {number} vem de {source}"));
                source
            }
            _ => String::new(),
        };
        let checkout = checkout_with(git, request, &source, log)?;
        if let Reference::PullRequest { number, .. } = &request.reference {
            self.remember(&checkout.commit, *number);
        }
        Ok(checkout)
    }
}

impl StatusReporter for Azure {
    fn id(&self) -> &str {
        "azure-devops"
    }

    fn report(&self, status: &CommitStatus, settings: &Value) -> Result<(), CiError> {
        let address = text(settings, "repository").unwrap_or(&status.repository);
        let repository = Repository::parse(address)?;
        let authorization = match text(settings, "credential") {
            Some(name) => Some(self.credential(name)?.http_authorization().to_owned()),
            None => None,
        };
        let client = Client::new(repository.api(self.api(settings)?.as_deref()), authorization);
        let state = match status.state {
            CommitState::Pending => "pending",
            CommitState::Success => "succeeded",
            CommitState::Failure => "failed",
            CommitState::Error => "error",
        };
        let mut body = json!({
            "state": state,
            "description": status.description,
            "context": { "name": status.context, "genre": text(settings, "genre").unwrap_or(GENRE) },
        });
        if let Some(url) = &status.url {
            body["targetUrl"] = json!(url);
        }
        let pull = settings.get("pull_request").and_then(Value::as_u64).or_else(|| self.pull_of(&status.commit));
        let path = match pull {
            Some(number) => format!("/pullRequests/{number}/statuses"),
            None => format!("/commits/{}/statuses", status.commit),
        };
        client.post(&path, &body).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_says_the_collection_the_project_and_the_repository() {
        let repo = Repository::parse("https://org@dev.azure.com/org/Meu%20Projeto/_git/site").expect("reads");
        assert_eq!(repo.collection, "https://dev.azure.com/org");
        assert_eq!(repo.project, "Meu%20Projeto");
        assert_eq!(repo.name, "site");
        assert_eq!(repo.api(None), "https://dev.azure.com/org/Meu%20Projeto/_apis/git/repositories/site");
        assert_eq!(
            repo.api(Some("http://127.0.0.1:9/col/")),
            "http://127.0.0.1:9/col/Meu%20Projeto/_apis/git/repositories/site"
        );
        let old = Repository::parse("https://org.visualstudio.com/proj/_git/repo").expect("reads");
        assert_eq!(old.collection, "https://org.visualstudio.com");
        let local = Repository::parse("/tmp/x/org/proj/_git/repo").expect("reads a folder too");
        assert_eq!((local.project.as_str(), local.name.as_str()), ("proj", "repo"));
        assert!(Repository::parse("https://github.com/dono/repo.git").is_err());
        assert!(Repository::parse("https://dev.azure.com/_git/").is_err());
    }
}
