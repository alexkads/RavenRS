//! The plugin `api-trigger` of Raven CI: builds asked for over HTTP.
//!
//! # Its role
//!
//! A program outside Raven CI (a command line, another service, a session of an AI) asks for a
//! build of a job with its parameters and gets its number, as the parameterized pipeline of an
//! Azure DevOps repository is started today.
//!
//! # How it works
//!
//! - **Only the jobs that ask for it.** It is a [`Trigger`] whose id is `api`: a job whose settings
//!   say `{"triggers": [{"id": "api"}]}` is watched, and only a watched job takes requests.
//! - **The route** `POST /api/ci/run` takes `{"job": "apps/site", "parameters": {...},
//!   "priority": 0}` and answers 201 with `{"job", "number"}`. A request equal to one still in the
//!   queue joins it and gets its number. The job goes in the body since its name has `/` in it.
//! - **Who.** Someone signed in who may run builds ([`HttpRequest::may_run`]): an administrator or
//!   someone with the role `Executores`; anyone else gets 403. The build keeps who asked.
//! - **What goes wrong** says why: 404 for a job that does not take requests, 400 for a body or a
//!   parameter that does not fit.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{
    BuildRequest, Builds, CiError, CiPlugin, HttpRequest, HttpResponse, Route, Trigger,
};
use ravenrs_app::{Module, ModuleManifest};
use serde_json::{Map, Value, json};

/// The path of the route.
pub const RUN_PATH: &str = "/api/ci/run";

/// The jobs watched, each with what it asks for builds with.
type Watched = Arc<Mutex<HashMap<String, Arc<dyn Builds>>>>;

/// The plugin `api-trigger`.
#[derive(Default)]
pub struct ApiTriggerPlugin {
    watched: Watched,
}

impl std::fmt::Debug for ApiTriggerPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiTriggerPlugin").finish_non_exhaustive()
    }
}

impl ApiTriggerPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for ApiTriggerPlugin {
    fn name(&self) -> &str {
        "api-trigger"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default()
    }
}

impl CiPlugin for ApiTriggerPlugin {
    fn summary(&self) -> String {
        "Pede builds pela API: POST /api/ci/run, com parâmetros, para quem pode rodar.".to_owned()
    }

    fn stop(&self) {
        self.watched.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    fn triggers(&self) -> Vec<Arc<dyn Trigger>> {
        vec![Arc::new(Api(self.watched.clone()))]
    }

    fn routes(&self) -> Vec<Route> {
        let watched = self.watched.clone();
        vec![Route::new("POST", RUN_PATH, move |request| run(&watched, request))]
    }
}

/// The trigger `api`.
struct Api(Watched);

impl Trigger for Api {
    fn id(&self) -> &str {
        "api"
    }

    fn watch(&self, job: &str, _settings: &Value, builds: Arc<dyn Builds>) -> Result<(), CiError> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).insert(job.to_owned(), builds);
        Ok(())
    }

    fn unwatch(&self, job: &str) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(job);
    }
}

fn refuse(status: u16, message: impl Into<String>) -> HttpResponse {
    HttpResponse::json(status, &json!({ "error": "ci", "message": message.into() }))
}

/// Answers `POST /api/ci/run`.
fn run(watched: &Watched, request: &HttpRequest) -> HttpResponse {
    if !request.may_run() {
        return refuse(403, "só quem tem o papel Executores, ou é administrador, pede builds");
    }
    let Ok(Value::Object(body)) = serde_json::from_slice::<Value>(&request.body) else {
        return refuse(400, r#"o corpo é um objeto JSON: {"job": "apps/site", "parameters": {}}"#);
    };
    let Some(job) = body.get("job").and_then(Value::as_str) else {
        return refuse(400, "falta o job");
    };
    let parameters = match body.get("parameters") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(parameters)) => parameters.clone(),
        Some(_) => return refuse(400, "parameters é um objeto, com um valor por parâmetro"),
    };
    let priority = body.get("priority").and_then(Value::as_i64).unwrap_or(0);
    let builds = watched.lock().unwrap_or_else(PoisonError::into_inner).get(job).cloned();
    let Some(builds) = builds else {
        return refuse(
            404,
            format!("o job {job} não aceita pedidos pela API: falta {{\"id\": \"api\"}} nos gatilhos dele"),
        );
    };
    let mut asked = BuildRequest::new(job, "pedido pela API").priority(i32::try_from(priority).unwrap_or(0));
    asked.parameters = parameters;
    if let Some(actor) = &request.actor {
        asked = asked.by(actor.clone());
    }
    match builds.request(asked) {
        Ok(number) => HttpResponse::json(201, &json!({ "job": job, "number": number })),
        Err(CiError::Settings { field, reason }) => refuse(400, format!("{field}: {reason}")),
        Err(e @ CiError::Unavailable(_)) => refuse(404, e.to_string()),
        Err(e) => refuse(400, e.to_string()),
    }
}
