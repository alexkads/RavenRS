//! What the core offers a plugin that is switched on.

use std::path::PathBuf;

use serde_json::Value;

use crate::{CiError, Permission};

/// What the core offers a plugin that is switched on, given to [`CiPlugin::start`](crate::CiPlugin::start).
///
/// **Its role.** A plugin reaches the server only through its host: its configuration, its own
/// folder and database, secrets and the log. That is what makes permissions mean something and
/// what lets the core give each plugin its own corner, so two plugins never share a table.
///
/// **How it works.** Each service checks the [`Permission`] it needs with
/// [`require`](Self::require): granted, it answers; not granted, it answers
/// [`CiError::Denied`]. Both answers go to the audit with the plugin and what it wanted. The
/// database is a SQLite file of the plugin's own, with its migrations applied before `start`.
///
/// **How to use it.** Keep the `Arc` `start` gets, and ask it when the plugin needs something:
///
/// ```
/// # use std::sync::Arc;
/// # use raven_ci_plugin_api::{CiError, Host};
/// fn token(host: &Arc<dyn Host>) -> Result<String, CiError> {
///     host.secret("github-token")
/// }
/// ```
pub trait Host: Send + Sync {
    /// The plugin this host serves.
    fn plugin(&self) -> &str;

    /// The permissions granted to it.
    fn granted(&self) -> Vec<Permission>;

    /// Checks that `permission` was granted before doing `what` (in words: "ler o segredo x"),
    /// and writes the answer to the audit. The services below call it themselves; a plugin calls
    /// it before something of its own that needs a permission, such as opening a connection.
    fn require(&self, permission: Permission, what: &str) -> Result<(), CiError>;

    /// Its configuration, as the administrator set it and checked against
    /// [`CiPlugin::config`](crate::CiPlugin::config); an empty object when it has none.
    fn settings(&self) -> Value;

    /// Its own folder, made when it is not there. Needs [`Permission::Disk`].
    fn data_dir(&self) -> Result<PathBuf, CiError>;

    /// Its own SQLite file, with its migrations applied, for it to open. Needs
    /// [`Permission::Disk`].
    fn database(&self) -> Result<PathBuf, CiError>;

    /// The secret `name`. Needs [`Permission::Secrets`]. The value is added to what the log of
    /// every build hides.
    fn secret(&self, name: &str) -> Result<String, CiError>;

    /// Writes a line to the server's log, with the plugin's name.
    fn log(&self, line: &str);
}
