//! The plugin `workspace-cleanup` of Raven CI: room on the disk for every build.
//!
//! # Its role
//!
//! The core gives each build a folder (`espacos/<job>/<number>`) and removes it when the build
//! ends, unless the job's settings say `workspace.keep`. What it does not do is look at the disk:
//! a build that starts with the disk almost full fails halfway through a compilation, with an
//! error that says nothing of why. This plugin refuses such a build before it fetches any code.
//!
//! # How it works
//!
//! It is a [`BuildHook`] whose id is `workspace`, so it reads the same `workspace` object of the
//! job's settings the core reads `keep` from:
//!
//! - **Before every build** it measures the free space of the disk that holds the build's folder
//!   ([`ravenrs_metrics::free_disk_bytes`]). With less than the minimum, the build fails before its
//!   code is fetched, saying "o disco tem X GB livres, menos que os Y GB pedidos".
//! - **The minimum** is `workspace.min_free_gb` of the job when it says one, else `min_free_gb` of
//!   the plugin's configuration, else 5 GB. `0` turns the check off.
//! - **After a build whose folder stays**, it writes the size of the folder to the server's log,
//!   so a job that keeps its folders shows what it costs.
//!
//! # How to use it
//!
//! ```toml
//! # the job's settings
//! [workspace]
//! min_free_gb = 20   # a compilation that needs room
//! keep = false
//! ```

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_plugin_api::{BuildContext, BuildHook, BuildStatus, CiError, CiPlugin, Host, Permission};
use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest, Rule};
use serde_json::Value;

/// The minimum, in GB, when neither the job nor the plugin says one.
pub const DEFAULT_MIN_FREE_GB: f64 = 5.0;

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

/// The plugin `workspace-cleanup`.
#[derive(Default)]
pub struct WorkspacePlugin {
    state: Arc<State>,
}

#[derive(Default)]
struct State {
    host: Mutex<Option<Arc<dyn Host>>>,
}

impl std::fmt::Debug for WorkspacePlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspacePlugin").finish_non_exhaustive()
    }
}

impl WorkspacePlugin {
    /// The plugin, switched off until the core starts it.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Module for WorkspacePlugin {
    fn name(&self) -> &str {
        "workspace-cleanup"
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(env!("CARGO_PKG_VERSION"))
            .unwrap_or_default()
            .capability(Permission::Disk.capability())
    }
}

impl CiPlugin for WorkspacePlugin {
    fn summary(&self) -> String {
        "Recusa o build quando o disco da pasta dele tem menos espaço livre que o pedido.".to_owned()
    }

    fn config(&self) -> Option<EntityInfo> {
        Some(
            EntityInfo::new("WorkspaceSettings")
                .field(FieldInfo::new("min_free_gb", FieldKind::Integer))
                .rule(Rule::range("min_free_gb", Some(0.0), None)),
        )
    }

    fn start(&self, host: Arc<dyn Host>) -> Result<(), CiError> {
        *self.state.host.lock().unwrap_or_else(PoisonError::into_inner) = Some(host);
        Ok(())
    }

    fn stop(&self) {
        self.state.host.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn build_hooks(&self) -> Vec<Arc<dyn BuildHook>> {
        vec![Arc::new(Room(self.state.clone()))]
    }
}

/// The hook `workspace`.
struct Room(Arc<State>);

impl Room {
    fn host(&self) -> Option<Arc<dyn Host>> {
        self.0.host.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The minimum the job asks, or the plugin's, or the default, in GB.
    fn minimum(&self, settings: &Value) -> f64 {
        settings
            .get("min_free_gb")
            .and_then(Value::as_f64)
            .or_else(|| self.host().and_then(|host| host.settings().get("min_free_gb").and_then(Value::as_f64)))
            .unwrap_or(DEFAULT_MIN_FREE_GB)
    }
}

/// `bytes` in GB, with one decimal.
fn in_gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / GB).replace('.', ",")
}

/// The size of everything under `folder`, in bytes.
fn size_of(folder: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(folder) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => size_of(&entry.path()),
            Ok(_) => entry.metadata().map_or(0, |m| m.len()),
            Err(_) => 0,
        })
        .sum()
}

impl BuildHook for Room {
    fn id(&self) -> &str {
        "workspace"
    }

    fn before(&self, build: &BuildContext, settings: &Value) -> Result<(), CiError> {
        let minimum = self.minimum(settings);
        if minimum <= 0.0 {
            return Ok(());
        }
        let Some(free) = ravenrs_metrics::free_disk_bytes(&build.workspace) else {
            return Err(CiError::unavailable(format!(
                "não achei o disco de {}, e por isso não sei se há os {minimum} GB pedidos",
                build.workspace.display()
            )));
        };
        if (free as f64) < minimum * GB {
            return Err(CiError::failed(format!(
                "o disco tem {} GB livres, menos que os {} GB pedidos; libere espaço ou baixe workspace.min_free_gb",
                in_gb(free),
                format!("{minimum}").replace('.', ",")
            )));
        }
        Ok(())
    }

    fn after(&self, build: &BuildContext, status: BuildStatus, settings: &Value) -> Result<(), CiError> {
        let kept = settings.get("keep").and_then(Value::as_bool) == Some(true);
        if kept && let Some(host) = self.host() {
            host.log(&format!(
                "{} #{} ({}) deixou a pasta {} com {} GB",
                build.job,
                build.number,
                status.words(),
                build.workspace.display(),
                in_gb(size_of(&build.workspace))
            ));
        }
        Ok(())
    }
}
