//! The file the core keeps: the state of each plugin, the audit, and the model of jobs and builds
//! (`model`), in tables `ci_*` of one SQLite file, the same file the runs of the queue live in.

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use raven_ci_plugin_api::{CiError, Permission};
use ravenrs_flows::Clock;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ci_plugins (
    name TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    settings TEXT NOT NULL DEFAULT '{}',
    revoked TEXT NOT NULL DEFAULT '[]'
);
CREATE TABLE IF NOT EXISTS ci_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at_ms INTEGER NOT NULL,
    plugin TEXT NOT NULL,
    permission TEXT NOT NULL,
    what TEXT NOT NULL,
    granted INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ci_audit_by_plugin ON ci_audit (plugin, id);
";

/// What the administrator set for a plugin.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginRow {
    /// Switched on by the administrator; a plugin seen for the first time is.
    pub enabled: bool,
    /// Its configuration.
    pub settings: Value,
    /// Permissions it declared and the administrator took away.
    pub revoked: Vec<Permission>,
}

impl Default for PluginRow {
    fn default() -> Self {
        Self { enabled: true, settings: Value::Object(serde_json::Map::new()), revoked: Vec::new() }
    }
}

/// A line of the audit: a plugin asked the host for something that needs a permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuditEntry {
    /// When, on the core's clock.
    pub at_ms: i64,
    /// The plugin.
    pub plugin: String,
    /// The permission it needed.
    pub permission: Permission,
    /// What it wanted, in words.
    pub what: String,
    /// Whether the host said yes.
    pub granted: bool,
}

/// The core's SQLite file.
pub struct Store {
    conn: Mutex<Connection>,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// The file at `path`, made when it is not there.
    pub fn open(path: &Path, clock: Arc<dyn Clock>) -> Result<Self, CiError> {
        Self::on(Connection::open(path).map_err(db)?, clock)
    }

    /// A store that lives only while it is held: for tests and the self-test.
    pub fn in_memory(clock: Arc<dyn Clock>) -> Result<Self, CiError> {
        Self::on(Connection::open_in_memory().map_err(db)?, clock)
    }

    fn on(conn: Connection, clock: Arc<dyn Clock>) -> Result<Self, CiError> {
        conn.busy_timeout(std::time::Duration::from_secs(10)).map_err(db)?;
        let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()));
        conn.execute_batch(SCHEMA).map_err(db)?;
        crate::model::prepare(&conn)?;
        Ok(Self { conn: Mutex::new(conn), clock })
    }

    /// The connection, for the modules of the core.
    pub(crate) fn db(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Now, on the core's clock.
    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    /// The core's clock.
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    /// What the administrator set for `name`; the defaults for a plugin never set.
    pub fn plugin(&self, name: &str) -> Result<PluginRow, CiError> {
        let row: Option<(bool, String, String)> = self
            .db()
            .query_row(
                "SELECT enabled, settings, revoked FROM ci_plugins WHERE name = ?1",
                params![name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(db)?;
        let Some((enabled, settings, revoked)) = row else { return Ok(PluginRow::default()) };
        let revoked: Vec<String> = serde_json::from_str(&revoked).unwrap_or_default();
        Ok(PluginRow {
            enabled,
            settings: serde_json::from_str(&settings).unwrap_or(Value::Null),
            revoked: revoked.iter().filter_map(|p| Permission::parse(p)).collect(),
        })
    }

    /// Keeps `row` for `name`.
    pub fn set_plugin(&self, name: &str, row: &PluginRow) -> Result<(), CiError> {
        let revoked: Vec<&str> = row.revoked.iter().map(|p| p.name()).collect();
        self.db()
            .execute(
                "INSERT INTO ci_plugins (name, enabled, settings, revoked) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (name) DO UPDATE SET enabled = excluded.enabled, settings = excluded.settings, revoked = excluded.revoked",
                params![name, row.enabled, row.settings.to_string(), serde_json::to_string(&revoked).unwrap_or_default()],
            )
            .map_err(db)?;
        Ok(())
    }

    /// Writes that `plugin` asked for `permission` to do `what`, and the answer.
    pub fn audit(&self, plugin: &str, permission: Permission, what: &str, granted: bool) {
        let now = self.now_ms();
        // The audit must not stop what it watches: a write that fails is left out.
        let _ = self.db().execute(
            "INSERT INTO ci_audit (at_ms, plugin, permission, what, granted) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![now, plugin, permission.name(), what, granted],
        );
    }

    /// The audit of `plugin`, oldest first.
    pub fn audit_of(&self, plugin: &str) -> Result<Vec<AuditEntry>, CiError> {
        let conn = self.db();
        let mut stmt = conn
            .prepare(
                "SELECT at_ms, plugin, permission, what, granted FROM ci_audit WHERE plugin = ?1 ORDER BY id",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![plugin], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(db)?;
        let mut entries = Vec::new();
        for row in rows {
            let (at_ms, plugin, permission, what, granted) = row.map_err(db)?;
            if let Some(permission) = Permission::parse(&permission) {
                entries.push(AuditEntry { at_ms, plugin, permission, what, granted });
            }
        }
        Ok(entries)
    }
}

/// A failure of the database, as the core says it.
pub(crate) fn db(e: rusqlite::Error) -> CiError {
    CiError::failed(format!("o banco do Raven CI: {e}"))
}
