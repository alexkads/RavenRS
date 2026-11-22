//! The plugin `junit` of Raven CI: the results of the tests of each build, and how they go over
//! the builds of a job.
//!
//! # Its role
//!
//! The tests of a build say what passed as JUnit XML (Maven Surefire, pytest, `cargo nextest`,
//! `cargo ravenrs e2e --format junit`). This [`Publisher`] reads those reports before the folder
//! of the build goes, keeps the count with the build and gives the trend of a job, as the JUnit
//! plugin of Jenkins does, so what broke is seen without opening the log.
//!
//! # How it works
//!
//! - **The reports** are found by `settings.paths` (`junit = ["**/junit.xml"]` in a
//!   `.raven-ci.toml`) with the search of the plugin `artifacts` ([`raven_ci_plugin_artifacts::find`]):
//!   the same patterns and the same refusals of what leaves the folder.
//! - **Each file** is read with [`ravenrs_junit::read`]. One that cannot be read goes to the log
//!   of the build with its name and why, and the others still count.
//! - **One attachment** of the kind `junit` per build, `testes.json`: every suite and case read,
//!   as JSON. Its summary holds the count (`tests`, `failures`, `errors`, `skipped`, `time_ms`),
//!   the `files` read and, in `failed`, the full name of each test that failed or errored (at most
//!   [`MOST_NAMED`]).
//! - **What finds no report** fails the publisher (it goes to the log), unless
//!   `allow_empty = true`. A failing test does not change the status of the build: the steps
//!   that ran the tests already did.
//! - **The trend**: `GET /api/ci/tests?job=` gives one item per build of the job, the newest first,
//!   with its number and its summary; `limit` (50 by default) says how many.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{
    Attachment, BuildInfo, BuildScope, CiError, CiPlugin, Host, HttpRequest, HttpResponse, Publisher, Route,
};
use raven_ci_plugin_artifacts::{find, patterns};
use ravenrs_app::{Module, ModuleManifest};
use ravenrs_junit::Suites;
use serde_json::{Value, json};

/// The kind of the attachment it keeps.
pub const KIND: &str = "junit";
/// The name of the attachment.
pub const NAME: &str = "testes.json";
/// How many names of tests that failed the summary keeps.
pub const MOST_NAMED: usize = 200;
/// How many builds the trend gives unless asked.
pub const TREND: usize = 50;

type Shared = Arc<Mutex<Option<Arc<dyn Host>>>>;

/// The plugin `junit`.
#[derive(Default)]
pub struct JunitPlugin {
    host: Shared,
}

impl std::fmt::Debug for JunitPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JunitPlugin").finish_non_exhaustive()
    }
}

impl JunitPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for JunitPlugin {
    fn name(&self) -> &str {
        "junit"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default()
    }
}

impl CiPlugin for JunitPlugin {
    fn summary(&self) -> String {
        "Lê o JUnit dos testes, guarda o placar e os que falharam, e dá a tendência do job pela API."
            .to_owned()
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn publishers(&self) -> Vec<Arc<dyn Publisher>> {
        vec![Arc::new(Junit)]
    }

    fn routes(&self) -> Vec<Route> {
        let host = self.host.clone();
        vec![Route::new("GET", "/api/ci/tests", move |request| trend(&host, request))]
    }
}

/// The publisher `junit`. See the [crate](crate).
struct Junit;

impl Publisher for Junit {
    fn id(&self) -> &str {
        "junit"
    }

    fn publish(
        &self,
        build: &BuildInfo,
        settings: &Value,
        workspace: &Path,
    ) -> Result<Vec<Attachment>, CiError> {
        let scope = BuildScope::of_build(&build.job, build.number);
        let log = |line: &str| {
            if let Some(scope) = &scope {
                scope.log("junit", line);
            }
        };
        let wanted = patterns(settings);
        if wanted.is_empty() {
            return Err(CiError::settings("paths", "diga onde estão os XML: paths = [\"**/junit.xml\"]"));
        }
        let mut all = Suites::default();
        let mut files = 0;
        let found = find(workspace, &wanted, &log);
        for file in &found {
            let read = std::fs::read_to_string(&file.path)
                .map_err(|e| e.to_string())
                .and_then(|text| ravenrs_junit::read(&text).map_err(|e| e.to_string()));
            match read {
                Ok(suites) => {
                    files += 1;
                    all.suites.extend(suites.suites);
                }
                Err(why) => log(&format!("{}: {why}", file.name)),
            }
        }
        if files == 0 {
            if settings.get("allow_empty").and_then(Value::as_bool) == Some(true) {
                return Ok(Vec::new());
            }
            let why =
                if found.is_empty() { "nenhum relatório achado" } else { "nenhum relatório legível" };
            return Err(CiError::failed(format!("{why} em {}", wanted.join(", "))));
        }
        let summary = all.summary();
        log(&format!(
            "{} testes: {} passaram, {} falharam, {} com erro, {} pulados",
            summary.tests,
            summary.passed(),
            summary.failures,
            summary.errors,
            summary.skipped
        ));
        let failed: Vec<&String> = summary.failed.iter().take(MOST_NAMED).collect();
        let bytes = serde_json::to_vec(&all).map_err(|e| CiError::failed(e.to_string()))?;
        Ok(vec![Attachment::new(KIND, NAME, "application/json", bytes).summary(json!({
            "tests": summary.tests,
            "failures": summary.failures,
            "errors": summary.errors,
            "skipped": summary.skipped,
            "time_ms": summary.time_ms,
            "files": files,
            "failed": failed,
        }))])
    }
}

fn refuse(status: u16, message: impl Into<String>) -> HttpResponse {
    HttpResponse::json(status, &json!({ "error": "ci", "message": message.into() }))
}

/// `GET /api/ci/tests?job=[&limit=]`.
fn trend(host: &Shared, request: &HttpRequest) -> HttpResponse {
    let Some(host) = host.lock().unwrap_or_else(PoisonError::into_inner).clone() else {
        return refuse(404, "o plugin junit não está ligado");
    };
    let Some(job) = request.query.get("job") else {
        return refuse(400, "job: falta na consulta");
    };
    let limit = request.query.get("limit").and_then(|l| l.parse().ok()).unwrap_or(TREND);
    match host.attachments(job, None, KIND) {
        Ok(list) => {
            let items: Vec<Value> = list
                .into_iter()
                .take(limit)
                .map(|a| {
                    let mut item = json!({ "number": a.number, "id": a.id });
                    if let (Value::Object(item), Value::Object(summary)) = (&mut item, a.summary) {
                        item.extend(summary);
                    }
                    item
                })
                .collect();
            HttpResponse::json(200, &Value::Array(items))
        }
        Err(e @ CiError::Unavailable(_)) => refuse(404, e.to_string()),
        Err(e) => refuse(500, e.to_string()),
    }
}
