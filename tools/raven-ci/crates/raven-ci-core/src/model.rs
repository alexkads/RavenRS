//! The model of the core: folders, jobs, builds, their stages and steps, and what builds leave.
//! Jobs and builds are rows of the core's file; a build's run, its steps and the bytes of its
//! attachments are kept by `ravenrs-flows-runtime` in the same file.

use std::collections::BTreeMap;

use raven_ci_plugin_api::{AttachmentInfo, BuildStatus, CiError, STAGE_KEY};
use ravenrs_flows::{FlowDefinition, NodeRun, NodeStatus, RunOutcome};
use ravenrs_flows_runtime::{RunRecord, RunStatus};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::store::db;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ci_folders (
    path TEXT PRIMARY KEY,
    title TEXT,
    description TEXT
);
CREATE TABLE IF NOT EXISTS ci_jobs (
    name TEXT PRIMARY KEY,
    folder TEXT NOT NULL,
    title TEXT,
    format TEXT NOT NULL,
    pipeline TEXT NOT NULL,
    settings TEXT NOT NULL DEFAULT '{}',
    keep_builds INTEGER,
    keep_days INTEGER,
    coalesce INTEGER NOT NULL DEFAULT 1,
    created_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS ci_builds (
    job TEXT NOT NULL,
    number INTEGER NOT NULL,
    run_id TEXT NOT NULL,
    flow TEXT NOT NULL,
    cause TEXT NOT NULL,
    actor TEXT,
    parameters TEXT NOT NULL,
    origin INTEGER,
    created_ms INTEGER NOT NULL,
    PRIMARY KEY (job, number)
);
CREATE UNIQUE INDEX IF NOT EXISTS ci_builds_by_run ON ci_builds (run_id);
CREATE TABLE IF NOT EXISTS ci_attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job TEXT NOT NULL,
    number INTEGER NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    mime TEXT NOT NULL,
    size INTEGER NOT NULL,
    blob TEXT NOT NULL,
    summary TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS ci_attachments_by_build ON ci_attachments (job, number);
CREATE TABLE IF NOT EXISTS ci_log (
    job TEXT NOT NULL,
    number INTEGER NOT NULL,
    seq INTEGER NOT NULL,
    step TEXT NOT NULL,
    at_ms INTEGER NOT NULL,
    text TEXT NOT NULL,
    PRIMARY KEY (job, number, seq)
);
";

/// Makes the tables of the model when they are not there.
pub(crate) fn prepare(conn: &Connection) -> Result<(), CiError> {
    conn.execute_batch(SCHEMA).map_err(db)?;
    // A file made before the build kept what its pipeline said besides the flow.
    let has_extras: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('ci_builds') WHERE name = 'extras'",
            [],
            |row| row.get(0),
        )
        .map_err(db)?;
    if !has_extras {
        conn.execute_batch("ALTER TABLE ci_builds ADD COLUMN extras TEXT NOT NULL DEFAULT '{}'")
            .map_err(db)?;
    }
    // And one made before #919 did not keep the commit each build fetched.
    let has_commit: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('ci_builds') WHERE name = 'commit_sha'",
            [],
            |row| row.get(0),
        )
        .map_err(db)?;
    if !has_commit {
        conn.execute_batch("ALTER TABLE ci_builds ADD COLUMN commit_sha TEXT").map_err(db)?;
    }
    Ok(())
}

/// Keeps the commit a build fetched.
pub(crate) fn set_commit(conn: &Connection, job: &str, number: u64, commit: &str) -> Result<(), CiError> {
    conn.execute(
        "UPDATE ci_builds SET commit_sha = ?3 WHERE job = ?1 AND number = ?2",
        params![job, number as i64, commit],
    )
    .map_err(db)?;
    Ok(())
}

/// The commit a build fetched, when it fetched one.
pub(crate) fn commit_of(conn: &Connection, job: &str, number: u64) -> Result<Option<String>, CiError> {
    conn.query_row(
        "SELECT commit_sha FROM ci_builds WHERE job = ?1 AND number = ?2",
        params![job, number as i64],
        |row| row.get(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(db)
}

/// A job as it is asked to be: its name (a path such as `apps/site`), the format of its pipeline
/// and the text, and how many builds it keeps.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JobSpec {
    /// Its path: the folders, then its own name, apart by `/`.
    pub name: String,
    /// What people see; its name when `None`.
    pub title: Option<String>,
    /// The format of its pipeline, by the id of a plugin's format: `toml`.
    pub format: String,
    /// The text of its pipeline.
    pub pipeline: String,
    /// The settings of the plugins it names, by plugin.
    pub settings: Value,
    /// How many builds it keeps; all when `None`.
    pub keep_builds: Option<u32>,
    /// For how many days it keeps a build; forever when `None`.
    pub keep_days: Option<u32>,
    /// A request equal to one still in the queue joins it (on by default).
    pub coalesce: bool,
}

impl JobSpec {
    /// The job `name`, whose pipeline is `pipeline` in `format`.
    pub fn new(name: impl Into<String>, format: impl Into<String>, pipeline: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            format: format.into(),
            pipeline: pipeline.into(),
            settings: Value::Object(Map::new()),
            keep_builds: None,
            keep_days: None,
            coalesce: true,
        }
    }

    /// Seen as `title`.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Keeping the last `builds` builds.
    pub fn keep_builds(mut self, builds: u32) -> Self {
        self.keep_builds = Some(builds);
        self
    }

    /// Keeping builds for `days` days.
    pub fn keep_days(mut self, days: u32) -> Self {
        self.keep_days = Some(days);
        self
    }

    /// With `settings`.
    pub fn settings(mut self, settings: Value) -> Self {
        self.settings = settings;
        self
    }

    /// Making a build for every request, even one equal to a build in the queue.
    pub fn every_request(mut self) -> Self {
        self.coalesce = false;
        self
    }

    /// The folder it is in: what comes before its last `/`; empty at the top.
    pub fn folder(&self) -> String {
        self.name.rsplit_once('/').map(|(folder, _)| folder.to_owned()).unwrap_or_default()
    }

    /// Checks its name: parts of letters, numbers, `-`, `_` and `.`, apart by `/`.
    pub(crate) fn check(&self) -> Result<(), CiError> {
        let fine = !self.name.is_empty()
            && self.name.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            });
        if fine {
            Ok(())
        } else {
            Err(CiError::settings(
                "name",
                format!(
                    "{:?} não serve: partes de letras, números, '-', '_' e '.', separadas por '/'",
                    self.name
                ),
            ))
        }
    }
}

/// A folder of jobs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Folder {
    /// Its path: `apps`, `apps/web`.
    pub path: String,
    /// What people see.
    pub title: Option<String>,
    /// The jobs right in it, by name.
    pub jobs: Vec<String>,
}

/// A build, as the screens and the command line show it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Build {
    /// Its job.
    pub job: String,
    /// Its number in the job, from 1.
    pub number: u64,
    /// Where it is.
    pub status: BuildStatus,
    /// Why it was asked for.
    pub cause: String,
    /// Who asked.
    pub actor: Option<String>,
    /// The values of its parameters.
    pub parameters: Map<String, Value>,
    /// The build it runs again, when it is a rerun.
    pub origin: Option<u64>,
    /// When it was asked for.
    pub created_ms: i64,
    /// When it ended.
    pub ended_ms: Option<i64>,
    /// Its place in the queue: the highest leaves first.
    pub priority: i32,
    /// How many later requests joined it while it waited.
    pub joined: u32,
    /// Why it did not pass: the step that failed and why, or why it was cancelled.
    pub reason: Option<String>,
    /// Its run in the queue.
    pub run_id: String,
    /// The commit it fetched, when its job fetches code.
    pub commit: Option<String>,
    /// Who took it from the queue: `executor-1` of the server, or `agente/<name>`.
    pub worker: Option<String>,
}

/// A step of a build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StepView {
    /// Its node in the flow.
    pub node: String,
    /// What it is called.
    pub title: String,
    /// Its kind, which a plugin brings.
    pub kind: String,
    /// How it went: `done`, `failed`, `skipped`…, or `pending` when it did not run yet.
    pub status: String,
    /// Why it failed.
    pub error: Option<String>,
    /// How many tries it took.
    pub attempts: u32,
}

/// A stage of a build: its steps, in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Stage {
    /// Its name.
    pub name: String,
    /// `done` when every step passed, `failed` when one failed, `running` while some ran and
    /// others did not, `pending` before any ran.
    pub status: String,
    /// Its steps.
    pub steps: Vec<StepView>,
}

/// What a build left, without its bytes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AttachmentRef {
    /// Its id, to read the bytes.
    pub id: i64,
    /// What kind it is: `artifact`, `junit`.
    pub kind: String,
    /// Its name.
    pub name: String,
    /// Its type.
    pub mime: String,
    /// Its size in bytes.
    pub size: u64,
    /// What a screen shows at a glance.
    pub summary: Value,
}

/// A row of `ci_builds`.
pub(crate) struct BuildRow {
    pub job: String,
    pub number: u64,
    pub run_id: String,
    pub flow: String,
    pub cause: String,
    pub actor: Option<String>,
    pub parameters: Map<String, Value>,
    pub origin: Option<u64>,
    pub created_ms: i64,
    /// What the pipeline said besides the flow: its environment, post steps, publishers and
    /// time limit, as [`Extras`](crate::lifecycle::Extras) writes them.
    pub extras: Value,
}

const BUILD_COLUMNS: &str = "job, number, run_id, flow, cause, actor, parameters, origin, created_ms, extras";

fn build_row(row: &Row<'_>) -> rusqlite::Result<BuildRow> {
    let parameters: String = row.get(6)?;
    Ok(BuildRow {
        job: row.get(0)?,
        number: row.get::<_, i64>(1)? as u64,
        run_id: row.get(2)?,
        flow: row.get(3)?,
        cause: row.get(4)?,
        actor: row.get(5)?,
        parameters: serde_json::from_str(&parameters).unwrap_or_default(),
        origin: row.get::<_, Option<i64>>(7)?.map(|n| n as u64),
        created_ms: row.get(8)?,
        extras: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or(Value::Null),
    })
}

pub(crate) fn save_job(conn: &Connection, job: &JobSpec, now: i64) -> Result<(), CiError> {
    conn.execute(
        "INSERT INTO ci_jobs (name, folder, title, format, pipeline, settings, keep_builds, keep_days, coalesce, created_ms, updated_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT (name) DO UPDATE SET folder = excluded.folder, title = excluded.title, format = excluded.format,
             pipeline = excluded.pipeline, settings = excluded.settings, keep_builds = excluded.keep_builds,
             keep_days = excluded.keep_days, coalesce = excluded.coalesce, updated_ms = excluded.updated_ms",
        params![
            job.name,
            job.folder(),
            job.title,
            job.format,
            job.pipeline,
            job.settings.to_string(),
            job.keep_builds,
            job.keep_days,
            job.coalesce,
            now
        ],
    )
    .map_err(db)?;
    Ok(())
}

pub(crate) fn job(conn: &Connection, name: &str) -> Result<JobSpec, CiError> {
    conn.query_row(
        "SELECT name, title, format, pipeline, settings, keep_builds, keep_days, coalesce FROM ci_jobs WHERE name = ?1",
        params![name],
        |row| {
            let settings: String = row.get(4)?;
            Ok(JobSpec {
                name: row.get(0)?,
                title: row.get(1)?,
                format: row.get(2)?,
                pipeline: row.get(3)?,
                settings: serde_json::from_str(&settings).unwrap_or(Value::Null),
                keep_builds: row.get(5)?,
                keep_days: row.get(6)?,
                coalesce: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(db)?
    .ok_or_else(|| CiError::unavailable(format!("não há o job {name}")))
}

pub(crate) fn job_names(conn: &Connection) -> Result<Vec<String>, CiError> {
    let mut stmt = conn.prepare("SELECT name FROM ci_jobs ORDER BY name").map_err(db)?;
    let rows = stmt.query_map([], |row| row.get(0)).map_err(db)?;
    rows.collect::<Result<_, _>>().map_err(db)
}

pub(crate) fn delete_job(conn: &Connection, name: &str) -> Result<(), CiError> {
    conn.execute("DELETE FROM ci_jobs WHERE name = ?1", params![name]).map_err(db)?;
    Ok(())
}

pub(crate) fn folders(conn: &Connection) -> Result<Vec<Folder>, CiError> {
    let mut by_path: BTreeMap<String, Folder> = BTreeMap::new();
    for name in job_names(conn)? {
        let folder = name.rsplit_once('/').map(|(f, _)| f.to_owned()).unwrap_or_default();
        // Every folder above it is there too, even with no job right in it.
        let mut path = String::new();
        for part in folder.split('/').filter(|p| !p.is_empty()) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            by_path.entry(path.clone()).or_insert_with(|| Folder {
                path: path.clone(),
                title: None,
                jobs: Vec::new(),
            });
        }
        by_path.entry(folder.clone()).or_insert_with(|| Folder {
            path: folder.clone(),
            title: None,
            jobs: Vec::new(),
        });
        if let Some(entry) = by_path.get_mut(&folder) {
            entry.jobs.push(name);
        }
    }
    let mut stmt = conn.prepare("SELECT path, title FROM ci_folders").map_err(db)?;
    let titles = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)))
        .map_err(db)?;
    for title in titles {
        let (path, title) = title.map_err(db)?;
        by_path.entry(path.clone()).or_insert_with(|| Folder { path, title: None, jobs: Vec::new() }).title =
            title;
    }
    Ok(by_path.into_values().collect())
}

pub(crate) fn set_folder_title(conn: &Connection, path: &str, title: &str) -> Result<(), CiError> {
    conn.execute(
        "INSERT INTO ci_folders (path, title) VALUES (?1, ?2) ON CONFLICT (path) DO UPDATE SET title = excluded.title",
        params![path, title],
    )
    .map_err(db)?;
    Ok(())
}

pub(crate) fn next_number(conn: &Connection, job: &str) -> Result<u64, CiError> {
    conn.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM ci_builds WHERE job = ?1", params![job], |row| {
        row.get::<_, i64>(0)
    })
    .map(|n| n as u64)
    .map_err(db)
}

pub(crate) fn insert_build(conn: &Connection, build: &BuildRow) -> Result<(), CiError> {
    conn.execute(
        &format!("INSERT INTO ci_builds ({BUILD_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"),
        params![
            build.job,
            build.number as i64,
            build.run_id,
            build.flow,
            build.cause,
            build.actor,
            Value::Object(build.parameters.clone()).to_string(),
            build.origin.map(|n| n as i64),
            build.created_ms,
            build.extras.to_string()
        ],
    )
    .map_err(db)?;
    Ok(())
}

pub(crate) fn build_by_run(conn: &Connection, run_id: &str) -> Result<Option<BuildRow>, CiError> {
    conn.query_row(
        &format!("SELECT {BUILD_COLUMNS} FROM ci_builds WHERE run_id = ?1"),
        params![run_id],
        build_row,
    )
    .optional()
    .map_err(db)
}

pub(crate) fn build(conn: &Connection, job: &str, number: u64) -> Result<BuildRow, CiError> {
    conn.query_row(
        &format!("SELECT {BUILD_COLUMNS} FROM ci_builds WHERE job = ?1 AND number = ?2"),
        params![job, number as i64],
        build_row,
    )
    .optional()
    .map_err(db)?
    .ok_or_else(|| CiError::unavailable(format!("não há o build {job} #{number}")))
}

pub(crate) fn builds_of(conn: &Connection, job: &str) -> Result<Vec<BuildRow>, CiError> {
    let mut stmt = conn
        .prepare(&format!("SELECT {BUILD_COLUMNS} FROM ci_builds WHERE job = ?1 ORDER BY number DESC"))
        .map_err(db)?;
    let rows = stmt.query_map(params![job], build_row).map_err(db)?;
    rows.collect::<Result<_, _>>().map_err(db)
}

/// Forgets the build, with the rows of what it left.
pub(crate) fn delete_build(conn: &Connection, job: &str, number: u64) -> Result<(), CiError> {
    let number = number as i64;
    conn.execute("DELETE FROM ci_attachments WHERE job = ?1 AND number = ?2", params![job, number])
        .map_err(db)?;
    conn.execute("DELETE FROM ci_log WHERE job = ?1 AND number = ?2", params![job, number]).map_err(db)?;
    conn.execute("DELETE FROM ci_builds WHERE job = ?1 AND number = ?2", params![job, number]).map_err(db)?;
    Ok(())
}

pub(crate) fn insert_attachment(
    conn: &Connection,
    job: &str,
    number: u64,
    attachment: &raven_ci_plugin_api::Attachment,
    blob: &str,
) -> Result<i64, CiError> {
    conn.execute(
        "INSERT INTO ci_attachments (job, number, kind, name, mime, size, blob, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            job,
            number as i64,
            attachment.kind,
            attachment.name,
            attachment.mime,
            attachment.bytes.len() as i64,
            blob,
            attachment.summary.to_string()
        ],
    )
    .map_err(db)?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn attachments(conn: &Connection, job: &str, number: u64) -> Result<Vec<AttachmentRef>, CiError> {
    let mut stmt = conn
        .prepare("SELECT id, kind, name, mime, size, summary FROM ci_attachments WHERE job = ?1 AND number = ?2 ORDER BY id")
        .map_err(db)?;
    let rows = stmt
        .query_map(params![job, number as i64], |row| {
            let summary: String = row.get(5)?;
            Ok(AttachmentRef {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                mime: row.get(3)?,
                size: row.get::<_, i64>(4)? as u64,
                summary: serde_json::from_str(&summary).unwrap_or(Value::Null),
            })
        })
        .map_err(db)?;
    rows.collect::<Result<_, _>>().map_err(db)
}

fn attachment_info_of(row: &Row<'_>) -> rusqlite::Result<AttachmentInfo> {
    let summary: String = row.get(7)?;
    Ok(AttachmentInfo {
        id: row.get(0)?,
        job: row.get(1)?,
        number: row.get::<_, i64>(2)? as u64,
        kind: row.get(3)?,
        name: row.get(4)?,
        mime: row.get(5)?,
        size: row.get::<_, i64>(6)? as u64,
        summary: serde_json::from_str(&summary).unwrap_or(Value::Null),
    })
}

const ATTACHMENT_INFO: &str = "SELECT id, job, number, kind, name, mime, size, summary FROM ci_attachments";

/// What the builds of `job` left of `kind`: of the build `number`, or of every build when `None`,
/// the newest build first and, in a build, in the order they were kept.
pub(crate) fn attachment_infos(
    conn: &Connection,
    job: &str,
    number: Option<u64>,
    kind: &str,
) -> Result<Vec<AttachmentInfo>, CiError> {
    let sql = format!(
        "{ATTACHMENT_INFO} WHERE job = ?1 AND kind = ?2 AND (?3 IS NULL OR number = ?3) ORDER BY number DESC, id"
    );
    let mut stmt = conn.prepare(&sql).map_err(db)?;
    let rows =
        stmt.query_map(params![job, kind, number.map(|n| n as i64)], attachment_info_of).map_err(db)?;
    rows.collect::<Result<_, _>>().map_err(db)
}

/// The attachment `id`, all but its bytes.
pub(crate) fn attachment_info(conn: &Connection, id: i64) -> Result<AttachmentInfo, CiError> {
    conn.query_row(&format!("{ATTACHMENT_INFO} WHERE id = ?1"), params![id], attachment_info_of)
        .optional()
        .map_err(db)?
        .ok_or_else(|| CiError::unavailable(format!("não há o anexo {id}")))
}

/// The blob of the attachment `id`, as kept: its type, size and where.
pub(crate) fn attachment_blob(conn: &Connection, id: i64) -> Result<(String, u64, String), CiError> {
    conn.query_row("SELECT mime, size, blob FROM ci_attachments WHERE id = ?1", params![id], |row| {
        Ok((row.get(0)?, row.get::<_, i64>(1)? as u64, row.get(2)?))
    })
    .optional()
    .map_err(db)?
    .ok_or_else(|| CiError::unavailable(format!("não há o anexo {id}")))
}

/// The start of the reason of a build the server ended because its agent went quiet past its
/// grace: such a build is [`BuildStatus::Interrupted`], not cancelled.
pub const INTERRUPTED: &str = "interrompido:";

/// Where a build is, from its run.
pub(crate) fn status_of(
    record: &RunRecord,
    parts: &BTreeMap<String, NodeRun>,
) -> (BuildStatus, Option<String>) {
    let failed = parts.iter().find(|(_, part)| part.status == NodeStatus::Failed);
    match (record.status, &record.outcome) {
        (RunStatus::Queued, _) => (BuildStatus::Queued, None),
        (RunStatus::Running | RunStatus::Waiting, _) => (BuildStatus::Running, None),
        // The server ended it when its agent went quiet past its grace (see `agents`).
        (RunStatus::Cancelled, Some(RunOutcome::Cancelled { reason })) if reason.starts_with(INTERRUPTED) => {
            (BuildStatus::Interrupted, Some(reason.clone()))
        }
        (RunStatus::Cancelled, Some(RunOutcome::Cancelled { reason })) => {
            (BuildStatus::Cancelled, Some(reason.clone()))
        }
        (RunStatus::Cancelled, _) => (BuildStatus::Cancelled, None),
        (RunStatus::TimedOut, _) => (BuildStatus::TimedOut, Some("passou do prazo".to_owned())),
        (RunStatus::Stopped, Some(RunOutcome::Stopped { node, reason })) if node.is_empty() => {
            (BuildStatus::Failed, Some(reason.clone()))
        }
        (RunStatus::Stopped, Some(RunOutcome::Stopped { node, reason })) => {
            (BuildStatus::Failed, Some(format!("o passo {node} falhou: {reason}")))
        }
        (RunStatus::Finished, _) => match failed {
            Some((node, part)) => (
                BuildStatus::Failed,
                Some(format!("o passo {node} falhou: {}", part.error.clone().unwrap_or_default())),
            ),
            None => match unstable_of(parts) {
                Some(reason) => (BuildStatus::Unstable, Some(reason)),
                None => (BuildStatus::Succeeded, None),
            },
        },
        _ => (BuildStatus::Failed, None),
    }
}

/// The output a step gives when its end is a warning (#947): its items say `reason`.
pub const UNSTABLE_PORT: &str = "unstable";

/// Why a build whose steps all ended is unstable: the first step that gave [`UNSTABLE_PORT`].
fn unstable_of(parts: &BTreeMap<String, NodeRun>) -> Option<String> {
    parts.iter().find_map(|(node, part)| {
        let items = part.outputs.get(UNSTABLE_PORT)?;
        let first = items.first()?;
        let why = first.json.get("reason").and_then(Value::as_str).unwrap_or("instável");
        Some(format!("o passo {node}: {why}"))
    })
}

/// The stages of a build, in the order they first come in its flow, with what each step did.
pub(crate) fn stages_of(flow: &FlowDefinition, parts: &BTreeMap<String, NodeRun>) -> Vec<Stage> {
    let mut stages: Vec<Stage> = Vec::new();
    for node in &flow.nodes {
        let Some(stage) = node.parameters.get(STAGE_KEY).and_then(Value::as_str) else { continue };
        let part = parts.get(&node.id);
        let step = StepView {
            node: node.id.clone(),
            title: node.title.clone().unwrap_or_else(|| node.id.clone()),
            kind: node.kind.clone(),
            status: part.map_or_else(|| "pending".to_owned(), |part| status_name(part.status)),
            error: part.and_then(|part| part.error.clone()),
            attempts: part.map_or(0, |part| part.attempts),
        };
        match stages.iter_mut().find(|s| s.name == stage) {
            Some(existing) => existing.steps.push(step),
            None => stages.push(Stage { name: stage.to_owned(), status: String::new(), steps: vec![step] }),
        }
    }
    for stage in &mut stages {
        let statuses: Vec<&str> = stage.steps.iter().map(|s| s.status.as_str()).collect();
        let passed = |s: &&str| matches!(*s, "done" | "skipped" | "disabled" | "pinned" | "reused");
        stage.status =
            if statuses.iter().any(|s| matches!(*s, "failed" | "timed_out" | "cancelled" | "uncertain")) {
                "failed"
            } else if statuses.iter().all(passed) {
                "done"
            } else if statuses.iter().any(|s| *s != "pending") {
                "running"
            } else {
                "pending"
            }
            .to_owned();
    }
    stages
}

/// The name of a node's status, as its JSON says it.
fn status_name(status: NodeStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{status:?}"))
}

/// A line of a build's log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogLine {
    /// Its place in the log, from 1: what a reader that follows the log asks after.
    pub seq: u64,
    /// The step that wrote it, by its node; empty for what the core wrote.
    pub step: String,
    /// When, on the core's clock.
    pub at_ms: i64,
    /// The line, with every secret hidden.
    pub text: String,
}

pub(crate) fn insert_log(conn: &Connection, job: &str, number: u64, line: &LogLine) -> Result<(), CiError> {
    conn.execute(
        "INSERT INTO ci_log (job, number, seq, step, at_ms, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![job, number as i64, line.seq as i64, line.step, line.at_ms, line.text],
    )
    .map_err(db)?;
    Ok(())
}

pub(crate) fn log_after(
    conn: &Connection,
    job: &str,
    number: u64,
    after: u64,
) -> Result<Vec<LogLine>, CiError> {
    let mut stmt = conn
        .prepare("SELECT seq, step, at_ms, text FROM ci_log WHERE job = ?1 AND number = ?2 AND seq > ?3 ORDER BY seq")
        .map_err(db)?;
    let rows = stmt
        .query_map(params![job, number as i64, after as i64], |row| {
            Ok(LogLine {
                seq: row.get::<_, i64>(0)? as u64,
                step: row.get(1)?,
                at_ms: row.get(2)?,
                text: row.get(3)?,
            })
        })
        .map_err(db)?;
    rows.collect::<Result<_, _>>().map_err(db)
}

pub(crate) fn last_log_seq(conn: &Connection, job: &str, number: u64) -> Result<u64, CiError> {
    conn.query_row(
        "SELECT COALESCE(MAX(seq), 0) FROM ci_log WHERE job = ?1 AND number = ?2",
        params![job, number as i64],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n as u64)
    .map_err(db)
}
