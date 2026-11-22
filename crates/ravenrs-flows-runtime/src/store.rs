//! The tables a durable run lives in, and the statements on them. The bottom of the crate:
//! SQL and JSON, no flow logic.
//!
//! - `runs`: one row per run, with the flow as it was when the run was asked for, its input,
//!   where it is (`status`), who holds it (`worker`, `lease_until`) and when it wakes.
//! - `steps`: one row per node of a run that started, `started` before it runs and `settled`
//!   with its part when it is over. A row left `started` is a node the run was in when it
//!   stopped.
//! - `loops_done`: the loops of a run that went round for the last time.
//! - `events`: what came for a waiting run, by name.
//! - `blobs`: files the items of a run carry, kept apart from their JSON.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{Connection, OptionalExtension, params};

/// What went wrong with the store.
#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// The database refused or failed.
    Database(rusqlite::Error),
    /// Something kept could not be read back.
    Unreadable(String),
    /// There is no run with this id.
    NoRun(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "the database: {e}"),
            Self::Unreadable(why) => write!(f, "something kept could not be read: {why}"),
            Self::NoRun(id) => write!(f, "there is no run {id}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Unreadable(e.to_string())
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    flow_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    definition TEXT NOT NULL,
    mode TEXT NOT NULL,
    actor TEXT,
    start_node TEXT,
    input TEXT NOT NULL,
    status TEXT NOT NULL,
    outcome TEXT,
    origin TEXT,
    worker TEXT,
    lease_until INTEGER,
    wake_at INTEGER,
    wake_event TEXT,
    started_ms INTEGER,
    created_ms INTEGER NOT NULL,
    ended_ms INTEGER,
    priority INTEGER NOT NULL DEFAULT 0,
    dedupe_key TEXT,
    joined INTEGER NOT NULL DEFAULT 0,
    cancel_requested TEXT
);
CREATE INDEX IF NOT EXISTS runs_by_status ON runs (status, created_ms);
CREATE TABLE IF NOT EXISTS steps (
    run_id TEXT NOT NULL,
    node TEXT NOT NULL,
    state TEXT NOT NULL,
    part TEXT,
    seq INTEGER NOT NULL,
    PRIMARY KEY (run_id, node)
);
CREATE TABLE IF NOT EXISTS loops_done (run_id TEXT NOT NULL, head TEXT NOT NULL, PRIMARY KEY (run_id, head));
CREATE TABLE IF NOT EXISTS events (run_id TEXT NOT NULL, name TEXT NOT NULL, items TEXT NOT NULL, PRIMARY KEY (run_id, name));
CREATE TABLE IF NOT EXISTS blobs (id TEXT PRIMARY KEY, run_id TEXT NOT NULL, mime TEXT NOT NULL, bytes BLOB NOT NULL);
CREATE INDEX IF NOT EXISTS blobs_by_run ON blobs (run_id);
";

/// Opens the tables on `conn`, making them when they are not there, with the settings two
/// processes on one file need: write-ahead log and waiting for a lock instead of failing.
pub(crate) fn prepare(conn: &Connection) -> Result<(), StoreError> {
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()));
    conn.execute_batch(SCHEMA)?;
    // Files made before the queue had priorities, keys and cancelling get the columns, empty.
    let columns: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(runs)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.collect::<Result<_, _>>()?
    };
    for (column, kind) in ADDED_COLUMNS {
        if !columns.iter().any(|c| c == column) {
            conn.execute_batch(&format!("ALTER TABLE runs ADD COLUMN {column} {kind}"))?;
        }
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS runs_in_line ON runs (status, priority DESC, created_ms);
         CREATE INDEX IF NOT EXISTS runs_by_key ON runs (dedupe_key, status);",
    )?;
    Ok(())
}

/// The columns that came after the first files, and how each is declared.
const ADDED_COLUMNS: [(&str, &str); 4] = [
    ("priority", "INTEGER NOT NULL DEFAULT 0"),
    ("dedupe_key", "TEXT"),
    ("joined", "INTEGER NOT NULL DEFAULT 0"),
    ("cancel_requested", "TEXT"),
];

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A new id, unique across the processes and threads that share a file: the time, the process
/// and a counter of this process.
pub(crate) fn new_id(prefix: &str, now_ms: i64) -> String {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    format!("{prefix}-{now_ms:013}-{}-{n:06}", std::process::id())
}

/// The run due first that no one holds: asked for, held by a worker whose lease ran out, or
/// waiting for a time that came; the highest priority first and, among equals, the oldest. It is
/// given to `worker` until `lease_until`, in one statement inside a write transaction, so two
/// workers never get the same run.
pub(crate) fn claim(
    conn: &mut Connection,
    worker: &str,
    now_ms: i64,
    lease_until: i64,
) -> Result<Option<String>, StoreError> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let id: Option<String> = tx
        .query_row(
            "UPDATE runs SET status = 'running', worker = ?1, lease_until = ?3
             WHERE id = (
                 SELECT id FROM runs
                 WHERE status = 'queued'
                    OR (status = 'running' AND lease_until < ?2)
                    OR (status = 'waiting' AND wake_at IS NOT NULL AND wake_at <= ?2)
                 ORDER BY priority DESC, created_ms, id LIMIT 1)
             RETURNING id",
            params![worker, now_ms, lease_until],
            |row| row.get(0),
        )
        .optional()?;
    tx.commit()?;
    Ok(id)
}

/// Gives `worker` more time on `run`; `false` when it no longer holds it.
pub(crate) fn renew(
    conn: &Connection,
    run: &str,
    worker: &str,
    lease_until: i64,
) -> Result<bool, StoreError> {
    let changed = conn.execute(
        "UPDATE runs SET lease_until = ?3 WHERE id = ?1 AND worker = ?2 AND status = 'running'",
        params![run, worker, lease_until],
    )?;
    Ok(changed == 1)
}

/// Writes that `node` of `run` started, keeping where it first came in the run.
pub(crate) fn started(conn: &Connection, run: &str, node: &str) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO steps (run_id, node, state, part, seq)
         VALUES (?1, ?2, 'started', NULL, (SELECT COUNT(*) FROM steps WHERE run_id = ?1))
         ON CONFLICT (run_id, node) DO UPDATE SET state = 'started'",
        params![run, node],
    )?;
    Ok(())
}

/// Writes the part of `node` of `run`, as JSON.
pub(crate) fn settled(conn: &Connection, run: &str, node: &str, part: &str) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO steps (run_id, node, state, part, seq)
         VALUES (?1, ?2, 'settled', ?3, (SELECT COUNT(*) FROM steps WHERE run_id = ?1))
         ON CONFLICT (run_id, node) DO UPDATE SET state = 'settled', part = excluded.part",
        params![run, node, part],
    )?;
    Ok(())
}

/// Writes that the loop `head` of `run` went round for the last time.
pub(crate) fn loop_done(conn: &Connection, run: &str, head: &str) -> Result<(), StoreError> {
    conn.execute("INSERT OR IGNORE INTO loops_done (run_id, head) VALUES (?1, ?2)", params![run, head])?;
    Ok(())
}

/// A step as kept: the node, whether it settled, and its part's JSON.
pub(crate) struct KeptStep {
    pub node: String,
    pub settled: bool,
    pub part: Option<String>,
}

/// The steps of `run`, in the order they first came.
pub(crate) fn steps(conn: &Connection, run: &str) -> Result<Vec<KeptStep>, StoreError> {
    let mut stmt =
        conn.prepare("SELECT node, state, part FROM steps WHERE run_id = ?1 ORDER BY seq, node")?;
    let rows = stmt.query_map(params![run], |row| {
        Ok(KeptStep { node: row.get(0)?, settled: row.get::<_, String>(1)? == "settled", part: row.get(2)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The loops of `run` that ended.
pub(crate) fn loops(conn: &Connection, run: &str) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT head FROM loops_done WHERE run_id = ?1")?;
    let rows = stmt.query_map(params![run], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The events that came for `run`: name and items as JSON.
pub(crate) fn events(conn: &Connection, run: &str) -> Result<Vec<(String, String)>, StoreError> {
    let mut stmt = conn.prepare("SELECT name, items FROM events WHERE run_id = ?1")?;
    let rows = stmt.query_map(params![run], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Deletes `run` and everything kept for it; how many files went with it.
pub(crate) fn delete(conn: &Connection, run: &str) -> Result<usize, StoreError> {
    let blobs = conn.execute("DELETE FROM blobs WHERE run_id = ?1", params![run])?;
    for table in ["steps", "loops_done", "events"] {
        conn.execute(&format!("DELETE FROM {table} WHERE run_id = ?1"), params![run])?;
    }
    conn.execute("DELETE FROM runs WHERE id = ?1", params![run])?;
    Ok(blobs)
}
