//! The picture of the server a plugin reads with `Host::snapshot` (#913): the queue, the builds
//! kept, the last build of each job, the agents, the plugins and the memory.
//!
//! # Its place
//!
//! It reads the core through its public methods (`jobs`, `builds`, `queue`, `agents`, the
//! register) and names no module above it, so the API, the panel and the hosts can all use it.

use std::collections::BTreeMap;

use raven_ci_plugin_api::{
    AgentSnapshot, BuildStatus, BuildTotal, CiError, CiSnapshot, JobSnapshot, PluginSnapshot,
};

use crate::{Ci, Memory, Origin, PluginState};

impl Ci {
    /// The server at this moment. See [`CiSnapshot`].
    pub fn snapshot(&self) -> Result<CiSnapshot, CiError> {
        let now = self.store().now_ms();
        let mut counts: BTreeMap<String, (BuildStatus, u64)> = BTreeMap::new();
        let mut jobs = Vec::new();
        let mut running = 0;
        for job in self.jobs()? {
            let builds = self.builds(&job.name)?;
            for build in &builds {
                let entry = counts.entry(build.status.words().to_string()).or_insert((build.status, 0));
                entry.1 += 1;
                if build.status == BuildStatus::Running {
                    running += 1;
                }
            }
            // The newest first.
            if let Some(last) = builds.first() {
                let took = last.ended_ms.unwrap_or(now).saturating_sub(last.created_ms).max(0);
                jobs.push(JobSnapshot {
                    job: job.name.clone(),
                    number: last.number,
                    status: last.status,
                    duration_ms: u64::try_from(took).unwrap_or(0),
                });
            }
        }
        let agents = self
            .agents()?
            .into_iter()
            .map(|agent| AgentSnapshot {
                name: agent.name,
                connected: agent.connected,
                executors: agent.executors,
            })
            .collect();
        let plugins = self
            .registry()
            .list()?
            .into_iter()
            .map(|view| PluginSnapshot {
                name: view.name,
                origin: match view.origin {
                    Origin::Embedded => "builtin",
                    Origin::External(_) => "external",
                }
                .to_owned(),
                state: match view.state {
                    PluginState::On => "on",
                    PluginState::Off => "off",
                    PluginState::Refused(_) => "refused",
                }
                .to_owned(),
                memory_bytes: match view.memory {
                    Memory::Bytes(bytes) => Some(bytes),
                    _ => None,
                },
            })
            .collect();
        let (kept, totals_since_ms) = crate::model::totals(&self.db())?;
        let totals = kept
            .into_iter()
            .filter_map(|(job, status, builds, seconds)| {
                let status: BuildStatus = serde_json::from_value(serde_json::Value::String(status)).ok()?;
                Some(BuildTotal { job, status, builds, seconds })
            })
            .collect();
        Ok(CiSnapshot {
            queued: u64::try_from(self.queue()?.len()).unwrap_or(0),
            running,
            builds: counts.into_values().collect(),
            jobs,
            agents,
            plugins,
            server_memory_bytes: ravenrs_metrics::rss_bytes_of(std::process::id()),
            totals,
            totals_since_ms,
        })
    }
}
