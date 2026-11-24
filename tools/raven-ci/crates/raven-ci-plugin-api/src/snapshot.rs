//! A picture of the server at one moment, for a plugin that reports on it: [`CiSnapshot`].

use serde::{Deserialize, Serialize};

use crate::BuildStatus;

/// The server at one moment: its queue, its builds, its agents, its plugins and its memory.
///
/// **Its role.** A plugin sees no queue and no builds; this is the one read of them it gets,
/// through [`Host::snapshot`](crate::Host::snapshot), for what reports on the server as a whole
/// (the `/metrics` of the plugin `metrics`).
///
/// **How it works.** The core makes it when asked, from what it keeps. The counts in `builds` are
/// of the builds still kept, so they go down when the retention removes some: read them as gauges.
/// The `totals` are of every build that ended since `totals_since_ms`, kept by the core apart from
/// the builds, so the retention never takes them down: read them as counters (#970).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CiSnapshot {
    /// Builds waiting in the queue.
    pub queued: u64,
    /// Builds running now.
    pub running: u64,
    /// The builds kept, counted by how they are: `(status, count)`.
    pub builds: Vec<(BuildStatus, u64)>,
    /// The last build of each job.
    pub jobs: Vec<JobSnapshot>,
    /// The agents the server knows.
    pub agents: Vec<AgentSnapshot>,
    /// The plugins, on or not.
    pub plugins: Vec<PluginSnapshot>,
    /// The memory the server's process holds, when the system says.
    pub server_memory_bytes: Option<u64>,
    /// Every build that ended, by job and by how it ended, since `totals_since_ms`; the
    /// retention does not take them down. Empty from a core older than #970.
    #[serde(default)]
    pub totals: Vec<BuildTotal>,
    /// When the core began counting `totals`, in milliseconds since 1970.
    #[serde(default)]
    pub totals_since_ms: Option<i64>,
}

/// The builds of one job that ended one way, since the core began counting (#970).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildTotal {
    /// The job, by the path it had when its builds ended; a job deleted keeps its totals.
    pub job: String,
    /// How they ended.
    pub status: BuildStatus,
    /// How many.
    pub builds: u64,
    /// The sum of how long they took, in seconds.
    pub seconds: f64,
}

/// The last build of one job.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JobSnapshot {
    /// The job, by its path.
    pub job: String,
    /// Its last build's number.
    pub number: u64,
    /// How it is.
    pub status: BuildStatus,
    /// How long it took, or has taken so far.
    pub duration_ms: u64,
}

/// One agent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    /// Its name.
    pub name: String,
    /// Whether it is connected now.
    pub connected: bool,
    /// How many builds it runs at once.
    pub executors: u32,
}

/// One plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSnapshot {
    /// Its name.
    pub name: String,
    /// `builtin` (compiled in, sharing the server's memory) or `external` (a program of its own).
    pub origin: String,
    /// `on`, `off` or `refused`.
    pub state: String,
    /// The memory of its program, for an external plugin running on a system that says.
    pub memory_bytes: Option<u64>,
}
