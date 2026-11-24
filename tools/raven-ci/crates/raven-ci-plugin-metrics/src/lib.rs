//! The plugin `metrics` of Raven CI: `/metrics` in the Prometheus format, for Prometheus to read
//! and Grafana to show.
//!
//! # Its role
//!
//! Whoever cares for the server should see from afar whether it is healthy (the queue, the builds
//! failing, the agents gone) and how much memory it holds, the number that says it costs a part
//! of what Jenkins does. This plugin is the route that says it.
//!
//! # How it works
//!
//! - **Each read** asks the core for a picture of the server ([`Host::snapshot`]) and writes it
//!   with `ravenrs-metrics` in the Prometheus text format (0.0.4). Nothing is kept between reads.
//! - **The metrics.** Most are gauges, read at the moment. The counts of builds come two ways:
//!   `raven_ci_builds` counts the builds the core still keeps, so it goes down when the retention
//!   lets some go; `raven_ci_builds_total` and `raven_ci_build_seconds_total` count every build
//!   that ended since `raven_ci_totals_since_seconds`, never go down, and are the ones for
//!   `rate()` and `increase()` (#970). A job deleted keeps its totals under the name it had.
//!
//! | Metric | Labels | What it is |
//! |---|---|---|
//! | `raven_ci_queue_builds` | | builds waiting |
//! | `raven_ci_running_builds` | | builds running |
//! | `raven_ci_builds` | `status` | builds kept, by how they ended |
//! | `raven_ci_builds_total` | `job`, `status` | counter: builds that ended, since the counting began |
//! | `raven_ci_build_seconds_total` | `job`, `status` | counter: the seconds those builds took |
//! | `raven_ci_totals_since_seconds` | | when the counting began, in seconds since 1970 |
//! | `raven_ci_last_build_duration_seconds` | `job` | how long the last build of each job took |
//! | `raven_ci_last_build_status` | `job`, `status` | 1 for how the last build of each job is |
//! | `raven_ci_agents` | `state` | agents `connected` and `disconnected` |
//! | `raven_ci_agent_executors` | `agent` | the executors of each agent |
//! | `raven_ci_plugins` | `state` | plugins `on`, `off` and `refused` |
//! | `raven_ci_plugin_memory_bytes` | `plugin`, `origin` | the memory of each external plugin running |
//! | `raven_ci_server_memory_bytes` | | the memory of the server's process |
//!
//! - **Who reads**: `GET /api/ci/metrics` for someone signed in, as every reading route of the
//!   server. With `open = true` in the plugin's settings, `GET /hooks/metrics` answers too, with no
//!   sign-in, for a Prometheus that keeps no token; without it, that path answers 404.
//! - **A core that gives no picture** answers 503, saying so.
//!
//! # How to use it
//!
//! ```yaml
//! # prometheus.yml
//! scrape_configs:
//!   - job_name: raven-ci
//!     metrics_path: /hooks/metrics
//!     static_configs:
//!       - targets: ["ci.local:8080"]
//! ```

use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{CiError, CiPlugin, CiSnapshot, Host, HttpRequest, HttpResponse, Route};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest};
use ravenrs_metrics::{MetricError, Registry};
use serde_json::Value;

/// The path for someone signed in.
pub const PATH: &str = "/api/ci/metrics";
/// The path with no sign-in, when the settings say `open`.
pub const OPEN_PATH: &str = "/hooks/metrics";
/// The type of the answer, as Prometheus expects it.
const CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

type Shared = Arc<Mutex<Option<Arc<dyn Host>>>>;

/// The plugin `metrics`.
#[derive(Default)]
pub struct MetricsPlugin {
    host: Shared,
}

impl std::fmt::Debug for MetricsPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsPlugin").finish_non_exhaustive()
    }
}

impl MetricsPlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for MetricsPlugin {
    fn name(&self) -> &str {
        "metrics"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default()
    }
}

impl CiPlugin for MetricsPlugin {
    fn summary(&self) -> String {
        "O /metrics do servidor no formato Prometheus: fila, builds, agentes, plugins e memória.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(EntityInfo::new("MetricsSettings").field(FieldInfo::new("open", FieldKind::Boolean)))
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        *self.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn routes(&self) -> Vec<Route> {
        let (signed, open) = (self.host.clone(), self.host.clone());
        vec![
            Route::new("GET", PATH, move |_: &HttpRequest| answer(&signed)),
            Route::new("GET", OPEN_PATH, move |_: &HttpRequest| {
                let allowed = lock(&open).as_ref().is_some_and(|host| {
                    host.settings().get("open").and_then(Value::as_bool).unwrap_or(false)
                });
                if allowed {
                    answer(&open)
                } else {
                    HttpResponse::text(
                        404,
                        "o /metrics sem login está fechado: ponha open = true no plugin metrics",
                    )
                }
            })
            .public(),
        ]
    }
}

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, Option<Arc<dyn Host>>> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

fn answer(shared: &Shared) -> HttpResponse {
    let Some(host) = lock(shared).clone() else {
        return HttpResponse::text(503, "o plugin metrics está desligado");
    };
    let snapshot = match host.snapshot() {
        Ok(snapshot) => snapshot,
        Err(e) => return HttpResponse::text(503, format!("sem o retrato do servidor: {e}")),
    };
    match render(&snapshot) {
        Ok(text) => {
            HttpResponse { status: 200, content_type: CONTENT_TYPE.to_owned(), body: text.into_bytes() }
        }
        Err(e) => HttpResponse::text(500, format!("as métricas não saíram: {e}")),
    }
}

fn count(n: u64) -> f64 {
    // Counts of builds and agents stay far below 2^53, where f64 holds every integer.
    #[allow(clippy::cast_precision_loss, reason = "counts far below 2^53")]
    let value = n as f64;
    value
}

/// `snapshot` in the Prometheus text format.
pub fn render(snapshot: &CiSnapshot) -> Result<String, MetricError> {
    let registry = Registry::new();
    registry
        .gauge("raven_ci_queue_builds", "Builds waiting in the queue.", &[])?
        .with(&[])?
        .set(count(snapshot.queued));
    registry
        .gauge("raven_ci_running_builds", "Builds running now.", &[])?
        .with(&[])?
        .set(count(snapshot.running));
    let builds = registry.gauge("raven_ci_builds", "Builds kept, by how they are.", &["status"])?;
    for (status, n) in &snapshot.builds {
        let label =
            serde_json::to_value(status).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        builds.with(&[&label])?.set(count(*n));
    }
    let totals = registry.counter(
        "raven_ci_builds_total",
        "Builds that ended, by job and by how, since the counting began.",
        &["job", "status"],
    )?;
    let seconds = registry.counter(
        "raven_ci_build_seconds_total",
        "The seconds the builds that ended took, by job and by how.",
        &["job", "status"],
    )?;
    for total in &snapshot.totals {
        let label = serde_json::to_value(total.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        totals.with(&[&total.job, &label])?.add(count(total.builds));
        seconds.with(&[&total.job, &label])?.add(total.seconds.max(0.0));
    }
    let since = registry.gauge(
        "raven_ci_totals_since_seconds",
        "When the counting of the totals began, in seconds since 1970.",
        &[],
    )?;
    if let Some(ms) = snapshot.totals_since_ms {
        #[allow(clippy::cast_precision_loss, reason = "milliseconds since 1970, far below 2^53")]
        since.with(&[])?.set(ms as f64 / 1000.0);
    }
    let duration = registry.gauge(
        "raven_ci_last_build_duration_seconds",
        "How long the last build of each job took, or has taken so far.",
        &["job"],
    )?;
    let last = registry.gauge(
        "raven_ci_last_build_status",
        "1 for how the last build of each job is.",
        &["job", "status"],
    )?;
    for job in &snapshot.jobs {
        duration.with(&[&job.job])?.set(count(job.duration_ms) / 1000.0);
        let label = serde_json::to_value(job.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        last.with(&[&job.job, &label])?.set(1.0);
    }
    let agents = registry.gauge("raven_ci_agents", "Agents, connected or not.", &["state"])?;
    let connected = snapshot.agents.iter().filter(|a| a.connected).count();
    agents.with(&["connected"])?.set(count(u64::try_from(connected).unwrap_or(0)));
    agents.with(&["disconnected"])?.set(count(u64::try_from(snapshot.agents.len() - connected).unwrap_or(0)));
    let executors = registry.gauge("raven_ci_agent_executors", "The executors of each agent.", &["agent"])?;
    for agent in &snapshot.agents {
        executors.with(&[&agent.name])?.set(f64::from(agent.executors));
    }
    let plugins = registry.gauge("raven_ci_plugins", "Plugins, by their state.", &["state"])?;
    for state in ["on", "off", "refused"] {
        let n = snapshot.plugins.iter().filter(|p| p.state == state).count();
        plugins.with(&[state])?.set(count(u64::try_from(n).unwrap_or(0)));
    }
    let memory = registry.gauge(
        "raven_ci_plugin_memory_bytes",
        "The memory of each external plugin whose program runs.",
        &["plugin", "origin"],
    )?;
    for plugin in &snapshot.plugins {
        if let Some(bytes) = plugin.memory_bytes {
            memory.with(&[&plugin.name, &plugin.origin])?.set(count(bytes));
        }
    }
    let server =
        registry.gauge("raven_ci_server_memory_bytes", "The memory of the server's process.", &[])?;
    if let Some(bytes) = snapshot.server_memory_bytes {
        server.with(&[])?.set(count(bytes));
    }
    Ok(registry.render())
}
