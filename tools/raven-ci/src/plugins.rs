//! The plugins that come compiled in. Each one is made switched off and does nothing until the
//! core starts it; the administrator switches any of them off with `raven-ci plugins desligar`.

use std::sync::Arc;

use raven_ci_plugin_api::CiPlugin;
use raven_ci_plugin_format_toml::TomlFormatPlugin;
use raven_ci_plugin_lock::LockPlugin;
use raven_ci_plugin_shell::ShellPlugin;
use raven_ci_plugin_workspace::WorkspacePlugin;

/// Every plugin compiled in, in the order they are listed.
pub fn builtin() -> Vec<Arc<dyn CiPlugin>> {
    vec![
        Arc::new(TomlFormatPlugin),
        Arc::new(ShellPlugin::new()),
        Arc::new(LockPlugin::new()),
        Arc::new(WorkspacePlugin::new()),
    ]
}
