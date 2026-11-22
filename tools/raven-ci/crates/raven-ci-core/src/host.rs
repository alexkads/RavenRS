//! The host each plugin switched on gets: its corner of the server, by permission.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use raven_ci_plugin_api::{CiError, Host, Migration, Permission};
use rusqlite::{Connection, params};
use serde_json::Value;

use crate::store::Store;

/// Where the secrets plugins ask for come from: the vault, once the plugin of credentials is in.
pub trait Secrets: Send + Sync {
    /// The secret `name`, when there is one.
    fn get(&self, name: &str) -> Option<String>;
}

/// No secrets at all: what a server with no vault has.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSecrets;

impl Secrets for NoSecrets {
    fn get(&self, _: &str) -> Option<String> {
        None
    }
}

/// Secrets kept in memory: for tests and the self-test.
#[derive(Clone, Debug, Default)]
pub struct MemorySecrets(pub BTreeMap<String, String>);

impl Secrets for MemorySecrets {
    fn get(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

/// The last lines of the server's log, and the values no log may show.
#[derive(Debug, Default)]
pub struct ServerLog {
    lines: Mutex<VecDeque<String>>,
    hidden: Mutex<BTreeSet<String>>,
}

/// How many lines the server keeps.
const KEPT_LINES: usize = 1000;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ServerLog {
    /// Writes `line`, with what has to be hidden hidden.
    pub fn write(&self, line: &str) {
        let line = self.hide(line);
        let mut lines = lock(&self.lines);
        if lines.len() == KEPT_LINES {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// The lines kept, oldest first.
    pub fn lines(&self) -> Vec<String> {
        lock(&self.lines).iter().cloned().collect()
    }

    /// Hides `value` from now on, in this log and in what [`hide`](Self::hide) is asked.
    pub fn add_hidden(&self, value: &str) {
        if value.len() >= 3 {
            lock(&self.hidden).insert(value.to_owned());
        }
    }

    /// `text` with every hidden value as `****`.
    pub fn hide(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for value in lock(&self.hidden).iter() {
            text = text.replace(value.as_str(), "****");
        }
        text
    }
}

/// The host of one plugin switched on.
pub(crate) struct PluginHost {
    plugin: String,
    granted: Vec<Permission>,
    settings: Value,
    dir: PathBuf,
    store: Arc<Store>,
    secrets: Arc<dyn Secrets>,
    log: Arc<ServerLog>,
}

impl fmt::Debug for PluginHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginHost")
            .field("plugin", &self.plugin)
            .field("granted", &self.granted)
            .finish_non_exhaustive()
    }
}

impl PluginHost {
    pub(crate) fn new(
        plugin: &str,
        granted: Vec<Permission>,
        settings: Value,
        dir: PathBuf,
        store: Arc<Store>,
        secrets: Arc<dyn Secrets>,
        log: Arc<ServerLog>,
    ) -> Self {
        Self { plugin: plugin.to_owned(), granted, settings, dir, store, secrets, log }
    }

    /// Applies the migrations not applied yet to the plugin's own database, each once, in order.
    pub(crate) fn migrate(&self, migrations: &[Migration]) -> Result<(), CiError> {
        if migrations.is_empty() {
            return Ok(());
        }
        self.require(Permission::Disk, "aplicar as migrações do banco próprio")?;
        let path = self.database()?;
        let mut conn = Connection::open(&path).map_err(crate::store::db)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS raven_ci_migrations (id TEXT PRIMARY KEY, applied_ms INTEGER NOT NULL)",
        )
        .map_err(crate::store::db)?;
        for migration in migrations {
            let done: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM raven_ci_migrations WHERE id = ?1",
                    params![migration.id],
                    |row| row.get(0),
                )
                .map_err(crate::store::db)?;
            if done {
                continue;
            }
            let tx = conn.transaction().map_err(crate::store::db)?;
            tx.execute_batch(&migration.sql).map_err(|e| {
                CiError::failed(format!("a migração {} de {}: {e}", migration.id, self.plugin))
            })?;
            tx.execute(
                "INSERT INTO raven_ci_migrations (id, applied_ms) VALUES (?1, ?2)",
                params![migration.id, self.store.now_ms()],
            )
            .map_err(crate::store::db)?;
            tx.commit().map_err(crate::store::db)?;
        }
        Ok(())
    }
}

impl Host for PluginHost {
    fn plugin(&self) -> &str {
        &self.plugin
    }

    fn granted(&self) -> Vec<Permission> {
        self.granted.clone()
    }

    fn require(&self, permission: Permission, what: &str) -> Result<(), CiError> {
        let granted = self.granted.contains(&permission);
        self.store.audit(&self.plugin, permission, what, granted);
        if granted {
            Ok(())
        } else {
            Err(CiError::Denied { plugin: self.plugin.clone(), permission, what: what.to_owned() })
        }
    }

    fn settings(&self) -> Value {
        self.settings.clone()
    }

    fn data_dir(&self) -> Result<PathBuf, CiError> {
        self.require(Permission::Disk, "usar a pasta própria")?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| CiError::failed(format!("a pasta de {}: {e}", self.plugin)))?;
        Ok(self.dir.clone())
    }

    fn database(&self) -> Result<PathBuf, CiError> {
        self.require(Permission::Disk, "abrir o banco próprio")?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| CiError::failed(format!("a pasta de {}: {e}", self.plugin)))?;
        Ok(self.dir.join("plugin.sqlite"))
    }

    fn secret(&self, name: &str) -> Result<String, CiError> {
        self.require(Permission::Secrets, &format!("ler o segredo {name}"))?;
        let value =
            self.secrets.get(name).ok_or_else(|| CiError::unavailable(format!("não há o segredo {name}")))?;
        self.log.add_hidden(&value);
        Ok(value)
    }

    fn log(&self, line: &str) {
        self.log.write(&format!("[{}] {line}", self.plugin));
    }
}
