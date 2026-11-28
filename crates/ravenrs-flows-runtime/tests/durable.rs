//! The acceptance of #535: runs that outlive the worker and the app, kept in a SQLite file, on a
//! controlled clock and with no window. A worker dying is a node that panics in the middle of a
//! run; the app opening again is a new `Runtime` on the same file.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use ravenrs_flows::{
    Catalog, Clock, FlowDefinition, Item, ManualClock, NodeContext, NodeDef, NodeError, NodeKind, NodeSpec,
    NodeStatus, Ports, Reconciled, RunOutcome,
};
use ravenrs_flows_runtime::{Request, Retention, Revision, RunStatus, Runtime};
use serde_json::{Value, json};

const DAY: i64 = 24 * 60 * 60 * 1000;

/// A database file of its own for each test, removed when it ends.
struct TempDb(PathBuf);

impl TempDb {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ravenrs-flows-{name}-{}.sqlite", std::process::id()));
        for end in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{end}", path.display()));
        }
        Self(path)
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for end in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{end}", self.0.display()));
        }
    }
}

fn main_port(items: Vec<Item>) -> Ports {
    Ports::from([("main".to_owned(), items)])
}

/// Sends what came, and counts how many times it did: an effect outside the flow.
struct Send(Arc<AtomicU32>);
impl NodeKind for Send {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("send", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(main_port(cx.input("main").to_vec()))
    }
}

/// Takes the worker down the first time; after that, passes its items on. Asked whether it ran,
/// it says it did not (the panic came before its call).
struct Dies(Arc<AtomicBool>);
impl NodeKind for Dies {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("dies", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        if !self.0.swap(true, Ordering::SeqCst) {
            panic!("the worker died");
        }
        Ok(main_port(cx.input("main").to_vec()))
    }
    fn reconcile(&self, _: &NodeContext<'_>) -> Reconciled {
        Reconciled::NotDone
    }
}

/// Takes the worker down the first time, and cannot tell whether its call happened.
struct DiesUnknowing(Arc<AtomicU32>);
impl NodeKind for DiesUnknowing {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("charge", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("the worker died while charging");
        }
        Ok(main_port(cx.input("main").to_vec()))
    }
}

fn flow(kind: &str) -> FlowDefinition {
    FlowDefinition::new("orders", "Orders")
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "id": 1 }, { "id": 2 }])))
        .node(NodeDef::new("send", "send"))
        .node(NodeDef::new("risky", kind))
        .node(NodeDef::new("after", "join").parameter("field", json!("id")))
        .connect("start", "main", "send", "main")
        .connect("send", "main", "risky", "main")
        .connect("risky", "main", "after", "main")
}

/// A flow of built-in kinds only: a start and a join of the ids.
fn simple() -> FlowDefinition {
    FlowDefinition::new("orders", "Orders")
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "id": 1 }, { "id": 2 }])))
        .node(NodeDef::new("after", "join").parameter("field", json!("id")))
        .connect("start", "main", "after", "main")
}

#[test]
fn a_run_whose_worker_died_resumes_from_its_checkpoint_without_sending_twice() {
    let db = TempDb::new("resume");
    let (sent, died) = (Arc::new(AtomicU32::new(0)), Arc::new(AtomicBool::new(false)));
    let catalog = || Catalog::builtin().with(Send(sent.clone())).with(Dies(died.clone()));
    let clock = ManualClock::at(1_000);

    let id = {
        let first = Runtime::open(&db.0, catalog()).expect("opens").clock(clock.clone()).lease_ms(5_000);
        let id = first.enqueue(Request::new(flow("dies"))).expect("asked for");
        let crashed = catch_unwind(AssertUnwindSafe(|| first.work("worker-a")));
        assert!(crashed.is_err(), "the worker went down");
        id
    };
    assert_eq!(sent.load(Ordering::SeqCst), 1);

    let reopened = Runtime::open(&db.0, catalog()).expect("opens again").clock(clock.clone()).lease_ms(5_000);
    assert_eq!(reopened.get(&id).expect("kept").status, RunStatus::Running);
    assert_eq!(reopened.work("worker-b").expect("claims"), None, "the dead worker's lease still holds");

    clock.advance(5_001);
    assert_eq!(reopened.work("worker-b").expect("claims").as_deref(), Some(id.as_str()));
    let record = reopened.get(&id).expect("kept");
    assert_eq!(record.status, RunStatus::Finished);
    assert_eq!(record.worker.as_deref(), Some("worker-b"));
    assert_eq!(sent.load(Ordering::SeqCst), 1, "the send that was over did not run again");
    let parts = reopened.parts(&id).expect("kept");
    assert_eq!(parts["after"].outputs["main"][0].json, json!({ "items": [1, 2] }));
}

#[test]
fn a_node_whose_result_no_one_knows_is_not_run_again_on_resuming() {
    let db = TempDb::new("unknown");
    let (sent, charged) = (Arc::new(AtomicU32::new(0)), Arc::new(AtomicU32::new(0)));
    let catalog = || Catalog::builtin().with(Send(sent.clone())).with(DiesUnknowing(charged.clone()));
    let clock = ManualClock::at(0);
    let id = {
        let first = Runtime::open(&db.0, catalog()).expect("opens").clock(clock.clone()).lease_ms(100);
        let id = first.enqueue(Request::new(flow("charge"))).expect("asked for");
        assert!(catch_unwind(AssertUnwindSafe(|| first.work("a"))).is_err());
        id
    };
    clock.advance(101);
    let reopened = Runtime::open(&db.0, catalog()).expect("opens").clock(clock.clone()).lease_ms(100);
    reopened.work("b").expect("runs");
    assert_eq!(charged.load(Ordering::SeqCst), 1, "not charged twice");
    let record = reopened.get(&id).expect("kept");
    assert_eq!(record.status, RunStatus::Stopped);
    assert_eq!(reopened.parts(&id).expect("kept")["risky"].status, NodeStatus::Uncertain);
}

/// Writes down which run it ran for, by the item's `n`.
struct Records(Arc<Mutex<Vec<i64>>>);
impl NodeKind for Records {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("records", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        for item in cx.input("main") {
            if let Some(n) = item.get("n").and_then(Value::as_i64) {
                self.0.lock().expect("not poisoned").push(n);
            }
        }
        Ok(main_port(cx.input("main").to_vec()))
    }
}

#[test]
fn two_workers_on_one_file_never_take_the_same_run() {
    let db = TempDb::new("two-workers");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let flow = FlowDefinition::new("count", "Count")
        .node(NodeDef::new("in", "manual"))
        .node(NodeDef::new("rec", "records"))
        .connect("in", "main", "rec", "main");
    {
        let asker = Runtime::open(&db.0, Catalog::builtin()).expect("opens");
        for n in 0..30 {
            asker
                .enqueue(Request::new(flow.clone()).starting("in", vec![Item::new(json!({ "n": n }))]))
                .expect("asked");
        }
    }
    let workers: Vec<_> = ["one", "two"]
        .into_iter()
        .map(|name| {
            let (path, seen) = (db.0.clone(), seen.clone());
            std::thread::spawn(move || {
                let runtime = Runtime::open(&path, Catalog::builtin().with(Records(seen))).expect("opens");
                runtime.work_until_idle(name).expect("works")
            })
        })
        .collect();
    let ran: usize = workers.into_iter().map(|w| w.join().expect("worked")).sum();
    assert_eq!(ran, 30);
    let mut seen = seen.lock().expect("not poisoned").clone();
    seen.sort_unstable();
    assert_eq!(seen, (0..30).collect::<Vec<_>>(), "every run once, none twice");
}

#[test]
fn a_lease_that_ran_out_puts_the_run_back_in_the_queue() {
    let clock = ManualClock::at(0);
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens").clock(clock.clone()).lease_ms(1_000);
    let id = runtime.enqueue(Request::new(simple())).expect("asked");
    assert_eq!(runtime.claim("a").expect("claims").as_deref(), Some(id.as_str()));
    assert_eq!(runtime.claim("b").expect("claims"), None);
    clock.advance(999);
    assert_eq!(runtime.claim("b").expect("claims"), None, "a still holds it");
    clock.advance(2);
    assert_eq!(runtime.claim("b").expect("claims").as_deref(), Some(id.as_str()));
    assert_eq!(runtime.get(&id).expect("kept").worker.as_deref(), Some("b"));
}

#[test]
fn a_wait_outlives_the_worker_and_the_run_goes_on_when_its_time_comes() {
    let db = TempDb::new("delay");
    let clock = ManualClock::at(10_000);
    let flow = FlowDefinition::new("later", "Later")
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "id": 7 }])))
        .node(NodeDef::new("pause", "delay").parameter("ms", json!(60_000)))
        .node(NodeDef::new("after", "join").parameter("field", json!("id")))
        .connect("start", "main", "pause", "main")
        .connect("pause", "main", "after", "main");
    let id = {
        let runtime = Runtime::open(&db.0, Catalog::builtin()).expect("opens").clock(clock.clone());
        let id = runtime.enqueue(Request::new(flow)).expect("asked");
        runtime.work("a").expect("runs");
        let record = runtime.get(&id).expect("kept");
        assert_eq!(record.status, RunStatus::Waiting);
        assert!(matches!(record.outcome, Some(RunOutcome::Waiting { ref node, .. }) if node == "pause"));
        id
    };
    let runtime = Runtime::open(&db.0, Catalog::builtin()).expect("opens again").clock(clock.clone());
    clock.advance(59_000);
    assert_eq!(runtime.work("b").expect("claims"), None, "not yet");
    clock.advance(1_000);
    assert_eq!(runtime.work("b").expect("claims").as_deref(), Some(id.as_str()));
    assert_eq!(runtime.get(&id).expect("kept").status, RunStatus::Finished);
    assert_eq!(runtime.parts(&id).expect("kept")["after"].outputs["main"][0].json, json!({ "items": [7] }));
}

#[test]
fn a_run_waiting_for_an_event_goes_on_when_it_is_told_and_gets_its_items() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens").clock(ManualClock::at(0));
    let flow = FlowDefinition::new("approve", "Approve")
        .node(NodeDef::new("start", "manual"))
        .node(NodeDef::new("ask", "wait_event").parameter("event", json!("approved")))
        .node(NodeDef::new("after", "join").parameter("field", json!("by")))
        .connect("start", "main", "ask", "main")
        .connect("ask", "main", "after", "main");
    let id = runtime.enqueue(Request::new(flow).by("ana")).expect("asked");
    runtime.work("a").expect("runs");
    assert_eq!(runtime.get(&id).expect("kept").status, RunStatus::Waiting);
    assert_eq!(runtime.work("a").expect("claims"), None, "nothing came yet");
    runtime.signal(&id, "rejected", vec![]).expect("told");
    assert_eq!(runtime.work("a").expect("claims"), None, "another event does not wake it");
    runtime.signal(&id, "approved", vec![Item::new(json!({ "by": "Bruno" }))]).expect("told");
    assert_eq!(runtime.work("a").expect("claims").as_deref(), Some(id.as_str()));
    let record = runtime.get(&id).expect("kept");
    assert_eq!(record.status, RunStatus::Finished);
    assert_eq!(record.actor.as_deref(), Some("ana"));
    assert_eq!(
        runtime.parts(&id).expect("kept")["after"].outputs["main"][0].json,
        json!({ "items": ["Bruno"] })
    );
}

#[test]
fn a_gate_that_keeps_its_items_lets_what_waited_go_on_with_the_approval() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens").clock(ManualClock::at(0));
    let flow = FlowDefinition::new("gate", "Gate")
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "order": 1 }, { "order": 2 }])))
        .node(
            NodeDef::new("ask", "wait_event")
                .parameter("event", json!("approved"))
                .parameter("keep", json!(true))
                .parameter("as", json!("approval")),
        )
        .connect("start", "main", "ask", "main");
    let id = runtime.enqueue(Request::new(flow)).expect("asked");
    runtime.work("a").expect("runs");
    assert_eq!(runtime.get(&id).expect("kept").status, RunStatus::Waiting);
    runtime.signal(&id, "approved", vec![Item::new(json!({ "by": "Bruno" }))]).expect("told");
    runtime.work_until_idle("a").expect("runs");
    let out = &runtime.parts(&id).expect("kept")["ask"].outputs["main"];
    let orders: Vec<_> = out.iter().map(|i| i.json.clone()).collect();
    assert_eq!(
        orders,
        [
            json!({ "order": 1, "approval": { "by": "Bruno" } }),
            json!({ "order": 2, "approval": { "by": "Bruno" } })
        ],
        "the orders that waited go on, each with the approval"
    );
    assert_eq!(out[0].from[0].node, "start", "and say where they came from");
}

#[test]
fn a_rerun_runs_the_original_or_the_current_revision_and_says_where_it_came_from() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens").clock(ManualClock::at(0));
    let first = FlowDefinition { revision: 1, ..simple() };
    let id = runtime
        .enqueue(Request::new(first.clone()).starting("start", vec![Item::new(json!({ "id": 9 }))]))
        .expect("asked");
    runtime.work_until_idle("a").expect("runs");

    let again = runtime.rerun(&id, Revision::Original).expect("asked again");
    let current = FlowDefinition { revision: 2, ..first };
    let newer = runtime.rerun(&id, Revision::Current(current)).expect("asked again");
    runtime.work_until_idle("a").expect("runs");
    for (rerun, revision) in [(&again, 1), (&newer, 2)] {
        let record = runtime.get(rerun).expect("kept");
        assert_eq!(record.origin.as_deref(), Some(id.as_str()));
        assert_eq!(record.revision, revision);
        assert_eq!(record.status, RunStatus::Finished);
        assert_eq!(
            runtime.parts(rerun).expect("kept")["after"].outputs["main"][0].json,
            json!({ "items": [9] })
        );
    }
    assert_eq!(runtime.runs_of("orders").expect("listed").len(), 3);
}

#[test]
fn a_retention_of_seven_days_forgets_older_runs_and_their_files_and_keeps_the_rest() {
    let clock = ManualClock::at(0);
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens").clock(clock.clone());
    let mut ids = Vec::new();
    for day in [0, 3, 8, 9] {
        clock.advance(day * DAY - clock.now_ms());
        let id = runtime.enqueue(Request::new(simple())).expect("asked");
        runtime.work_until_idle("a").expect("runs");
        assert_eq!(runtime.get(&id).expect("kept").status, RunStatus::Finished);
        runtime.put_blob(&id, "text/plain", b"a receipt").expect("kept");
        ids.push(id);
    }
    let file = runtime.put_blob(&ids[3], "text/plain", b"kept").expect("kept");
    clock.advance(10 * DAY - clock.now_ms());

    let pruned = runtime
        .prune(&Retention { older_than_ms: Some(7 * DAY as u64), ..Retention::default() })
        .expect("prunes");
    assert_eq!((pruned.runs, pruned.blobs), (2, 2), "the runs of day 0 and day 3, with their files");
    let left: BTreeSet<String> =
        runtime.runs_of("orders").expect("listed").into_iter().map(|r| r.id).collect();
    assert_eq!(left, BTreeSet::from([ids[2].clone(), ids[3].clone()]));
    assert_eq!(runtime.blob(&file).expect("reads").as_deref(), Some(&b"kept"[..]));
    assert!(runtime.get(&ids[0]).is_err());

    let kept = runtime.prune(&Retention { keep_last: Some(1), ..Retention::default() }).expect("prunes");
    assert_eq!(kept.runs, 1, "only the newest stays");
    assert_eq!(runtime.runs_of("orders").expect("listed")[0].id, ids[3]);
}

#[test]
fn a_flow_that_does_not_compile_ends_stopped_with_why() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let broken = FlowDefinition::new("broken", "Broken").node(NodeDef::new("x", "nowhere"));
    let id = runtime.enqueue(Request::new(broken)).expect("asked");
    runtime.work("a").expect("runs");
    let record = runtime.get(&id).expect("kept");
    assert_eq!(record.status, RunStatus::Stopped);
    assert!(
        matches!(record.outcome, Some(RunOutcome::Stopped { ref reason, .. }) if reason.contains("nowhere"))
    );
}

// The queue of #839: priorities, requests that join, cancelling and moving a run, and files made
// before any of it.

/// A flow of its own id, so each run is told apart by its flow.
fn named(id: &str) -> FlowDefinition {
    FlowDefinition::new(id, id)
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "id": 1 }])))
        .node(NodeDef::new("after", "join").parameter("field", json!("id")))
        .connect("start", "main", "after", "main")
}

#[test]
fn the_highest_priority_leaves_the_queue_first_and_the_oldest_among_equals() {
    let clock = ManualClock::at(0);
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens").clock(clock.clone());
    let old = runtime.enqueue(Request::new(named("old"))).expect("asked");
    clock.advance(1);
    let urgent = runtime.enqueue(Request::new(named("urgent")).priority(10)).expect("asked");
    clock.advance(1);
    let newer = runtime.enqueue(Request::new(named("newer"))).expect("asked");
    let line: Vec<String> = runtime.queue().expect("listed").into_iter().map(|r| r.id).collect();
    assert_eq!(line, [urgent.clone(), old.clone(), newer.clone()]);
    assert_eq!(runtime.claim("a").expect("claims"), Some(urgent));
    assert_eq!(runtime.claim("a").expect("claims"), Some(old));

    runtime.set_priority(&newer, 5).expect("moved");
    assert_eq!(runtime.get(&newer).expect("kept").priority, 5);
    assert!(runtime.set_priority("run-none", 1).is_err(), "there is no such run");
}

#[test]
fn requests_with_the_key_of_a_run_in_the_queue_join_it() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let first = runtime.enqueue(Request::new(named("site")).key("site@main")).expect("asked");
    let second = runtime.enqueue(Request::new(named("site")).key("site@main")).expect("asked");
    let third = runtime.enqueue(Request::new(named("site")).key("site@main").priority(3)).expect("asked");
    assert_eq!((second.as_str(), third.as_str()), (first.as_str(), first.as_str()));
    let record = runtime.get(&first).expect("kept");
    assert_eq!((record.joined, record.priority, record.key.as_deref()), (2, 3, Some("site@main")));
    assert_eq!(runtime.work_until_idle("a").expect("runs"), 1, "three requests, one run");

    // Once it left the queue, the same key makes a run of its own.
    let after = runtime.enqueue(Request::new(named("site")).key("site@main")).expect("asked");
    assert_ne!(after, first);
    // A rerun is asked for on purpose and never joins.
    let again = runtime.rerun(&first, Revision::Original).expect("asked");
    assert_ne!(again, after);
    assert_eq!(runtime.get(&again).expect("kept").priority, 3, "a rerun keeps the priority");
}

#[test]
fn a_run_cancelled_in_the_queue_ends_without_running() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let id = runtime.enqueue(Request::new(simple())).expect("asked");
    assert_eq!(runtime.cancel(&id, "no longer needed").expect("cancels"), RunStatus::Cancelled);
    assert_eq!(runtime.work_until_idle("a").expect("runs"), 0);
    let record = runtime.get(&id).expect("kept");
    assert!(
        matches!(record.outcome, Some(RunOutcome::Cancelled { ref reason }) if reason == "no longer needed")
    );
    assert!(record.ended_ms.is_some());
    assert!(runtime.queue().expect("listed").is_empty());
    // A run that ended stays as it ended.
    assert_eq!(runtime.cancel(&id, "again").expect("answers"), RunStatus::Cancelled);
}

/// Waits, a little at a time, until it is cancelled; says it started.
struct Busy(Arc<AtomicBool>);
impl NodeKind for Busy {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("busy", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        self.0.store(true, Ordering::SeqCst);
        cx.sleep(60_000)?;
        Ok(main_port(cx.input("main").to_vec()))
    }
}

#[test]
fn a_run_cancelled_while_it_runs_stops_its_node_through_the_token() {
    let started = Arc::new(AtomicBool::new(false));
    let runtime =
        Arc::new(Runtime::in_memory(Catalog::builtin().with(Busy(started.clone()))).expect("opens"));
    let flow = FlowDefinition::new("slow", "Slow")
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "id": 1 }])))
        .node(NodeDef::new("wait", "busy"))
        .connect("start", "main", "wait", "main");
    let id = runtime.enqueue(Request::new(flow)).expect("asked");
    let worker = {
        let runtime = runtime.clone();
        std::thread::spawn(move || runtime.work("a").expect("runs"))
    };
    let begun = std::time::Instant::now();
    while !started.load(Ordering::SeqCst) {
        assert!(begun.elapsed().as_secs() < 10, "the node started");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(runtime.cancel(&id, "stopped by Ana").expect("asks"), RunStatus::Running);
    worker.join().expect("the worker ends");
    assert!(begun.elapsed().as_secs() < 10, "it did not wait the minute out");
    let record = runtime.get(&id).expect("kept");
    assert_eq!(record.status, RunStatus::Cancelled);
}

#[test]
fn a_run_another_process_cancels_stops_at_its_next_step() {
    let db = TempDb::new("cancel-across");
    let catalog = Catalog::builtin().with(Send(Arc::new(AtomicU32::new(0))));
    let flow = FlowDefinition::new("two", "Two")
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "id": 1 }])))
        .node(NodeDef::new("send", "send"))
        .connect("start", "main", "send", "main");
    let here = Runtime::open(&db.0, catalog.clone()).expect("opens");
    let id = here.enqueue(Request::new(flow)).expect("asked");
    assert_eq!(here.claim("a").expect("claims"), Some(id.clone()));
    // The other process sees it running and asks; this one finds out when it writes a step.
    let there = Runtime::open(&db.0, catalog).expect("opens");
    assert_eq!(there.cancel(&id, "from the panel").expect("asks"), RunStatus::Running);
    assert_eq!(here.execute(&id, "a").expect("runs"), RunStatus::Cancelled);
}

#[test]
fn a_retention_for_one_flow_leaves_the_others_alone() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens");
    for flow in ["a", "a", "b", "b"] {
        runtime.enqueue(Request::new(named(flow))).expect("asked");
    }
    runtime.work_until_idle("w").expect("runs");
    let only_a = Retention { keep_last: Some(1), flow_id: Some("a".into()), ..Retention::default() };
    assert_eq!(runtime.prune(&only_a).expect("prunes").runs, 1);
    assert_eq!(runtime.runs_of("a").expect("listed").len(), 1);
    assert_eq!(runtime.runs_of("b").expect("listed").len(), 2);
}

#[test]
fn a_file_made_before_the_queue_had_priorities_opens_and_gets_the_columns() {
    let db = TempDb::new("old-file");
    {
        let conn = rusqlite::Connection::open(&db.0).expect("opens");
        conn.execute_batch(
            "CREATE TABLE runs (
                id TEXT PRIMARY KEY, flow_id TEXT NOT NULL, revision INTEGER NOT NULL, definition TEXT NOT NULL,
                mode TEXT NOT NULL, actor TEXT, start_node TEXT, input TEXT NOT NULL, status TEXT NOT NULL,
                outcome TEXT, origin TEXT, worker TEXT, lease_until INTEGER, wake_at INTEGER, wake_event TEXT,
                started_ms INTEGER, created_ms INTEGER NOT NULL, ended_ms INTEGER);",
        )
        .expect("the old table");
        conn.execute(
            "INSERT INTO runs (id, flow_id, revision, definition, mode, input, status, created_ms)
             VALUES ('run-old', 'orders', 1, ?1, 'active', '[]', 'queued', 0)",
            [simple().to_json()],
        )
        .expect("an old run");
    }
    let runtime = Runtime::open(&db.0, Catalog::builtin()).expect("opens the old file");
    let record = runtime.get("run-old").expect("kept");
    assert_eq!((record.priority, record.joined, record.key), (0, 0, None));
    assert_eq!(runtime.work_until_idle("a").expect("runs"), 1);
    assert_eq!(runtime.get("run-old").expect("kept").status, RunStatus::Finished);
}

#[test]
fn a_runtime_with_other_kinds_shares_the_queue_even_in_memory() {
    let sent = Arc::new(AtomicU32::new(0));
    let before = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let id = before.enqueue(Request::new(flow("send"))).expect("asked");
    let after = before.with_catalog(Catalog::builtin().with(Send(sent.clone())));
    assert_eq!(after.queue().expect("listed").len(), 1, "the run asked for before is there");
    after.work_until_idle("a").expect("runs");
    assert_eq!(before.get(&id).expect("kept").status, RunStatus::Finished);
    assert_eq!(sent.load(Ordering::SeqCst), 2, "both send nodes ran with the kind the new catalog has");
}

#[test]
fn a_host_fails_a_run_it_took_without_running_it_and_says_why() {
    let runtime = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let id = runtime.enqueue(Request::new(simple())).expect("enqueues");
    let taken = runtime.claim("w").expect("claims");
    assert_eq!(taken.as_deref(), Some(id.as_str()));
    let status = runtime.fail(&id, "w", "o checkout falhou").expect("fails");
    assert_eq!(status, RunStatus::Stopped);
    let record = runtime.get(&id).expect("kept");
    assert!(record.ended_ms.is_some());
    assert!(runtime.parts(&id).expect("parts").is_empty(), "no node ran");
    assert_eq!(runtime.work("w").expect("works"), None, "nothing left in the queue");
}
