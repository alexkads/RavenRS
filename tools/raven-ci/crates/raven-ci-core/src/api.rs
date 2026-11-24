//! The HTTP API of the server: who may come in, what the core answers, and the routes of the
//! plugins switched on.
//!
//! # Its role
//!
//! `raven-ci servir` answers over HTTP with [`ravenrs_app_server`], whose `ApiService` signs people
//! in (`POST /api/auth/token`), keeps keys for programs and checks every token. The CI's own paths
//! change while it runs (a plugin switched on brings its routes, switched off takes them), so
//! they are answered by the service's `fallback`, which this module makes: [`service`].
//!
//! # What it answers
//!
//! - `GET /health`, with no one signed in (the HTTP layer of the server answers it).
//! - For anyone signed in, `GET /api/ci/build/summary?job=&number=[&format=md]`: the summary of a
//!   build for programs and sessions of AI (#941, [`summary`](crate::summary)).
//! - For anyone signed in, to read: `GET /api/ci/jobs`, `/api/ci/builds?job=`,
//!   `/api/ci/build?job=&number=` (with its stages and attachments), `/api/ci/log?job=&number=&after=`
//!   (the lines after `after`, for a reader that follows) and `/api/ci/queue`.
//! - For administrators: `GET /api/ci/credentials`, the credentials with no value, each with what
//!   names it; `POST /api/ci/credentials` (`{name, kind, description?, fields}`), one made or its
//!   value rotated; `DELETE /api/ci/credentials?name=`, one revoked (#922). Each goes to the audit
//!   with who did it.
//! - For administrators: `POST /api/ci/jobs`, a job made or changed, and `POST /api/ci/config`, a
//!   whole configuration applied (#898); the triggers of what changed are watched again at once.
//! - The [`Route`](raven_ci_plugin_api::Route)s of the plugins on: a public one with no one signed
//!   in (under `/hooks/`, the only paths the server lets in with no token), any other only for
//!   someone signed in, told their name and roles.
//! - Everything else goes on to the API of `ravenrs-app-server`, which answers 401 to no one.
//!
//! # Who
//!
//! People and their roles are in `usuarios.sqlite`, in the folder of data, with the roles of the
//! CI ([`ROLE_ADMIN`], [`ROLE_RUN`](raven_ci_plugin_api::ROLE_RUN), [`ROLE_READ`](raven_ci_plugin_api::ROLE_READ)),
//! kept by [`people`](crate::people). The first person made is an administrator ([`create_user`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use raven_ci_plugin_api::{CiError, HttpRequest, ROLE_ADMIN};
use ravenrs_app::UserDirectory;
use ravenrs_app_server::{ApiRequest, ApiResponse, ApiService, Caller};
use serde_json::{Value as Json, json};

use crate::Ci;

/// The prefix of the paths that come in with no token: the webhooks of other services.
pub const OPEN_PREFIX: &str = "/hooks/";

pub use crate::people::{USERS_FILE, create_user, roles, users};

/// The service one thread of the server answers with: the people of `users_file`, and `ci` behind
/// its fallback.
pub fn service(ci: Arc<Ci>, users_file: &Path) -> Result<ApiService, String> {
    let maker = users(users_file).map_err(|e| e.to_string())?;
    UserDirectory::new(maker.clone(), roles()).map_err(|e| e.to_string())?;
    let service = ApiService::new(maker, roles()).map_err(|e| e.to_string())?.title("Raven CI");
    Ok(service.fallback(move |request, who| answer(&ci, request, who)))
}

/// The service of [`service`], with what the panel needs besides: the routes of
/// [`panel`](crate::panel), its files and what the server says of itself (`options`).
pub fn service_with(
    ci: Arc<Ci>,
    users_file: &Path,
    options: crate::panel::PanelOptions,
) -> Result<ApiService, String> {
    let maker = users(users_file).map_err(|e| e.to_string())?;
    UserDirectory::new(maker.clone(), roles()).map_err(|e| e.to_string())?;
    let service = ApiService::new(maker, roles()).map_err(|e| e.to_string())?.title("Raven CI");
    let mut options = options;
    options.users_file.get_or_insert_with(|| users_file.to_path_buf());
    Ok(service.fallback(move |request, who| {
        crate::panel::answer(&ci, &options, request, who).or_else(|| answer(&ci, request, who))
    }))
}

/// The file of people of the core whose data is in `dir`.
pub fn users_file(dir: &Path) -> PathBuf {
    dir.join(USERS_FILE)
}

fn json(status: u16, value: &Json) -> ApiResponse {
    ApiResponse::json(status, value.clone())
}

fn unauthorized() -> ApiResponse {
    json(401, &json!({ "error": "unauthorized", "message": "Entre para ver isto." }))
}

fn from_error(e: &CiError) -> ApiResponse {
    let status = match e {
        CiError::Unavailable(_) => 404,
        CiError::Settings { .. } => 400,
        CiError::Denied { .. } => 403,
        _ => 500,
    };
    json(status, &json!({ "error": "ci", "message": e.to_string() }))
}

fn to_json(value: &impl serde::Serialize) -> Json {
    serde_json::to_value(value).unwrap_or(Json::Null)
}

/// What the core answers `request`, or `None` for a path that is not its own.
fn answer(ci: &Ci, request: &ApiRequest, who: &mut dyn FnMut() -> Option<Caller>) -> Option<ApiResponse> {
    let path: Vec<&str> = request.path.iter().map(String::as_str).collect();
    if request.method == "POST"
        && let ["api", "ci", what @ ("jobs" | "config")] = path.as_slice()
    {
        return Some(write(ci, what, request, who));
    }
    if let Some(answered) = crate::agents::answer(ci, request, &path, who) {
        return Some(answered);
    }
    // A build asked for a job that is not there says so, before the trigger of the API says the
    // job lacks it (#929): someone who integrates reads "falta o gatilho" as a setting to fix.
    if request.method == "POST" && path.as_slice() == ["api", "ci", "run"] {
        let asked = serde_json::from_slice::<Json>(&request.body).ok();
        if let Some(job) = asked.as_ref().and_then(|body| body.get("job")).and_then(Json::as_str)
            && ci.job(job).is_err()
        {
            if who().is_none() {
                return Some(unauthorized());
            }
            return Some(json(
                404,
                &json!({ "error": "not_found", "message": format!("o job {job} não existe") }),
            ));
        }
    }
    if path.as_slice() == ["api", "ci", "credentials"] {
        return Some(credentials(ci, request, who));
    }
    if path.as_slice() == ["api", "ci", "build", "summary"] && request.method == "GET" {
        if who().is_none() {
            return Some(unauthorized());
        }
        return Some(summary(ci, &request.query).unwrap_or_else(|e| from_error(&e)));
    }
    if let ["api", "ci", what] = path.as_slice()
        && request.method == "GET"
        && READS.contains(what)
    {
        if who().is_none() {
            return Some(unauthorized());
        }
        return Some(read(ci, what, &request.query).unwrap_or_else(|e| from_error(&e)));
    }
    plugin_route(ci, request, &path, who)
}

/// `POST /api/ci/jobs` (one job, as a [`JobConfig`](crate::config::JobConfig) in JSON, with its
/// pipeline in it) and `POST /api/ci/config[?remover=1]` (a whole
/// [`ServerConfig`](crate::config::ServerConfig), its files already read): only for
/// administrators. The triggers of what changed are watched again at once.
fn write(ci: &Ci, what: &str, request: &ApiRequest, who: &mut dyn FnMut() -> Option<Caller>) -> ApiResponse {
    match who() {
        None => return unauthorized(),
        Some(caller) if !caller.administrator => {
            return json(
                403,
                &json!({ "error": "denied", "message": "só administradores mudam os jobs e a configuração" }),
            );
        }
        Some(_) => {}
    }
    let bad = |e: serde_json::Error| CiError::settings("corpo", format!("não deu para ler o JSON: {e}"));
    let done = if what == "jobs" {
        serde_json::from_slice::<crate::config::JobConfig>(&request.body).map_err(bad).and_then(|job| {
            let spec = job.spec()?;
            let made = ci.job(&spec.name).is_err();
            let name = spec.name.clone();
            ci.save_job(spec)?;
            let warnings = ci.watch_job(&name);
            Ok(json!({ "job": name, "created": made, "warnings": warnings }))
        })
    } else {
        let remove = request.query.get("remover").is_some_and(|v| v == "1" || v == "true");
        serde_json::from_slice::<crate::config::ServerConfig>(&request.body)
            .map_err(bad)
            .and_then(|config| ci.apply_config(&config, remove))
            .map(|changes| json!({ "changes": changes }))
    };
    done.map_or_else(|e| from_error(&e), |body| json(200, &body))
}

/// What `POST /api/ci/credentials` takes: the fields go in the body, never in the address.
#[derive(serde::Deserialize)]
struct NewCredential {
    name: String,
    kind: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    fields: Json,
}

/// `/api/ci/credentials`, only for administrators: `GET` lists them with no value, `POST` makes
/// one or rotates it, `DELETE ?name=` revokes it. Making, rotating and revoking go to the audit
/// with who did it.
fn credentials(ci: &Ci, request: &ApiRequest, who: &mut dyn FnMut() -> Option<Caller>) -> ApiResponse {
    let caller = match who() {
        None => return unauthorized(),
        Some(caller) if !caller.administrator => {
            return json(
                403,
                &json!({ "error": "denied", "message": "só administradores cuidam das credenciais" }),
            );
        }
        Some(caller) => caller,
    };
    let done = match request.method.as_str() {
        "GET" => ci.credentials().map(|list| to_json(&list)),
        "POST" => serde_json::from_slice::<NewCredential>(&request.body)
            .map_err(|e| CiError::settings("corpo", format!("não deu para ler o JSON: {e}")))
            .and_then(|asked| {
                let existed = ci.credentials()?.iter().any(|c| c.name == asked.name);
                ci.save_credential(
                    &asked.name,
                    &asked.kind,
                    &asked.fields,
                    asked.description.as_deref(),
                    Some(&caller.name),
                )?;
                Ok(json!({ "name": asked.name, "kind": asked.kind, "created": !existed }))
            }),
        "DELETE" => wanted(&request.query, "name").and_then(|name| {
            ci.delete_credential(name, Some(&caller.name))?;
            Ok(json!({ "name": name, "deleted": true }))
        }),
        other => Err(CiError::settings("método", format!("{other} não serve aqui: GET, POST ou DELETE"))),
    };
    done.map_or_else(|e| from_error(&e), |body| json(200, &body))
}

/// What the core reads at `GET /api/ci/{what}`; any other path under `/api/ci/` is a plugin's,
/// as `POST /api/ci/run`.
const READS: [&str; 5] = ["jobs", "builds", "build", "log", "queue"];

/// The parameter `name` of the query, or a refusal saying it is missing.
fn wanted<'q>(query: &'q BTreeMap<String, String>, name: &str) -> Result<&'q str, CiError> {
    query.get(name).map(String::as_str).ok_or_else(|| CiError::settings(name, "falta na consulta"))
}

fn number(query: &BTreeMap<String, String>, name: &str) -> Result<u64, CiError> {
    wanted(query, name)?.parse().map_err(|_| CiError::settings(name, "tem que ser um número"))
}

/// The reading routes of the core.
fn read(ci: &Ci, what: &str, query: &BTreeMap<String, String>) -> Result<ApiResponse, CiError> {
    let body = match what {
        "jobs" => to_json(&ci.jobs()?),
        "builds" => to_json(&ci.builds(wanted(query, "job")?)?),
        "build" => {
            let (job, n) = (wanted(query, "job")?, number(query, "number")?);
            let times: Vec<Json> = ci
                .stage_times(job, n)?
                .into_iter()
                .map(|(stage, start, end)| json!({ "stage": stage, "start_ms": start, "end_ms": end }))
                .collect();
            let started = ci.build_log(job, n, 0)?.first().map(|line| line.at_ms);
            json!({
                "build": to_json(&ci.build(job, n)?),
                "stages": to_json(&ci.stages(job, n)?),
                "attachments": to_json(&ci.attachments(job, n)?),
                "stage_times": times,
                "started_ms": started,
                "plugins": ci.plugins_of(job, n)?,
            })
        }
        "log" => {
            let (job, n) = (wanted(query, "job")?, number(query, "number")?);
            let after = query.get("after").and_then(|a| a.parse().ok()).unwrap_or(0);
            to_json(&ci.build_log(job, n, after)?)
        }
        _ => to_json(&ci.queue()?),
    };
    Ok(json(200, &body))
}

/// `GET /api/ci/build/summary?job=&number=[&format=md]`: the summary of a build for programs and
/// sessions of AI ([`summary`](crate::summary)), in JSON or, with `format=md`, as Markdown. With no
/// `number`, the last build of the job.
fn summary(ci: &Ci, query: &BTreeMap<String, String>) -> Result<ApiResponse, CiError> {
    let job = wanted(query, "job")?;
    let n = match query.get("number") {
        Some(_) => number(query, "number")?,
        None => ci
            .builds(job)?
            .first()
            .map(|build| build.number)
            .ok_or_else(|| CiError::unavailable(format!("o job {job} ainda não tem build")))?,
    };
    let summary = ci.summary(job, n)?;
    if query.get("format").is_some_and(|f| f == "md") {
        let text = crate::summary::to_markdown(&summary);
        return Ok(ApiResponse::raw(200, "text/markdown; charset=utf-8".to_owned(), text.into_bytes()));
    }
    Ok(json(200, &to_json(&summary)))
}

/// The route of a plugin on that answers `request`, when there is one.
fn plugin_route(
    ci: &Ci,
    request: &ApiRequest,
    path: &[&str],
    who: &mut dyn FnMut() -> Option<Caller>,
) -> Option<ApiResponse> {
    let joined = format!("/{}", path.join("/"));
    let Some(route) = ci
        .registry()
        .on()
        .iter()
        .flat_map(|plugin| plugin.routes())
        .find(|route| route.matches(&request.method, &joined))
    else {
        // A route of a plugin that is not on says which plugin to switch on (#942), to someone
        // signed in.
        let off = ci
            .registry()
            .not_on()
            .into_iter()
            .find(|plugin| plugin.routes().iter().any(|route| route.matches(&request.method, &joined)))?;
        if who().is_none() {
            return Some(unauthorized());
        }
        let name = off.name().to_owned();
        return Some(json(
            503,
            &json!({
                "error": "plugin_off",
                "plugin": name,
                "message": format!("o plugin {name} está desligado: ligue-o na página Plugins do painel, ou com raven-ci plugins ligar {name} --servidor URL"),
            }),
        ));
    };
    let caller = if route.public {
        if !joined.starts_with(OPEN_PREFIX) {
            return Some(json(
                500,
                &json!({
                    "error": "ci",
                    "message": format!("a rota pública {} não está em {OPEN_PREFIX}", route.path),
                }),
            ));
        }
        None
    } else {
        match who() {
            Some(caller) => Some(caller),
            None => return Some(unauthorized()),
        }
    };
    let asked = HttpRequest {
        method: request.method.clone(),
        path: joined,
        query: request.query.clone(),
        headers: request.headers.clone(),
        body: request.body.clone(),
        actor: caller.as_ref().map(|c| c.name.clone()),
        roles: caller
            .map(|c| {
                let mut roles = c.roles;
                if c.administrator && !roles.iter().any(|r| r == ROLE_ADMIN) {
                    roles.push(ROLE_ADMIN.to_owned());
                }
                roles
            })
            .unwrap_or_default(),
    };
    let answered = (route.handler)(&asked);
    Some(ApiResponse::raw(answered.status, answered.content_type, answered.body))
}
