//! The limits of an agent. (A-04 fills them in: cores, memory and disk.)

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::site::AgentSite;

/// The limits of an agent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Limits {}

impl Limits {
    /// What `args` say.
    pub fn from_args(_args: &[String]) -> Result<Self, String> {
        Ok(Self {})
    }

    /// How many executors it may have, of the `asked`.
    pub fn executors(&self, asked: u32) -> u32 {
        asked
    }

    /// Puts the limits on the agent's own process; what to say of them.
    pub fn apply(&self, _name: &str) -> Vec<String> {
        Vec::new()
    }

    /// The limits and their use, for the hello.
    pub fn report(&self, _dir: &Path) -> Value {
        Value::Null
    }

    /// Whether there is room to take a build.
    pub fn room(&self, _dir: &Path) -> bool {
        true
    }

    /// The variables the steps get.
    pub fn environment(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// Watches the build of `site` in `workspace` while the result is held.
    pub fn watch(&self, _workspace: &Path, _site: Arc<AgentSite>) -> Watch {
        Watch
    }
}

/// A build watched; dropped, the watch stops.
#[derive(Debug)]
pub struct Watch;

impl Drop for Watch {
    fn drop(&mut self) {}
}

/// The memory the agent holds now, in bytes, when the system says.
pub fn memory_now() -> Option<u64> {
    ravenrs_metrics::process_rss_bytes().or_else(|| ravenrs_metrics::rss_bytes_of(std::process::id()))
}
