//! The plugin `explain` of Raven CI: why a build failed, in two or three sentences, written by a
//! model of AI that runs on the machine (#942).
//!
//! # Its role
//!
//! Whoever looks at a build that failed, with no session of AI open (a person in the panel, a
//! script), gets the cause and what to change without reading the log. Nothing leaves the machine:
//! the model is local, from `ravenrs-ai`, and what it reads is the summary of the build (#941),
//! with the secrets already hidden.
//!
//! # How it works
//!
//! - **Off from the start** ([`CiPlugin::on_from_the_start`]): it needs the program of the model
//!   and a model downloaded, which a server does not have until someone installs them.
//! - **The model never runs in the server's process.** A model takes from 1 to 5 GB, and the
//!   service of the RPM holds the server to 256 MB. The plugin hands the summary to the program
//!   `raven-ci-explicador` ([`explainer`]) and reads its answer:
//!   - through the socket of systemd `socket` (`/run/raven-ci/explicador.sock` unless said), when
//!     it is there: the RPM's `raven-ci-explicador.socket` starts, for each request, an instance of
//!     its own with its own limit of memory and no network, which ends with the answer;
//!   - otherwise, the program `programa` (`raven-ci-explicador` beside the server unless said), run
//!     as a child with the summary on its input, as on a desktop or in a test.
//! - **`POST /api/ci/build/explain?job=&number=`** (Executores and Administradores) explains the
//!   build, or gives the explanation kept before, without running the model again. It refuses a
//!   build that did not fail. **`GET`** on the same path gives the one kept, or 404.
//! - **What it keeps**: the attachment `explicacao.md`, of kind [`KIND`], with the build, its
//!   summary naming the model and how long it took.
//!
//! # Settings
//!
//! | Setting | Default | What it does |
//! |---|---|---|
//! | `modelo` | `qwen3-1.7b-q4` | the model of `ravenrs-ai` that writes it |
//! | `socket` | `/run/raven-ci/explicador.sock` | the socket of systemd that starts the program |
//! | `programa` | `raven-ci-explicador` beside the server | the program, when there is no socket |
//! | `pasta` | `modelos` in the plugin's folder | where the program finds the models, when run as a child |
//! | `prazo_s` | 300 | how long it may take |

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raven_ci_plugin_api::{
    Attachment, CiError, CiPlugin, Host, HttpRequest, HttpResponse, Permission, Route,
};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use serde_json::{Value, json};

pub mod explainer;

/// The kind of the attachment it keeps.
pub const KIND: &str = "explicacao";
/// The name of the attachment.
pub const NAME: &str = "explicacao.md";
/// The path of its routes.
pub const PATH: &str = "/api/ci/build/explain";

type Shared = Arc<Mutex<Option<Arc<dyn Host>>>>;

/// The plugin `explain`.
#[derive(Default)]
pub struct ExplainPlugin {
    host: Shared,
}

impl std::fmt::Debug for ExplainPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExplainPlugin").finish_non_exhaustive()
    }
}

impl ExplainPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for ExplainPlugin {
    fn name(&self) -> &str {
        "explain"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Process.capability())
            .capability(Permission::Disk.capability())
    }
}

impl CiPlugin for ExplainPlugin {
    fn summary(&self) -> String {
        "Explica por que um build falhou, com uma IA local, fora do processo do servidor.".to_owned()
    }

    fn on_from_the_start(&self) -> bool {
        false
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(
            EntityInfo::new("ExplainSettings")
                .field(FieldInfo::new("modelo", FieldKind::Text))
                .field(FieldInfo::new("socket", FieldKind::Text))
                .field(FieldInfo::new("programa", FieldKind::Text))
                .field(FieldInfo::new("pasta", FieldKind::Text))
                .field(FieldInfo::new("prazo_s", FieldKind::Integer)),
        )
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        host.require(Permission::Process, "rodar o programa que explica as falhas")?;
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn routes(&self) -> Vec<Route> {
        let (asked, read) = (self.host.clone(), self.host.clone());
        vec![
            Route::new("POST", PATH, move |request| answer(&asked, request, true)),
            Route::new("GET", PATH, move |request| answer(&read, request, false)),
        ]
    }
}

fn refused(status: u16, message: impl Into<String>) -> HttpResponse {
    HttpResponse::json(status, &json!({ "error": "explain", "message": message.into() }))
}

/// The answer to `request`: the explanation kept, or, when `make`, one made now and kept.
fn answer(shared: &Shared, request: &HttpRequest, make: bool) -> HttpResponse {
    let Some(host) = shared.lock().unwrap_or_else(PoisonError::into_inner).clone() else {
        return refused(503, "o plugin explain está desligado: ligue-o em Plugins, ou com raven-ci plugins ligar explain");
    };
    if make && !request.may_run() {
        return refused(403, "só Executores e Administradores pedem a explicação de um build");
    }
    let Some(job) = request.query.get("job").filter(|job| !job.is_empty()) else {
        return refused(400, "falta job na consulta");
    };
    let Some(number) = request.query.get("number").and_then(|n| n.parse::<u64>().ok()) else {
        return refused(400, "falta number na consulta, um número");
    };
    match explanation(host.as_ref(), job, number, make) {
        Ok(Some(body)) => HttpResponse::json(200, &body),
        Ok(None) => refused(404, format!("o build {job} #{number} ainda não tem explicação: peça com POST")),
        Err(CiError::Settings { reason, .. }) => refused(400, reason),
        Err(CiError::Unavailable(message)) => refused(404, message),
        Err(e) => refused(500, e.to_string()),
    }
}

/// The explanation kept with the build, or, with `make`, one made now and kept; `None` when there
/// is none and it is not to be made.
pub fn explanation(host: &dyn Host, job: &str, number: u64, make: bool) -> Result<Option<Value>, CiError> {
    if let Some(kept) = host.attachments(job, Some(number), KIND)?.into_iter().next() {
        let (info, bytes) = host.attachment(kept.id)?;
        return Ok(Some(json!({
            "job": job,
            "number": number,
            "texto": String::from_utf8_lossy(&bytes),
            "modelo": info.summary.get("modelo"),
            "ms": info.summary.get("ms"),
            "guardada": true,
        })));
    }
    if !make {
        return Ok(None);
    }
    let summary = host.build_summary(job, number)?;
    let status = summary.get("status").and_then(Value::as_str).unwrap_or_default();
    if status != "failed" {
        return Err(CiError::settings(
            "number",
            format!("o build {job} #{number} não falhou ({status}): não há falha para explicar"),
        ));
    }
    let settings = host.settings();
    let made = explainer::explain(host, &settings, &summary)?;
    let text = format!("{}\n", made.text.trim());
    let attachment = Attachment::new(KIND, NAME, "text/markdown", text.clone().into_bytes())
        .summary(json!({ "modelo": made.model, "ms": made.ms }));
    host.attach(job, number, &attachment)?;
    Ok(Some(json!({
        "job": job,
        "number": number,
        "texto": text,
        "modelo": made.model,
        "ms": made.ms,
        "guardada": false,
    })))
}

/// How long the model may take unless the settings say.
pub(crate) fn deadline(settings: &Value) -> Duration {
    Duration::from_secs(settings.get("prazo_s").and_then(Value::as_u64).unwrap_or(300).max(1))
}
