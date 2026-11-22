//! What the core offers a plugin that is switched on.

use std::path::PathBuf;

use serde_json::Value;

use crate::{AttachmentInfo, CiError, Permission};

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

    /// What the builds of `job` left of `kind` (`artifact`, `junit`), without the bytes.
    ///
    /// **Its role.** What a [`Publisher`](crate::Publisher) returned is kept by the core, not by
    /// the plugin; a route that lists artifacts or shows the trend of the tests of a job reads it
    /// back here.
    ///
    /// **How it works.** Of the build `number`, or of every build of the job still kept when
    /// `None`, the newest build first and, in a build, in the order they were kept. Asks no
    /// permission: it reads only what builds left, and the route that calls it is behind the
    /// sign-in. A host that keeps no builds answers [`CiError::Unavailable`], as this default
    /// does.
    fn attachments(
        &self,
        job: &str,
        number: Option<u64>,
        kind: &str,
    ) -> Result<Vec<AttachmentInfo>, CiError> {
        let _ = (job, number, kind);
        Err(CiError::unavailable("este núcleo não dá os anexos dos builds aos plugins"))
    }

    /// The attachment `id`, with all its bytes in memory.
    ///
    /// **Its role and its size.** For attachments the size of a report: a summary of tests, a
    /// page, a log. The download of a big artifact is the route of the plugin `artifacts`, and
    /// these bytes are read once from the blob the core keeps them in, with no second copy.
    ///
    /// **How it works.** [`CiError::Unavailable`] when there is no such attachment, its bytes went
    /// with the retention, or the host keeps no builds (this default).
    fn attachment(&self, id: i64) -> Result<(AttachmentInfo, Vec<u8>), CiError> {
        let _ = id;
        Err(CiError::unavailable("este núcleo não dá os anexos dos builds aos plugins"))
    }
}
