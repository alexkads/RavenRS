//! The plugins of an agent: every one it brings is known and switched off, and a build switches on
//! only the ones it needs, with the settings the server has for them. A plugin goes off again when
//! no build that runs needs it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use raven_ci_core::agents::PluginUse;
use raven_ci_core::{Ci, PluginState};
use raven_ci_plugin_api::{CiError, CiPlugin};
use serde_json::Value;

/// Every plugin an agent brings compiled in: those of steps, of fetching code, of hooks and the
/// publishers. A build with `artifacts`, `junit`, a step `cargo`, a `container` or a `kubectl` asks
/// the agent for their plugin, and the agent of 0.1.3, with only the first four, refused it
/// ("este agente não tem o plugin artifacts").
pub fn builtin() -> Vec<Arc<dyn CiPlugin>> {
    vec![
        Arc::new(raven_ci_plugin_shell::ShellPlugin::new()),
        Arc::new(raven_ci_plugin_git::GitPlugin::new()),
        Arc::new(raven_ci_plugin_lock::LockPlugin::new()),
        Arc::new(raven_ci_plugin_workspace::WorkspacePlugin::new()),
        Arc::new(raven_ci_plugin_artifacts::ArtifactsPlugin::new()),
        Arc::new(raven_ci_plugin_junit::JunitPlugin::new()),
        Arc::new(raven_ci_plugin_cargo::CargoPlugin::new()),
        Arc::new(raven_ci_plugin_docker::DockerPlugin::new()),
        Arc::new(raven_ci_plugin_kubernetes::KubernetesPlugin::new()),
    ]
}

/// Which plugins the builds that run need, and how many of them.
#[derive(Debug)]
pub struct Plugins {
    ci: Arc<Ci>,
    users: Mutex<HashMap<String, usize>>,
}

impl Plugins {
    /// Over `ci`, with every plugin it knows switched off.
    pub fn new(ci: Arc<Ci>) -> Result<Self, CiError> {
        for view in ci.registry().list()? {
            if view.enabled {
                ci.disable_plugin(&view.name)?;
            }
        }
        Ok(Self { ci, users: Mutex::default() })
    }

    /// Switches on what a build needs; refused, saying which, when the agent does not have one or
    /// it does not start.
    pub fn take(&self, wanted: &[PluginUse]) -> Result<(), CiError> {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        let known: Vec<String> = self.ci.registry().list()?.into_iter().map(|view| view.name).collect();
        if let Some(missing) = wanted.iter().find(|plugin| !known.contains(&plugin.name)) {
            return Err(CiError::unavailable(format!(
                "este agente não tem o plugin {}, que o build pede",
                missing.name
            )));
        }
        let mut taken: Vec<String> = Vec::new();
        for plugin in wanted {
            if users.get(&plugin.name).copied().unwrap_or(0) == 0
                && let Err(e) = self.switch_on(plugin)
            {
                for name in &taken {
                    Self::give_back(&self.ci, &mut users, name);
                }
                return Err(e);
            }
            *users.entry(plugin.name.clone()).or_default() += 1;
            taken.push(plugin.name.clone());
        }
        Ok(())
    }

    fn switch_on(&self, plugin: &PluginUse) -> Result<(), CiError> {
        let registry = self.ci.registry();
        let view = registry.info(&plugin.name)?;
        for permission in &view.declared {
            let revoked = plugin.revoked.contains(permission);
            if revoked != !view.granted.contains(permission) {
                registry.revoke(&plugin.name, *permission, revoked)?;
            }
        }
        if matches!(&plugin.settings, Value::Object(map) if !map.is_empty()) {
            self.ci.configure_plugin(&plugin.name, plugin.settings.clone())?;
        }
        self.ci.enable_plugin(&plugin.name)?;
        let state = registry.info(&plugin.name)?.state;
        if state != PluginState::On {
            return Err(CiError::unavailable(format!(
                "o plugin {} não ligou: {}",
                plugin.name,
                state.words()
            )));
        }
        Ok(())
    }

    fn give_back(ci: &Ci, users: &mut HashMap<String, usize>, name: &str) {
        let left = users.get(name).copied().unwrap_or(0).saturating_sub(1);
        users.insert(name.to_owned(), left);
        if left == 0 {
            let _ = ci.disable_plugin(name);
        }
    }

    /// The build that needed `wanted` ended: what no other build needs goes off.
    pub fn give(&self, wanted: &[PluginUse]) {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        for plugin in wanted {
            Self::give_back(&self.ci, &mut users, &plugin.name);
        }
    }

    /// The plugins on now, by name.
    pub fn on(&self) -> Vec<String> {
        self.ci.registry().on().iter().map(|plugin| plugin.name().to_owned()).collect()
    }
}
