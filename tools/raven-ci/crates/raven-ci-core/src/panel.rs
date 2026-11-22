//! What the panel of Raven CI reads and does over the API, the screens the plugins put in it, the
//! panel's own files and the live channel that keeps it up to date.
//!
//! # Its role
//!
//! The panel (`raven-ci-painel`, on the desktop and in the browser) runs no code of the server:
//! it reads JSON and asks for things. [`api`](crate::api) answers the reading routes of C-09;
//! this module answers the rest of what the screens of Fase 4 need, through the same fallback,
//! and makes the events of the topic [`LIVE_TOPIC`].
//!
//! # What it answers
//!
//! | Route | Who | What |
//! |---|---|---|
//! | `GET /api/ci/screens` | signed in | the screens of the plugins on ([`screens`]) |
//! | `GET /api/ci/server` | signed in | version, executors, memory of the process, plugins on |
//! | `GET /api/ci/overview` | signed in | every job with its last build, the last durations and its parameters |
//! | `GET /api/ci/occupancy?since_ms=` | signed in | the executors and the builds since then, with who ran each |
//! | `GET /api/ci/attachment?id=` | signed in | the bytes of what a build left |
//! | `POST /api/ci/pipeline/check` | signed in | the errors of a pipeline, with their lines |
//! | `POST /api/ci/cancel`, `/rebuild`, `/priority` | Executores, Administradores | acting on a build |
//! | `POST /api/ci/job` | Administradores | saving a job (`name`, `format`, `pipeline`, `settings`), checked first |
//! | `GET /api/ci/plugins`, `POST /api/ci/plugins/enable`, `/disable` | Administradores | the plugins |
//! | `GET /api/ci/users`, `POST /api/ci/users` | Administradores | people and their roles |
//! | `GET /api/ci/audit?plugin=&limit=` | Administradores | what plugins and people did, the newest first |
//! | `GET /painel/...` | anyone | the files of the panel for the browser, from [`PanelOptions::files`] |
//!
//! # The screens of plugins
//!
//! A plugin puts screens in the panel the way a module of `ravenrs-app` does: in
//! [`Module::extend_model`](ravenrs_app::Module::extend_model) it adds a dashboard view
//! (`Views/<name>_DashboardView`, `Kind = DashboardView`, with `Items`) and its place in
//! `NavigationItems`, the shape [`DashboardInfo`](ravenrs_app::DashboardInfo) has. The group
//! [`BUILD_TAB`] makes it a tab on the page of a build; any other group makes it an item of the
//! menu, with its page. A piece whose `Control` starts with `/api/` is the route whose JSON the
//! panel shows, with `{job}` and `{number}` replaced on the page of a build. Only plugins on are
//! asked, so a plugin switched off leaves the screens.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use raven_ci_plugin_api::{BuildRequest, BuildStatus, CiError, ROLE_ADMIN, ROLE_RUN};
use ravenrs_app::{
    FieldKind, ModelNode, Query, SECURITY_ROLE, SECURITY_USER, SECURITY_USER_ROLE, Value, generate_model,
};
use ravenrs_app_server::{ApiRequest, ApiResponse, Caller};
use serde_json::{Value as Json, json};

use crate::{Ci, JobSpec};

/// The topic of the live channel the panel listens to: `build`, `line` and `plugins` events.
pub const LIVE_TOPIC: &str = "ci";

/// The group of navigation that makes a plugin's screen a tab on the page of a build.
pub const BUILD_TAB: &str = "build";

/// Where the files of the panel are served, with no token.
pub const PANEL_PATH: &str = "/painel";

/// How many builds the panel's overview takes the durations of, per job.
const TREND: usize = 10;

/// What the server tells the panel about itself, and where the panel's files are.
#[derive(Clone, Debug, Default)]
pub struct PanelOptions {
    /// How many executors run builds.
    pub executors: usize,
    /// The folder of the panel for the browser (`index.html` and `pkg/`), served at
    /// [`PANEL_PATH`]; `None` serves nothing there.
    pub files: Option<PathBuf>,
    /// The version of the server.
    pub version: String,
    /// The file of people and roles.
    pub users_file: Option<PathBuf>,
}

fn reply(status: u16, value: Json) -> ApiResponse {
    ApiResponse::json(status, value)
}

fn refused(status: u16, message: impl Into<String>) -> ApiResponse {
    reply(status, json!({ "error": "ci", "message": message.into() }))
}

fn from_error(e: &CiError) -> ApiResponse {
    let status = match e {
        CiError::Unavailable(_) => 404,
        CiError::Settings { .. } => 400,
        CiError::Denied { .. } => 403,
        _ => 500,
    };
    refused(status, e.to_string())
}

fn to_json(value: &impl serde::Serialize) -> Json {
    serde_json::to_value(value).unwrap_or(Json::Null)
}

/// Whether `caller` may act on builds: an administrator or an executor.
fn runs(caller: &Caller) -> bool {
    caller.administrator || caller.roles.iter().any(|r| r == ROLE_ADMIN || r == ROLE_RUN)
}

fn administers(caller: &Caller) -> bool {
    caller.administrator || caller.roles.iter().any(|r| r == ROLE_ADMIN)
}

/// Who may do something: anyone signed in, those who run builds, or administrators.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Need {
    SignedIn,
    Runner,
    Admin,
}

fn body(request: &ApiRequest) -> Json {
    serde_json::from_slice(&request.body).unwrap_or(Json::Null)
}

fn text<'j>(value: &'j Json, name: &str) -> Result<&'j str, CiError> {
    value.get(name).and_then(Json::as_str).ok_or_else(|| CiError::settings(name, "falta no corpo"))
}

fn whole(value: &Json, name: &str) -> Result<u64, CiError> {
    value
        .get(name)
        .and_then(Json::as_u64)
        .ok_or_else(|| CiError::settings(name, "falta no corpo, como número"))
}

/// What this module answers `request`, or `None` for a path that is not its own.
pub fn answer(
    ci: &Ci,
    options: &PanelOptions,
    request: &ApiRequest,
    who: &mut dyn FnMut() -> Option<Caller>,
) -> Option<ApiResponse> {
    let path: Vec<&str> = request.path.iter().map(String::as_str).collect();
    if path.first() == Some(&PANEL_PATH.trim_start_matches('/')) && request.method == "GET" {
        return Some(panel_file(options, &path[1..]));
    }
    let (need, what) = match (request.method.as_str(), path.as_slice()) {
        ("GET", ["api", "ci", what @ ("screens" | "server" | "overview" | "occupancy" | "attachment")]) => {
            (Need::SignedIn, *what)
        }
        ("POST", ["api", "ci", "pipeline", "check"]) => (Need::SignedIn, "check"),
        ("POST", ["api", "ci", what @ ("cancel" | "rebuild" | "priority")]) => (Need::Runner, *what),
        ("POST", ["api", "ci", "job"]) => (Need::Admin, "job"),
        ("GET", ["api", "ci", what @ ("plugins" | "users" | "audit")]) => (Need::Admin, *what),
        ("POST", ["api", "ci", "plugins", what @ ("enable" | "disable")]) => (Need::Admin, *what),
        ("POST", ["api", "ci", "users"]) => (Need::Admin, "new-user"),
        _ => return None,
    };
    let Some(caller) = who() else {
        return Some(refused(401, "Entre para ver isto."));
    };
    let allowed = match need {
        Need::SignedIn => true,
        Need::Runner => runs(&caller),
        Need::Admin => administers(&caller),
    };
    if !allowed {
        let whom = if need == Need::Admin { "administradores" } else { "executores e administradores" };
        return Some(refused(403, format!("só {whom} podem fazer isto")));
    }
    Some(act(ci, options, request, &caller, what).unwrap_or_else(|e| from_error(&e)))
}

fn act(
    ci: &Ci,
    options: &PanelOptions,
    request: &ApiRequest,
    caller: &Caller,
    what: &str,
) -> Result<ApiResponse, CiError> {
    let asked = body(request);
    let query = &request.query;
    let value = match what {
        "screens" => to_json(&screens(ci)),
        "server" => server(ci, options),
        "overview" => overview(ci)?,
        "occupancy" => {
            let since = query.get("since_ms").and_then(|s| s.parse().ok()).unwrap_or(0);
            occupancy(ci, options, since)?
        }
        "attachment" => {
            let id = query
                .get("id")
                .and_then(|i| i.parse::<i64>().ok())
                .ok_or_else(|| CiError::settings("id", "falta na consulta, como número"))?;
            let bytes = ci.attachment_bytes(id)?;
            let mime = attachment_mime(ci, id).unwrap_or_else(|| "application/octet-stream".to_owned());
            return Ok(ApiResponse::raw(200, mime, bytes));
        }
        "check" => {
            let errors = check(ci, text(&asked, "format")?, text(&asked, "text")?)?;
            json!({ "errors": errors })
        }
        "cancel" => {
            let (job, number) = (text(&asked, "job")?, whole(&asked, "number")?);
            let status = ci.cancel(job, number, &format!("cancelado por {}", caller.name))?;
            record(ci, &caller.name, &format!("cancelou {job} #{number}"));
            json!({ "status": to_json(&status) })
        }
        "rebuild" => {
            let (job, number) = (text(&asked, "job")?, whole(&asked, "number")?);
            let new = ci.rebuild(job, number, Some(&caller.name))?;
            record(ci, &caller.name, &format!("repetiu {job} #{number} como #{new}"));
            json!({ "number": new })
        }
        "priority" => {
            let (job, number) = (text(&asked, "job")?, whole(&asked, "number")?);
            let priority = asked.get("priority").and_then(Json::as_i64).unwrap_or(1);
            ci.set_priority(job, number, i32::try_from(priority).unwrap_or(i32::MAX))?;
            record(ci, &caller.name, &format!("pôs {job} #{number} com prioridade {priority}"));
            json!({ "priority": priority })
        }
        "job" => {
            let (name, format, pipeline) =
                (text(&asked, "name")?, text(&asked, "format")?, text(&asked, "pipeline")?);
            let errors = check(ci, format, pipeline)?;
            if !errors.is_empty() {
                return Ok(reply(
                    400,
                    json!({ "error": "pipeline", "message": "o pipeline tem erros", "errors": errors }),
                ));
            }
            let spec = match ci.job(name) {
                Ok(mut old) => {
                    old.format = format.to_owned();
                    old.pipeline = pipeline.to_owned();
                    old
                }
                Err(_) => JobSpec::new(name, format, pipeline),
            };
            // The settings of the plugins it names (its triggers, say), when given.
            let spec = match asked.get("settings") {
                Some(settings @ Json::Object(_)) => spec.settings(settings.clone()),
                _ => spec,
            };
            ci.save_job(spec)?;
            record(ci, &caller.name, &format!("salvou o pipeline de {name}"));
            json!({ "saved": name })
        }
        "plugins" => to_json(&ci.registry().list()?),
        "enable" | "disable" => {
            let name = text(&asked, "name")?;
            let changed = if what == "enable" { ci.enable_plugin(name)? } else { ci.disable_plugin(name)? };
            let words = if what == "enable" { "ligou" } else { "desligou" };
            record(ci, &caller.name, &format!("{words} o plugin {name}"));
            json!({ "changed": changed })
        }
        "users" => users(options)?,
        "new-user" => {
            let file = options
                .users_file
                .as_deref()
                .ok_or_else(|| CiError::unavailable("este servidor não guarda usuários"))?;
            let (name, password) = (text(&asked, "name")?, text(&asked, "password")?);
            let role = asked.get("role").and_then(Json::as_str).unwrap_or(ROLE_RUN);
            let role = crate::people::create_user(file, name, password, role)?;
            record(ci, &caller.name, &format!("criou o usuário {name} com o papel {role}"));
            json!({ "name": name, "role": role })
        }
        _ => {
            let limit = query.get("limit").and_then(|l| l.parse().ok()).unwrap_or(200);
            audit(ci, query.get("plugin").map(String::as_str), limit)?
        }
    };
    Ok(reply(200, value))
}

/// A screen a plugin puts in the panel.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Screen {
    /// The plugin that put it there.
    pub plugin: String,
    /// `page` (an item of the menu, with its page) or `build_tab` (a tab on the page of a build).
    pub kind: String,
    /// What it is known by.
    pub name: String,
    /// What people see.
    pub caption: String,
    /// The group of the menu it goes under.
    pub group: String,
    /// The name of its icon, when it has one.
    pub icon: Option<String>,
    /// Its pieces, in order.
    pub items: Vec<ScreenItem>,
}

/// A piece of a screen.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ScreenItem {
    /// What people see over it.
    pub caption: String,
    /// The route whose JSON it shows, as `/api/ci/locks`, when it is one.
    pub source: Option<String>,
}

/// The screens the plugins on put in the panel, in the order the plugins started.
pub fn screens(ci: &Ci) -> Vec<Screen> {
    let mut found = Vec::new();
    for plugin in ci.registry().on() {
        let Ok(mut root) = generate_model(&plugin.entities()) else { continue };
        plugin.extend_model(&mut root);
        let views = root.get("Views");
        let Some(navigation) = root.get("NavigationItems") else { continue };
        for group in navigation.in_order() {
            for item in group.in_order() {
                let Some(view_id) = item.text("View") else { continue };
                let Some(view) = views.and_then(|v| v.get(view_id)) else { continue };
                if view.text("Kind") != Some("DashboardView") {
                    continue;
                }
                let items = view.get("Items").map(pieces).unwrap_or_default();
                found.push(Screen {
                    plugin: plugin.name().to_owned(),
                    kind: if group.id == BUILD_TAB { "build_tab" } else { "page" }.to_owned(),
                    name: item.id.clone(),
                    caption: item.text("Caption").unwrap_or(&item.id).to_owned(),
                    group: group.text("Caption").unwrap_or(&group.id).to_owned(),
                    icon: item.text("Icon").map(str::to_owned),
                    items,
                });
            }
        }
    }
    found
}

fn pieces(items: &ModelNode) -> Vec<ScreenItem> {
    items
        .in_order()
        .into_iter()
        .map(|piece| ScreenItem {
            caption: piece.text("Caption").unwrap_or(&piece.id).to_owned(),
            source: piece.text("Control").filter(|c| c.starts_with("/api/")).map(str::to_owned),
        })
        .collect()
}

impl Ci {
    /// The panel of this server, when its configuration says the address people reach it at:
    /// the link a notice of a build gives.
    pub(crate) fn panel_url(&self) -> Option<String> {
        let url = self.url().ok().flatten()?;
        Some(format!("{}{PANEL_PATH}", url.trim_end_matches('/')))
    }
}

/// What the server says of itself.
fn server(ci: &Ci, options: &PanelOptions) -> Json {
    let on: Vec<String> = ci.registry().on().iter().map(|p| p.name().to_owned()).collect();
    json!({
        "version": options.version,
        "executors": options.executors,
        "memory_bytes": ravenrs_metrics::rss_bytes_of(std::process::id()),
        "plugins_on": on,
        "formats": formats(ci),
        "now_ms": ci.store().now_ms(),
    })
}

/// The formats of pipeline the plugins on read.
fn formats(ci: &Ci) -> Vec<String> {
    ci.registry().on().iter().flat_map(|p| p.pipeline_formats()).map(|f| f.id().to_owned()).collect()
}

/// How long a build took, or has taken so far.
fn duration_ms(build: &crate::Build, now: i64) -> i64 {
    build.ended_ms.unwrap_or(now).saturating_sub(build.created_ms).max(0)
}

/// Every job with its last build and the durations of the last ones, the folders, and the queue.
fn overview(ci: &Ci) -> Result<Json, CiError> {
    let now = ci.store().now_ms();
    let mut jobs = Vec::new();
    let mut running = 0;
    for job in ci.jobs()? {
        let builds = ci.builds(&job.name)?;
        running += builds.iter().filter(|b| b.status == BuildStatus::Running).count();
        let trend: Vec<i64> = builds
            .iter()
            .filter(|b| b.status.ended())
            .take(TREND)
            .map(|b| duration_ms(b, now))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let last = builds.first().map(|b| {
            json!({
                "number": b.number,
                "status": to_json(&b.status),
                "cause": b.cause,
                "actor": b.actor,
                "duration_ms": duration_ms(b, now),
                "created_ms": b.created_ms,
                "ended_ms": b.ended_ms,
            })
        });
        jobs.push(json!({
            "name": job.name,
            "title": job.title,
            "folder": job.folder(),
            "format": job.format,
            "last": last,
            "trend_ms": trend,
            "parameters": parameters(ci, &job),
        }));
    }
    let queue = ci.queue()?;
    let oldest = queue.iter().map(|b| b.created_ms).min();
    Ok(json!({
        "now_ms": now,
        "jobs": jobs,
        "folders": to_json(&ci.folders()?),
        "queued": queue.len(),
        "running": running,
        "oldest_queued_ms": oldest,
        "formats": formats(ci),
    }))
}

/// The parameters the pipeline of `job` asks for, as a form shows them; none when it does not read.
fn parameters(ci: &Ci, job: &JobSpec) -> Json {
    let Ok(format) = ci.format(&job.format) else { return json!([]) };
    let Ok(pipeline) = format.read(&job.name, &job.pipeline) else { return json!([]) };
    Json::Array(
        pipeline
            .parameters
            .iter()
            .map(|field| {
                let (kind, choices) = match &field.kind {
                    FieldKind::Boolean => ("bool", Vec::new()),
                    FieldKind::Enum(options) => ("choice", options.iter().map(|o| o.code.clone()).collect()),
                    _ => ("text", Vec::new()),
                };
                json!({
                    "name": field.name,
                    "caption": field.caption,
                    "kind": kind,
                    "choices": choices,
                    "default": ravenrs_app::value_to_json(&field.default_value),
                })
            })
            .collect(),
    )
}

/// A value of a record as text: its key, or nothing.
fn key(value: &Value) -> String {
    ravenrs_app::key_text(value).unwrap_or_default()
}

/// The builds made since `since_ms`, the newest first, at most `limit`.
fn recent(ci: &Ci, since_ms: i64, limit: usize) -> Result<Vec<crate::Build>, CiError> {
    let keys: Vec<(String, u64)> = {
        let db = ci.db();
        let mut stmt = db
            .prepare(
                "SELECT job, number FROM ci_builds WHERE created_ms >= ?1 ORDER BY created_ms DESC LIMIT ?2",
            )
            .map_err(crate::store::db)?;
        let rows = stmt
            .query_map(rusqlite::params![since_ms, limit as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .map_err(crate::store::db)?;
        rows.collect::<Result<_, _>>().map_err(crate::store::db)?
    };
    keys.iter().map(|(job, number)| ci.build(job, *number)).collect()
}

/// When a build began: its first line of log, or when it was asked for.
fn began_ms(ci: &Ci, build: &crate::Build) -> i64 {
    ci.build_log(&build.job, build.number, 0)
        .ok()
        .and_then(|lines| lines.first().map(|l| l.at_ms))
        .unwrap_or(build.created_ms)
}

/// The executors and the builds since `since_ms`, each with who ran it, when it began and ended.
fn occupancy(ci: &Ci, options: &PanelOptions, since_ms: i64) -> Result<Json, CiError> {
    let runtime = ci.runtime();
    let mut builds = Vec::new();
    let mut workers: BTreeSet<String> =
        (1..=options.executors.max(1)).map(|n| format!("executor-{n}")).collect();
    for build in recent(ci, since_ms, 500)? {
        let worker = runtime.get(&build.run_id).ok().and_then(|record| record.worker);
        if let Some(worker) = &worker {
            workers.insert(worker.clone());
        }
        let begun = (build.status != BuildStatus::Queued).then(|| began_ms(ci, &build));
        builds.push(json!({
            "job": build.job,
            "number": build.number,
            "status": to_json(&build.status),
            "worker": worker,
            "created_ms": build.created_ms,
            "start_ms": begun,
            "end_ms": build.ended_ms,
            "priority": build.priority,
            "reason": build.reason,
        }));
    }
    let busy: BTreeMap<String, usize> = builds
        .iter()
        .filter(|b| b["status"] == "running")
        .filter_map(|b| b["worker"].as_str().map(str::to_owned))
        .fold(BTreeMap::new(), |mut busy, worker| {
            *busy.entry(worker).or_default() += 1;
            busy
        });
    let executors: Vec<Json> = workers
        .iter()
        .map(|name| json!({ "name": name, "kind": "local", "busy": busy.get(name).copied().unwrap_or(0) }))
        .collect();
    Ok(json!({ "now_ms": ci.store().now_ms(), "executors": executors, "builds": builds }))
}

/// The type an attachment was kept with.
fn attachment_mime(ci: &Ci, id: i64) -> Option<String> {
    ci.db().query_row("SELECT mime FROM ci_attachments WHERE id = ?1", [id], |row| row.get(0)).ok()
}

/// The errors of `text` read as `format`, each with its line when it has one.
fn check(ci: &Ci, format: &str, text: &str) -> Result<Vec<Json>, CiError> {
    let reader = ci.format(format)?;
    Ok(match reader.read("verificar", text) {
        Ok(_) => Vec::new(),
        Err(errors) => errors.iter().map(|e| json!({ "line": e.line, "message": e.message })).collect(),
    })
}

/// People and their roles.
fn users(options: &PanelOptions) -> Result<Json, CiError> {
    let file = options
        .users_file
        .as_deref()
        .ok_or_else(|| CiError::unavailable("este servidor não guarda usuários"))?;
    let failed = |e: &dyn std::fmt::Display| CiError::failed(format!("os usuários: {e}"));
    let space = crate::people::users(file)?();
    let named = |records: Vec<ravenrs_app::Object>| -> BTreeMap<String, String> {
        records.iter().map(|r| (key(&r.get("id")), key(&r.get("name")))).collect()
    };
    let people = space.query(SECURITY_USER, &Query::all()).map_err(|e| failed(&e))?;
    let roles = named(space.query(SECURITY_ROLE, &Query::all()).map_err(|e| failed(&e))?);
    let links = space.query(SECURITY_USER_ROLE, &Query::all()).map_err(|e| failed(&e))?;
    let list: Vec<Json> = people
        .iter()
        .map(|person| {
            let own = key(&person.get("id"));
            let theirs: Vec<String> = links
                .iter()
                .filter(|l| key(&l.get("user_id")) == own)
                .filter_map(|l| roles.get(&key(&l.get("role_id"))).cloned())
                .collect();
            json!({ "name": key(&person.get("name")), "roles": theirs })
        })
        .collect();
    Ok(json!(list))
}

/// What people did through the panel, kept with the audit of the plugins.
const ACTIONS: &str = "CREATE TABLE IF NOT EXISTS ci_panel_actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at_ms INTEGER NOT NULL,
    actor TEXT NOT NULL,
    what TEXT NOT NULL
)";

/// Keeps that `actor` did `what`; the audit must not stop what it watches, so a write that fails
/// is left out.
fn record(ci: &Ci, actor: &str, what: &str) {
    let now = ci.store().now_ms();
    let db = ci.store().db();
    if db.execute_batch(ACTIONS).is_ok() {
        let _ = db.execute(
            "INSERT INTO ci_panel_actions (at_ms, actor, what) VALUES (?1, ?2, ?3)",
            rusqlite::params![now, actor, what],
        );
    }
}

/// The audit, the newest first: what plugins asked of the host and, with no `plugin`, what people
/// did through the panel.
fn audit(ci: &Ci, plugin: Option<&str>, limit: usize) -> Result<Json, CiError> {
    let db = ci.store().db();
    db.execute_batch(ACTIONS).map_err(crate::store::db)?;
    let mut lines: Vec<(i64, Json)> = Vec::new();
    {
        let mut stmt = db
            .prepare(
                "SELECT at_ms, plugin, permission, what, granted FROM ci_audit \
                 WHERE ?1 IS NULL OR plugin = ?1 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(crate::store::db)?;
        let rows = stmt
            .query_map(rusqlite::params![plugin, limit as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    json!({
                        "at_ms": row.get::<_, i64>(0)?,
                        "who": row.get::<_, String>(1)?,
                        "kind": "plugin",
                        "permission": row.get::<_, String>(2)?,
                        "what": row.get::<_, String>(3)?,
                        "granted": row.get::<_, bool>(4)?,
                    }),
                ))
            })
            .map_err(crate::store::db)?;
        lines.extend(rows.filter_map(Result::ok));
    }
    if plugin.is_none() {
        let mut stmt = db
            .prepare("SELECT at_ms, actor, what FROM ci_panel_actions ORDER BY id DESC LIMIT ?1")
            .map_err(crate::store::db)?;
        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    json!({
                        "at_ms": row.get::<_, i64>(0)?,
                        "who": row.get::<_, String>(1)?,
                        "kind": "person",
                        "permission": "",
                        "what": row.get::<_, String>(2)?,
                        "granted": true,
                    }),
                ))
            })
            .map_err(crate::store::db)?;
        lines.extend(rows.filter_map(Result::ok));
    }
    lines.sort_by_key(|line| std::cmp::Reverse(line.0));
    lines.truncate(limit);
    Ok(Json::Array(lines.into_iter().map(|(_, line)| line).collect()))
}

/// The type of a file of the panel, by its extension.
fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript",
        "wasm" => "application/wasm",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ttf" => "font/ttf",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// A file of the panel: `index.html` for the folder, never outside it.
fn panel_file(options: &PanelOptions, parts: &[&str]) -> ApiResponse {
    let Some(folder) = &options.files else {
        return refused(404, "este servidor não serve o painel; use raven-ci servir --painel PASTA");
    };
    let wanted: PathBuf = parts.iter().filter(|p| !p.is_empty()).collect();
    if wanted.components().any(|c| !matches!(c, Component::Normal(_))) {
        return refused(400, "caminho inválido");
    }
    let path = if wanted.as_os_str().is_empty() { folder.join("index.html") } else { folder.join(&wanted) };
    let path = if path.is_dir() { path.join("index.html") } else { path };
    match std::fs::read(&path) {
        Ok(bytes) => ApiResponse::raw(200, mime_of(&path), bytes),
        Err(_) => refused(404, format!("{} não está no painel", wanted.display())),
    }
}

/// Watches the core and tells the live channel what changed: `build` when a build's status
/// changes, `line` for each new line of a build that runs, `plugins` when the plugins on change.
/// It stops when dropped.
pub struct LiveWatch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for LiveWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveWatch").finish_non_exhaustive()
    }
}

/// What the watch saw last.
#[derive(Default)]
struct Seen {
    status: BTreeMap<(String, u64), String>,
    lines: BTreeMap<(String, u64), u64>,
    plugins: Vec<String>,
    primed: bool,
}

impl Seen {
    /// Looks once, and hands `publish` what changed since the last look. The first look only
    /// learns where things are, and sends nothing.
    fn look(&mut self, ci: &Ci, publish: &dyn Fn(&str, Json)) {
        let on: Vec<String> = ci.registry().on().iter().map(|p| p.name().to_owned()).collect();
        if self.primed && on != self.plugins {
            publish("plugins", json!({ "on": on }));
        }
        self.plugins = on;
        let since = ci.store().now_ms() - 6 * 60 * 60 * 1000;
        let Ok(builds) = recent(ci, since, 200) else { return };
        for build in builds {
            let key = (build.job.clone(), build.number);
            let status = to_json(&build.status).as_str().unwrap_or_default().to_owned();
            if build.status == BuildStatus::Running || self.lines.contains_key(&key) {
                let after = self.lines.get(&key).copied().unwrap_or(0);
                if let Ok(lines) = ci.build_log(&build.job, build.number, after) {
                    for line in &lines {
                        if self.primed || build.status == BuildStatus::Running {
                            publish(
                                "line",
                                json!({ "job": build.job, "number": build.number, "seq": line.seq,
                                        "step": line.step, "at_ms": line.at_ms, "text": line.text }),
                            );
                        }
                    }
                    if let Some(last) = lines.last() {
                        self.lines.insert(key.clone(), last.seq);
                    } else {
                        self.lines.entry(key.clone()).or_insert(after);
                    }
                }
                if build.status.ended() {
                    self.lines.remove(&key);
                }
            }
            if self.status.get(&key) != Some(&status) {
                if self.primed {
                    publish("build", json!({ "job": build.job, "number": build.number, "status": status }));
                }
                self.status.insert(key, status);
            }
        }
        self.primed = true;
    }
}

impl LiveWatch {
    /// Looks at `ci` every `every` and hands each change to `publish` (kind, data), on a thread
    /// of its own.
    pub fn start(ci: Arc<Ci>, every: Duration, publish: impl Fn(&str, Json) + Send + 'static) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::Builder::new()
            .name("raven-ci-ao-vivo".to_owned())
            .spawn(move || {
                let mut seen = Seen::default();
                while !stopping.load(Ordering::SeqCst) {
                    seen.look(&ci, &publish);
                    let until = std::time::Instant::now() + every;
                    while !stopping.load(Ordering::SeqCst) && std::time::Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(25).min(every));
                    }
                }
            })
            .ok();
        Self { stop, thread }
    }

    /// Looks at `ci` once with a fresh memory, as a test does: every line and status counts as new.
    pub fn look_once(ci: &Ci, publish: &dyn Fn(&str, Json)) {
        let mut seen = Seen { primed: true, ..Seen::default() };
        seen.look(ci, publish);
    }
}

impl Drop for LiveWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Asks for a build of `job` as `actor`, with `parameters`: what the panel's tests and the
/// self-test do without the plugin api-trigger.
pub fn request(
    ci: &Ci,
    job: &str,
    actor: &str,
    parameters: serde_json::Map<String, Json>,
) -> Result<u64, CiError> {
    let mut asked = BuildRequest::new(job, "pelo painel");
    asked.actor = Some(actor.to_owned());
    asked.parameters = parameters;
    Ok(ci.request_build(asked)?.number)
}
