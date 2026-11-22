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
//! - For anyone signed in, to read: `GET /api/ci/jobs`, `/api/ci/builds?job=`,
//!   `/api/ci/build?job=&number=` (with its stages and attachments), `/api/ci/log?job=&number=&after=`
//!   (the lines after `after`, for a reader that follows) and `/api/ci/queue`.
//! - For administrators: `GET /api/ci/credentials`, the credentials with no value.
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
//! CI ([`ROLE_ADMIN`], [`ROLE_RUN`], [`ROLE_READ`]). The first person made is an administrator
//! ([`create_user`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use raven_ci_plugin_api::{CiError, HttpRequest, ROLE_ADMIN, ROLE_READ, ROLE_RUN};
use ravenrs_app::{
    ObjectSpace, Query, Role, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, SpaceMaker, UserDirectory,
    Value, hash_password, security_entities,
};
use ravenrs_app_server::{ApiRequest, ApiResponse, ApiService, Caller};
use ravenrs_app_sqlite::{SqliteDatabase, TableMap};
use serde_json::{Value as Json, json};

use crate::Ci;

/// The file of people and roles, in the folder of data.
pub const USERS_FILE: &str = "usuarios.sqlite";

/// The prefix of the paths that come in with no token: the webhooks of other services.
pub const OPEN_PREFIX: &str = "/hooks/";

/// The roles of the CI, as the server knows them.
pub fn roles() -> Vec<Role> {
    vec![
        Role::new(ROLE_ADMIN).administrative(),
        Role::new(ROLE_RUN),
        Role::new(ROLE_READ),
        Role::new(crate::agents::ROLE_AGENT),
    ]
}

fn failed(e: impl std::fmt::Display) -> CiError {
    CiError::failed(format!("os usuários: {e}"))
}

/// What makes units of work over the people in `file`: each thread that answers opens it.
pub fn users(file: &Path) -> Result<SpaceMaker, CiError> {
    let connection = Rc::new(rusqlite::Connection::open(file).map_err(failed)?);
    let builder =
        security_entities().into_iter().try_fold(SqliteDatabase::builder(connection), |builder, entity| {
            let table = TableMap::new(format!("ci_{}", entity.name.to_lowercase()));
            builder.managed(entity, table)
        });
    let database = builder.map_err(failed)?.build();
    Ok(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>))
}

/// Makes the person `name` with `password` and the role `role` in `file`. The first person is an
/// administrator whatever `role` says; after that, `role` has to be one of the CI's, and a name
/// already there is refused.
pub fn create_user(file: &Path, name: &str, password: &str, role: &str) -> Result<String, CiError> {
    let maker = users(file)?;
    let directory = UserDirectory::new(maker.clone(), roles()).map_err(failed)?;
    if directory.is_empty().map_err(failed)? {
        directory.create_first_administrator(name, password).map_err(failed)?;
        return Ok(ROLE_ADMIN.to_owned());
    }
    if !roles().iter().any(|known| known.name() == role) {
        return Err(CiError::settings(
            "papel",
            format!("{role} não existe; use {ROLE_ADMIN}, {ROLE_RUN} ou {ROLE_READ}"),
        ));
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(CiError::settings("nome", "um usuário tem nome"));
    }
    if directory.identity_of(name).map_err(failed)?.is_some() {
        return Err(CiError::settings("nome", format!("já há o usuário {name}")));
    }
    let mut space = maker();
    let records = space.query(SECURITY_ROLE, &Query::all()).map_err(failed)?;
    let role_key = records
        .iter()
        .find(|record| record.get("name") == Value::Text(role.to_owned()))
        .map(|record| record.get("id"))
        .ok_or_else(|| failed(format!("o papel {role} não está no arquivo")))?;
    let mut user = space.new_object(SECURITY_USER).map_err(failed)?;
    user.set("name", name);
    user.set("password_hash", hash_password(password).map_err(failed)?);
    user.set("must_change_password", false);
    let key = user.get("id");
    space.save(user).map_err(failed)?;
    let mut link = space.new_object(SECURITY_USER_ROLE).map_err(failed)?;
    link.set("user_id", key);
    link.set("role_id", role_key);
    space.save(link).map_err(failed)?;
    space.commit().map_err(failed)?;
    Ok(role.to_owned())
}

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
    if path.as_slice() == ["api", "ci", "credentials"] && request.method == "GET" {
        return Some(match who() {
            None => unauthorized(),
            Some(caller) if !caller.administrator => {
                json(403, &json!({ "error": "denied", "message": "só administradores veem as credenciais" }))
            }
            Some(_) => ci.credentials().map_or_else(|e| from_error(&e), |list| json(200, &to_json(&list))),
        });
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
            json!({
                "build": to_json(&ci.build(job, n)?),
                "stages": to_json(&ci.stages(job, n)?),
                "attachments": to_json(&ci.attachments(job, n)?),
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

/// The route of a plugin on that answers `request`, when there is one.
fn plugin_route(
    ci: &Ci,
    request: &ApiRequest,
    path: &[&str],
    who: &mut dyn FnMut() -> Option<Caller>,
) -> Option<ApiResponse> {
    let joined = format!("/{}", path.join("/"));
    let route = ci
        .registry()
        .on()
        .iter()
        .flat_map(|plugin| plugin.routes())
        .find(|route| route.matches(&request.method, &joined))?;
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
