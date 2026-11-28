//! Durable runs: asking for a run, workers claiming and running them with checkpoints, waking
//! the waiting ones, running again, and forgetting the old ones. Above `store`.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use ravenrs_flows::{
    BinaryRef, CancelToken, Catalog, Clock, FlowDefinition, Item, Mode, NodeRun, NodeStatus, Plan, Run,
    RunOutcome, SystemClock, Wake,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::remote::{Held, Lease, StepRecord};
use crate::store::{self, StoreError};

/// How long a worker holds a run before another may take it, unless it renews: it renews at
/// every step.
pub const LEASE_MS: u64 = 60_000;

/// Where a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunStatus {
    /// Asked for, not taken yet.
    Queued,
    /// A worker holds it.
    Running,
    /// It waits for a time or an event.
    Waiting,
    /// Every node that could run, ran.
    Finished,
    /// It stopped at a node that failed.
    Stopped,
    /// It was cancelled.
    Cancelled,
    /// It took longer than its time limit.
    TimedOut,
}

impl RunStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Finished => "finished",
            Self::Stopped => "stopped",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
        }
    }

    fn parse(text: &str) -> Result<Self, StoreError> {
        Ok(match text {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "waiting" => Self::Waiting,
            "finished" => Self::Finished,
            "stopped" => Self::Stopped,
            "cancelled" => Self::Cancelled,
            "timed_out" => Self::TimedOut,
            other => return Err(StoreError::Unreadable(format!("a run status {other}"))),
        })
    }

    /// Whether the run is over and will not run again by itself.
    pub fn ended(self) -> bool {
        matches!(self, Self::Finished | Self::Stopped | Self::Cancelled | Self::TimedOut)
    }

    fn of(outcome: &RunOutcome) -> Self {
        match outcome {
            RunOutcome::Finished => Self::Finished,
            RunOutcome::Cancelled { .. } => Self::Cancelled,
            RunOutcome::TimedOut { .. } => Self::TimedOut,
            RunOutcome::Waiting { .. } => Self::Waiting,
            _ => Self::Stopped,
        }
    }
}

/// What to ask a run for: the flow, what it starts with, and who asks.
#[derive(Clone, Debug)]
pub struct Request {
    /// The flow, kept as it is now: later revisions do not change this run.
    pub flow: FlowDefinition,
    /// The node that starts with `input` instead of running; the flow's triggers run without it.
    pub start_node: Option<String>,
    /// The items the run starts with.
    pub input: Vec<Item>,
    /// Who asked, as the app names them.
    pub actor: Option<String>,
    /// A test from the editor, or the flow working.
    pub mode: Mode,
    /// Which runs leave the queue first: the highest, and among equals the oldest. 0 by default.
    pub priority: i32,
    /// Requests that ask for the same thing: one with the key of a run still in the queue joins
    /// that run instead of making another.
    pub key: Option<String>,
}

impl Request {
    /// A run of `flow` as it works, started by its triggers.
    pub fn new(flow: FlowDefinition) -> Self {
        Self {
            flow,
            start_node: None,
            input: Vec::new(),
            actor: None,
            mode: Mode::Active,
            priority: 0,
            key: None,
        }
    }

    /// Leaving the queue before the runs of a lower `priority`, and after those of a higher one.
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Joining the run of the same `key` still in the queue, if there is one, instead of making
    /// another: ten pushes in a row while a build waits make one build.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Starting at `node` with `input`.
    pub fn starting(mut self, node: impl Into<String>, input: Vec<Item>) -> Self {
        self.start_node = Some(node.into());
        self.input = input;
        self
    }

    /// Asked for by `actor`.
    pub fn by(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// In `mode`.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
}

/// A run as kept.
#[derive(Clone, Debug, PartialEq)]
pub struct RunRecord {
    /// Its id.
    pub id: String,
    /// The flow's id.
    pub flow_id: String,
    /// The flow's revision it runs.
    pub revision: u32,
    /// Where it is.
    pub status: RunStatus,
    /// How it ended, once it did (or how it waits).
    pub outcome: Option<RunOutcome>,
    /// The run it runs again, when it is a rerun.
    pub origin: Option<String>,
    /// Who asked for it.
    pub actor: Option<String>,
    /// The worker that holds it or last held it.
    pub worker: Option<String>,
    /// When it was asked for, on the runtime's clock.
    pub created_ms: i64,
    /// When it ended.
    pub ended_ms: Option<i64>,
    /// Where it goes in the queue: the highest first.
    pub priority: i32,
    /// What requests that ask for the same thing share, when it was given.
    pub key: Option<String>,
    /// How many later requests joined it while it waited in the queue.
    pub joined: u32,
}

/// Which flow a rerun runs.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Revision {
    /// The flow as the first run had it.
    Original,
    /// The flow as it is now.
    Current(FlowDefinition),
}

/// What [`Runtime::prune`] forgets: ended runs older than `older_than_ms`, of the statuses in
/// `statuses` (all, when empty), keeping the newest `keep_last` of each flow.
#[derive(Clone, Debug, Default)]
pub struct Retention {
    /// Only runs that ended this long ago, or longer.
    pub older_than_ms: Option<u64>,
    /// Only runs with these statuses; every ended status when empty.
    pub statuses: Vec<RunStatus>,
    /// The newest runs of each flow that stay whatever their age.
    pub keep_last: Option<usize>,
    /// Only the runs of this flow, so that each flow can keep its own; every flow when `None`.
    pub flow_id: Option<String>,
}

/// What a prune forgot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    /// Runs.
    pub runs: usize,
    /// Files the items of those runs carried.
    pub blobs: usize,
}

/// Durable runs of flows, kept in a SQLite file that several processes may share: each claims a
/// run with a lease, runs it writing a checkpoint after every node, and a run left by a worker
/// that died goes back to the queue when its lease runs out.
pub struct Runtime {
    conn: Arc<Mutex<Connection>>,
    catalog: Catalog,
    clock: Arc<dyn Clock>,
    lease_ms: u64,
    /// The token of each run this process is running, so cancelling it reaches the node at once.
    running: Arc<Mutex<BTreeMap<String, CancelToken>>>,
}

impl fmt::Debug for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Runtime").field("catalog", &self.catalog).field("lease_ms", &self.lease_ms).finish()
    }
}

impl Runtime {
    /// The runs kept in the file at `path`, made when it is not there, run with the kinds of
    /// `catalog`.
    pub fn open(path: impl AsRef<Path>, catalog: Catalog) -> Result<Self, StoreError> {
        Self::on(Connection::open(path)?, catalog)
    }

    /// Runs kept only while it lives: for tests and one-off runs.
    pub fn in_memory(catalog: Catalog) -> Result<Self, StoreError> {
        Self::on(Connection::open_in_memory()?, catalog)
    }

    fn on(conn: Connection, catalog: Catalog) -> Result<Self, StoreError> {
        store::prepare(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            catalog,
            clock: Arc::new(SystemClock),
            lease_ms: LEASE_MS,
            running: Arc::default(),
        })
    }

    /// The same runs, on the same connection and clock, run with the kinds of `catalog`: what a
    /// host whose kinds change while it runs uses (a plugin switched on or off), so the queue
    /// goes on whole, even one kept only in memory. Cancelling a run reaches it from either.
    pub fn with_catalog(&self, catalog: Catalog) -> Self {
        Self {
            conn: self.conn.clone(),
            catalog,
            clock: self.clock.clone(),
            lease_ms: self.lease_ms,
            running: self.running.clone(),
        }
    }

    /// On `clock`: the time runs are asked for, held, woken and forgotten by.
    pub fn clock(mut self, clock: impl Clock + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// With leases of `ms` milliseconds.
    pub fn lease_ms(mut self, ms: u64) -> Self {
        self.lease_ms = ms.max(1);
        self
    }

    /// How long its leases last, in milliseconds: what a worker whose node runs long renews within.
    pub fn lease_length(&self) -> u64 {
        self.lease_ms
    }

    fn db(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    /// Asks for a run; its id. It waits in the queue until a worker takes it, after the runs of
    /// a higher [`priority`](Request::priority). A request with a [`key`](Request::key) that a run
    /// still in the queue has joins that run and gives its id: the run counts it in
    /// [`RunRecord::joined`] and takes the higher of the two priorities.
    pub fn enqueue(&self, request: Request) -> Result<String, StoreError> {
        self.insert(&request, None)
    }

    fn insert(&self, request: &Request, origin: Option<&str>) -> Result<String, StoreError> {
        let now = self.now();
        let mut db = self.db();
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(key) = &request.key {
            let waiting: Option<String> = tx
                .query_row(
                    "SELECT id FROM runs WHERE dedupe_key = ?1 AND status = 'queued' ORDER BY created_ms, id LIMIT 1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(id) = waiting {
                tx.execute(
                    "UPDATE runs SET joined = joined + 1, priority = MAX(priority, ?2) WHERE id = ?1",
                    params![id, request.priority],
                )?;
                tx.commit()?;
                return Ok(id);
            }
        }
        let id = store::new_id("run", now);
        let mode = if request.mode == Mode::Test { "test" } else { "active" };
        tx.execute(
            "INSERT INTO runs (id, flow_id, revision, definition, mode, actor, start_node, input, status, origin, created_ms, priority, dedupe_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, ?10, ?11, ?12)",
            params![
                id,
                request.flow.id,
                request.flow.revision,
                request.flow.to_json(),
                mode,
                request.actor,
                request.start_node,
                serde_json::to_string(&request.input)?,
                origin,
                now,
                request.priority,
                request.key
            ],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Cancels `id`, saying `reason`; where it is after. A run in the queue or waiting ends as
    /// cancelled without running again. A run this process is running is cancelled at once
    /// through its token; one another process runs is cancelled when that process writes its
    /// next step. A run that ended stays as it ended.
    pub fn cancel(&self, id: &str, reason: &str) -> Result<RunStatus, StoreError> {
        let outcome = serde_json::to_string(&RunOutcome::Cancelled { reason: reason.to_owned() })?;
        let now = self.now();
        let changed = self.db().execute(
            "UPDATE runs SET status = 'cancelled', outcome = ?2, ended_ms = ?3, lease_until = NULL, wake_at = NULL
             WHERE id = ?1 AND status IN ('queued', 'waiting')",
            params![id, outcome, now],
        )?;
        if changed == 0 {
            self.db().execute(
                "UPDATE runs SET cancel_requested = ?2 WHERE id = ?1 AND status = 'running'",
                params![id, reason],
            )?;
            if let Some(token) = self.tokens().get(id) {
                token.cancel(reason);
            }
        }
        Ok(self.get(id)?.status)
    }

    /// Moves `id` in the queue, to leave it before the runs of a lower `priority`. A run that is
    /// running or ended keeps its place in the history and is not changed.
    pub fn set_priority(&self, id: &str, priority: i32) -> Result<(), StoreError> {
        self.get(id)?;
        self.db().execute(
            "UPDATE runs SET priority = ?2 WHERE id = ?1 AND status IN ('queued', 'waiting')",
            params![id, priority],
        )?;
        Ok(())
    }

    /// The runs that wait, in the order they are to leave: those asked for and not taken, and
    /// those waiting for a time or an event, the highest priority first and among equals the
    /// oldest.
    pub fn queue(&self) -> Result<Vec<RunRecord>, StoreError> {
        let ids: Vec<String> = {
            let db = self.db();
            let mut stmt = db.prepare(
                "SELECT id FROM runs WHERE status IN ('queued', 'waiting') ORDER BY priority DESC, created_ms, id",
            )?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.iter().map(|id| self.get(id)).collect()
    }

    fn tokens(&self) -> MutexGuard<'_, BTreeMap<String, CancelToken>> {
        self.running.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Takes the run due first, for `worker`: one asked for, one whose worker's lease ran out,
    /// or one whose wait is over. Two workers never take the same run.
    pub fn claim(&self, worker: &str) -> Result<Option<String>, StoreError> {
        let now = self.now();
        store::claim(&mut self.db(), worker, now, now.saturating_add(self.lease_ms as i64))
    }

    /// Takes the run due first and runs it, as `worker`; its id, or `None` when nothing was due.
    pub fn work(&self, worker: &str) -> Result<Option<String>, StoreError> {
        match self.claim(worker)? {
            Some(id) => {
                self.execute(&id, worker)?;
                Ok(Some(id))
            }
            None => Ok(None),
        }
    }

    /// Runs whatever is due, one after another, until nothing is; how many it ran.
    pub fn work_until_idle(&self, worker: &str) -> Result<usize, StoreError> {
        let mut ran = 0;
        while self.work(worker)?.is_some() {
            ran += 1;
        }
        Ok(ran)
    }

    /// Ends `id`, which `worker` holds, as stopped with `reason` and without running it: what a
    /// host does when what a run needs before its first node failed (fetching code, a folder
    /// to work in). Its status after.
    pub fn fail(&self, id: &str, worker: &str, reason: &str) -> Result<RunStatus, StoreError> {
        let outcome = RunOutcome::Stopped { node: String::new(), reason: reason.to_owned() };
        self.end(id, worker, &outcome)
    }

    /// Runs `id`, which `worker` holds: from its checkpoint when it ran before. Each step is
    /// written as it happens, and the lease renewed.
    pub fn execute(&self, id: &str, worker: &str) -> Result<RunStatus, StoreError> {
        let lease = self.lease_of(id, worker)?;
        let failure: RefCell<Option<StoreError>> = RefCell::new(None);
        let lost = RefCell::new(false);
        let token = CancelToken::new();
        self.tokens().insert(id.to_owned(), token.clone());
        let run = lease.run(&self.catalog, self.clock.clone(), token.clone(), |record| {
            match self.record(id, worker, &record) {
                Ok(held) => {
                    if !held.held {
                        *lost.borrow_mut() = true;
                    }
                    if let Some(reason) = held.cancel
                        && !token.is_cancelled()
                    {
                        token.cancel(reason);
                    }
                }
                Err(e) => {
                    failure.borrow_mut().get_or_insert(e);
                }
            }
        });
        self.tokens().remove(id);
        if let Some(e) = failure.into_inner() {
            return Err(e);
        }
        if lost.into_inner() {
            return Ok(RunStatus::Running);
        }
        match run {
            Ok(run) => self.end(id, worker, &run.outcome),
            Err(reason) => self.end(id, worker, &RunOutcome::Stopped { node: String::new(), reason }),
        }
    }

    /// The runs due now, by id, in the order [`claim`](Self::claim) takes them: what a host that
    /// chooses which run goes to which worker (by the labels of a build agent) looks at before
    /// [`claim_id`](Self::claim_id).
    pub fn due(&self) -> Result<Vec<String>, StoreError> {
        store::due(&self.db(), self.now())
    }

    /// Takes the run `id` for `worker`, when it is due; whether it did. Two workers never take the
    /// same run.
    pub fn claim_id(&self, id: &str, worker: &str) -> Result<bool, StoreError> {
        let now = self.now();
        store::claim_id(&self.db(), id, worker, now, now.saturating_add(self.lease_ms as i64))
    }

    /// The runs `worker` holds now, running, oldest first: what a host gives back to a worker
    /// elsewhere that came back as a new process and lost track of them.
    pub fn held_by(&self, worker: &str) -> Result<Vec<String>, StoreError> {
        let db = self.db();
        let mut stmt = db.prepare(
            "SELECT id FROM runs WHERE worker = ?1 AND status = 'running' ORDER BY created_ms, id",
        )?;
        let rows = stmt.query_map(params![worker], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// What a worker in another process needs to run `id`, which `worker` holds: see
    /// [`Lease`]. [`StoreError::NotHeld`] when `worker` does not hold it.
    pub fn lease(&self, id: &str, worker: &str) -> Result<Lease, StoreError> {
        if !store::holds(&self.db(), id, worker)? {
            return Err(StoreError::NotHeld(id.to_owned()));
        }
        self.lease_of(id, worker)
    }

    fn lease_of(&self, id: &str, worker: &str) -> Result<Lease, StoreError> {
        let row = self.row(id)?;
        let flow =
            FlowDefinition::from_json(&row.definition).map_err(|e| StoreError::Unreadable(e.to_string()))?;
        let started_ms = match row.started_ms {
            Some(ms) => ms,
            None => {
                let now = self.now();
                self.db().execute("UPDATE runs SET started_ms = ?2 WHERE id = ?1", params![id, now])?;
                now
            }
        };
        // A flow that no longer compiles keeps every part: the worker refuses it anyway.
        let plan = Plan::compile(&flow, &self.catalog).ok();
        let Kept { checkpoint, in_progress, woken } = self.checkpoint(id, plan.as_ref(), started_ms)?;
        let mut events = BTreeMap::new();
        for (name, items) in store::events(&self.db(), id)? {
            events.insert(name, serde_json::from_str::<Vec<Item>>(&items)?);
        }
        let settled = checkpoint
            .order
            .iter()
            .filter_map(|node| checkpoint.nodes.get(node).map(|part| (node.clone(), part.clone())))
            .collect();
        Ok(Lease {
            id: id.to_owned(),
            worker: worker.to_owned(),
            definition: row.definition,
            test: row.mode == Mode::Test,
            start_node: row.start_node,
            input: row.input,
            started_ms,
            settled,
            in_progress,
            woken,
            events,
            cancel_requested: row.cancel_requested,
        })
    }

    /// Writes `step` of `id`, which `worker` runs elsewhere, and renews its lease; whether it still
    /// holds the run and whether someone asked to cancel it. A worker that no longer holds it
    /// writes nothing.
    pub fn record(&self, id: &str, worker: &str, step: &StepRecord) -> Result<Held, StoreError> {
        {
            let db = self.db();
            if !store::holds(&db, id, worker)? {
                return Ok(Held { held: false, cancel: None });
            }
            match step {
                StepRecord::Started { node } => store::started(&db, id, node)?,
                StepRecord::Settled { node, part } => {
                    store::settled(&db, id, node, &serde_json::to_string(part)?)?;
                }
                StepRecord::LoopDone { head } => store::loop_done(&db, id, head)?,
            }
        }
        self.renew(id, worker)
    }

    /// Gives `worker` more time on `id`, with no step to write: what a worker does while a node
    /// runs for long, and what a host does for a worker it knows is still there. Whether it still
    /// holds the run, and whether someone asked to cancel it.
    pub fn renew(&self, id: &str, worker: &str) -> Result<Held, StoreError> {
        let db = self.db();
        let until = self.now().saturating_add(self.lease_ms as i64);
        let held = store::renew(&db, id, worker, until)?;
        let cancel: Option<String> = db
            .query_row("SELECT cancel_requested FROM runs WHERE id = ?1", params![id], |row| row.get(0))
            .optional()?
            .flatten();
        Ok(Held { held, cancel })
    }

    /// Ends `id`, which `worker` ran elsewhere, as `outcome` says; its status after.
    /// [`StoreError::NotHeld`] when `worker` no longer holds it, and then nothing changes.
    pub fn finish(&self, id: &str, worker: &str, outcome: &RunOutcome) -> Result<RunStatus, StoreError> {
        if !store::holds(&self.db(), id, worker)? {
            return Err(StoreError::NotHeld(id.to_owned()));
        }
        self.end(id, worker, outcome)
    }

    /// What `id` kept: the parts of the nodes that settled (a loop only once it ended), the
    /// nodes it was in when it stopped, and how each waiting node waits.
    fn checkpoint(&self, id: &str, plan: Option<&Plan>, started: i64) -> Result<Kept, StoreError> {
        let db = self.db();
        let ended: BTreeSet<String> = store::loops(&db, id)?.into_iter().collect();
        let mut unfinished: BTreeSet<&str> = BTreeSet::new();
        if let Some(plan) = plan {
            for head in plan.order().iter().filter(|n| !plan.loop_body(n).is_empty()) {
                if !ended.contains(head) {
                    unfinished.insert(head);
                    unfinished.extend(plan.loop_body(head).iter().map(String::as_str));
                }
            }
        }
        let mut run = Run::default();
        run.started_ms = started;
        let (mut busy, mut woken) = (BTreeSet::new(), BTreeMap::new());
        for step in store::steps(&db, id)? {
            if unfinished.contains(step.node.as_str()) {
                continue;
            }
            match (step.settled, step.part) {
                (true, Some(json)) => {
                    let part: NodeRun = serde_json::from_str(&json)?;
                    if part.status == NodeStatus::Waiting {
                        woken.insert(step.node.clone(), part.wake.clone().unwrap_or_default());
                    }
                    run.order.push(step.node.clone());
                    run.nodes.insert(step.node, part);
                }
                _ => {
                    busy.insert(step.node);
                }
            }
        }
        Ok(Kept { checkpoint: run, in_progress: busy, woken })
    }

    fn end(&self, id: &str, worker: &str, outcome: &RunOutcome) -> Result<RunStatus, StoreError> {
        let status = RunStatus::of(outcome);
        let (wake_at, wake_event) = match outcome {
            RunOutcome::Waiting { wake, .. } => (wake.until_ms, wake.event.clone()),
            _ => (None, None),
        };
        let ended = status.ended().then(|| self.now());
        self.db().execute(
            "UPDATE runs SET status = ?3, outcome = ?4, wake_at = ?5, wake_event = ?6, ended_ms = ?7, lease_until = NULL
             WHERE id = ?1 AND worker = ?2 AND status = 'running'",
            params![id, worker, status.as_str(), serde_json::to_string(outcome)?, wake_at, wake_event, ended],
        )?;
        Ok(status)
    }

    /// Tells a run waiting for `event` that it came, with `items`; it is due at once.
    pub fn signal(&self, id: &str, event: &str, items: Vec<Item>) -> Result<(), StoreError> {
        let db = self.db();
        db.execute(
            "INSERT INTO events (run_id, name, items) VALUES (?1, ?2, ?3)
             ON CONFLICT (run_id, name) DO UPDATE SET items = excluded.items",
            params![id, event, serde_json::to_string(&items)?],
        )?;
        db.execute(
            "UPDATE runs SET wake_at = ?3 WHERE id = ?1 AND status = 'waiting' AND wake_event = ?2",
            params![id, event, self.now()],
        )?;
        Ok(())
    }

    /// Asks for `id` to run again, with the same input, on the flow `revision` says; the new
    /// run's id, which keeps where it came from.
    pub fn rerun(&self, id: &str, revision: Revision) -> Result<String, StoreError> {
        let row = self.row(id)?;
        let flow = match revision {
            Revision::Current(flow) => flow,
            _ => FlowDefinition::from_json(&row.definition)
                .map_err(|e| StoreError::Unreadable(e.to_string()))?,
        };
        // Running again is asked for on purpose: it never joins a run in the queue.
        let request = Request {
            flow,
            start_node: row.start_node,
            input: row.input,
            actor: row.actor,
            mode: row.mode,
            priority: row.priority,
            key: None,
        };
        self.insert(&request, Some(id))
    }

    /// The run `id`, as kept.
    pub fn get(&self, id: &str) -> Result<RunRecord, StoreError> {
        self.db()
            .query_row(
                "SELECT id, flow_id, revision, status, outcome, origin, actor, worker, created_ms, ended_ms, priority, dedupe_key, joined FROM runs WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        (row.get::<_, i32>(10)?, row.get::<_, Option<String>>(11)?, row.get::<_, u32>(12)?),
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| StoreError::NoRun(id.to_owned()))
            .and_then(|(id, flow_id, revision, status, outcome, origin, actor, worker, created_ms, ended_ms, queue)| {
                let (priority, key, joined) = queue;
                Ok(RunRecord {
                    id,
                    flow_id,
                    revision,
                    status: RunStatus::parse(&status)?,
                    outcome: outcome.map(|o| serde_json::from_str(&o)).transpose()?,
                    origin,
                    actor,
                    worker,
                    created_ms,
                    ended_ms,
                    priority,
                    key,
                    joined,
                })
            })
    }

    /// The runs of the flow `flow_id`, the newest first.
    pub fn runs_of(&self, flow_id: &str) -> Result<Vec<RunRecord>, StoreError> {
        let ids: Vec<String> = {
            let db = self.db();
            let mut stmt =
                db.prepare("SELECT id FROM runs WHERE flow_id = ?1 ORDER BY created_ms DESC, id DESC")?;
            let rows = stmt.query_map(params![flow_id], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.iter().map(|id| self.get(id)).collect()
    }

    /// The part of each node of `id` that settled, by node: what the detail of a run shows.
    pub fn parts(&self, id: &str) -> Result<BTreeMap<String, NodeRun>, StoreError> {
        let mut parts = BTreeMap::new();
        for step in store::steps(&self.db(), id)? {
            if let (true, Some(json)) = (step.settled, step.part) {
                parts.insert(step.node, serde_json::from_str(&json)?);
            }
        }
        Ok(parts)
    }

    /// Keeps `bytes` for the run `id`; the reference an item carries to them.
    pub fn put_blob(&self, id: &str, mime: &str, bytes: &[u8]) -> Result<BinaryRef, StoreError> {
        let blob = store::new_id("blob", self.now());
        self.db().execute(
            "INSERT INTO blobs (id, run_id, mime, bytes) VALUES (?1, ?2, ?3, ?4)",
            params![blob, id, mime, bytes],
        )?;
        Ok(BinaryRef { mime: mime.to_owned(), size: bytes.len() as u64, location: format!("blob:{blob}") })
    }

    /// The bytes a reference from [`put_blob`](Self::put_blob) points to, while they are kept.
    pub fn blob(&self, binary: &BinaryRef) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(blob) = binary.location.strip_prefix("blob:") else { return Ok(None) };
        Ok(self
            .db()
            .query_row("SELECT bytes FROM blobs WHERE id = ?1", params![blob], |row| row.get(0))
            .optional()?)
    }

    /// Forgets the ended runs `retention` says, with their steps, events and files.
    pub fn prune(&self, retention: &Retention) -> Result<Pruned, StoreError> {
        let now = self.now();
        let candidates: Vec<(String, String, String, i64)> = {
            let db = self.db();
            let mut stmt = db.prepare(
                "SELECT id, flow_id, status, ended_ms FROM runs WHERE ended_ms IS NOT NULL ORDER BY flow_id, ended_ms DESC, id DESC",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut pruned = Pruned::default();
        for (id, flow, status, ended) in candidates {
            if retention.flow_id.as_ref().is_some_and(|only| *only != flow) {
                continue;
            }
            let place = seen.entry(flow).or_default();
            *place += 1;
            if retention.keep_last.is_some_and(|keep| *place <= keep) {
                continue;
            }
            if retention.older_than_ms.is_some_and(|age| now - ended < age as i64) {
                continue;
            }
            let status = RunStatus::parse(&status)?;
            if !retention.statuses.is_empty() && !retention.statuses.contains(&status) {
                continue;
            }
            pruned.blobs += store::delete(&self.db(), &id)?;
            pruned.runs += 1;
        }
        Ok(pruned)
    }

    fn row(&self, id: &str) -> Result<RunRow, StoreError> {
        let raw = self
            .db()
            .query_row(
                "SELECT definition, mode, actor, start_node, input, started_ms, priority, cancel_requested FROM runs WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        (row.get::<_, i32>(6)?, row.get::<_, Option<String>>(7)?),
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| StoreError::NoRun(id.to_owned()))?;
        let (definition, mode, actor, start_node, input, started_ms, (priority, cancel_requested)) = raw;
        Ok(RunRow {
            definition,
            mode: if mode == "test" { Mode::Test } else { Mode::Active },
            actor,
            start_node,
            input: serde_json::from_str(&input)?,
            started_ms,
            priority,
            cancel_requested,
        })
    }
}

/// What a run kept: the parts to go on from, the nodes it was in, and how each waiting node waits.
struct Kept {
    checkpoint: Run,
    in_progress: BTreeSet<String>,
    woken: BTreeMap<String, Wake>,
}

struct RunRow {
    definition: String,
    mode: Mode,
    actor: Option<String>,
    start_node: Option<String>,
    input: Vec<Item>,
    started_ms: Option<i64>,
    priority: i32,
    cancel_requested: Option<String>,
}
