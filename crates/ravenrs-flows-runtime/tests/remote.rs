//! A run worked in another process through a `Lease`: what goes out, what comes back, a worker
//! that dies and comes back, and one that no longer holds the run.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use ravenrs_flows::{
    CancelToken, Catalog, FlowDefinition, ManualClock, NodeContext, NodeDef, NodeError, NodeKind, NodeSpec,
    Ports, Reconciled, RunOutcome,
};
use ravenrs_flows_runtime::{Lease, Request, RunStatus, Runtime, StepRecord, StoreError};
use serde_json::json;

fn main_port(cx: &NodeContext<'_>) -> Ports {
    Ports::from([("main".to_owned(), cx.input("main").to_vec())])
}

/// Counts its runs; asked whether a run happened, says it did not.
struct Count(&'static str, Arc<AtomicU32>);
impl NodeKind for Count {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new(self.0, 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        self.1.fetch_add(1, Ordering::SeqCst);
        Ok(main_port(cx))
    }
    fn reconcile(&self, _: &NodeContext<'_>) -> Reconciled {
        Reconciled::NotDone
    }
}

fn flow() -> FlowDefinition {
    FlowDefinition::new("build", "Build")
        .node(NodeDef::new("start", "manual").parameter("items", json!([{ "n": 1 }])))
        .node(NodeDef::new("compile", "compile"))
        .node(NodeDef::new("test", "test"))
        .connect("start", "main", "compile", "main")
        .connect("compile", "main", "test", "main")
}

/// The lease, as it is after crossing the wire.
fn over_the_wire(lease: &Lease) -> Lease {
    serde_json::from_str(&serde_json::to_string(lease).expect("to JSON")).expect("from JSON")
}

#[test]
fn a_run_worked_elsewhere_is_written_step_by_step_and_ends_on_the_host() {
    let host = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let id = host.enqueue(Request::new(flow())).expect("asked for");
    assert_eq!(host.due().expect("due"), vec![id.clone()]);
    assert!(host.claim_id(&id, "agent/a/1").expect("claims"));
    assert!(!host.claim_id(&id, "agent/b/1").expect("asks"), "two workers never take the same run");
    assert!(host.due().expect("due").is_empty());

    let (compiled, tested) = (Arc::new(AtomicU32::new(0)), Arc::new(AtomicU32::new(0)));
    let catalog =
        Catalog::builtin().with(Count("compile", compiled.clone())).with(Count("test", tested.clone()));
    let lease = over_the_wire(&host.lease(&id, "agent/a/1").expect("leased"));
    assert!(lease.settled.is_empty());
    let mut records = Vec::new();
    let run = lease
        .run(&catalog, Arc::new(ManualClock::at(0)), CancelToken::new(), |step| records.push(step))
        .expect("compiles");
    assert_eq!(run.outcome, RunOutcome::Finished);
    for record in &records {
        let record: StepRecord =
            serde_json::from_str(&serde_json::to_string(record).expect("to JSON")).expect("from JSON");
        assert!(host.record(&id, "agent/a/1", &record).expect("written").held);
    }
    assert_eq!(host.finish(&id, "agent/a/1", &run.outcome).expect("ends"), RunStatus::Finished);
    assert_eq!(host.parts(&id).expect("parts").len(), 3);
    assert_eq!((compiled.load(Ordering::SeqCst), tested.load(Ordering::SeqCst)), (1, 1));
}

#[test]
fn a_worker_that_died_comes_back_to_its_run_from_the_last_step_it_sent() {
    let host = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let id = host.enqueue(Request::new(flow())).expect("asked for");
    assert!(host.claim_id(&id, "agent/a/1").expect("claims"));
    let (compiled, tested) = (Arc::new(AtomicU32::new(0)), Arc::new(AtomicU32::new(0)));
    let catalog =
        Catalog::builtin().with(Count("compile", compiled.clone())).with(Count("test", tested.clone()));

    // The first life: everything until `test` started reaches the host, then the worker dies.
    let lease = host.lease(&id, "agent/a/1").expect("leased");
    let mut records = Vec::new();
    let _ = lease.run(&catalog, Arc::new(ManualClock::at(0)), CancelToken::new(), |step| records.push(step));
    let cut = records
        .iter()
        .position(|r| *r == StepRecord::Started { node: "test".to_owned() })
        .expect("test started");
    for record in &records[..=cut] {
        host.record(&id, "agent/a/1", record).expect("written");
    }

    // The second life, with the same name: the host knows what it held, and the lease starts
    // where the host was left.
    assert_eq!(host.held_by("agent/a/1").expect("held"), vec![id.clone()]);
    let lease = over_the_wire(&host.lease(&id, "agent/a/1").expect("leased again"));
    let settled: Vec<&str> = lease.settled.iter().map(|(node, _)| node.as_str()).collect();
    assert_eq!(settled, ["start", "compile"]);
    assert!(lease.in_progress.contains("test"));
    let mut again = Vec::new();
    let run = lease
        .run(&catalog, Arc::new(ManualClock::at(0)), CancelToken::new(), |step| again.push(step))
        .expect("compiles");
    for record in &again {
        host.record(&id, "agent/a/1", record).expect("written");
    }
    assert_eq!(host.finish(&id, "agent/a/1", &run.outcome).expect("ends"), RunStatus::Finished);
    assert_eq!(compiled.load(Ordering::SeqCst), 1, "compile ran in the first life and not again");
    assert_eq!(tested.load(Ordering::SeqCst), 2, "test was asked, said it did not happen, and ran again");
    assert_eq!(host.parts(&id).expect("parts")["test"].status, ravenrs_flows::NodeStatus::Done);
}

#[test]
fn a_worker_that_no_longer_holds_the_run_writes_nothing_and_cannot_end_it() {
    let clock = ManualClock::at(1_000);
    let host = Runtime::in_memory(Catalog::builtin()).expect("opens").clock(clock.clone()).lease_ms(5_000);
    let id = host.enqueue(Request::new(flow())).expect("asked for");
    assert!(host.claim_id(&id, "agent/a/1").expect("claims"));
    assert!(host.renew(&id, "agent/a/1").expect("renews").held);

    clock.advance(5_001);
    assert_eq!(host.due().expect("due"), vec![id.clone()], "a lease that ran out makes the run due");
    assert!(host.claim_id(&id, "agent/b/1").expect("claims"));

    let late = StepRecord::Started { node: "compile".to_owned() };
    assert!(!host.record(&id, "agent/a/1", &late).expect("asked").held);
    assert!(host.parts(&id).expect("parts").is_empty());
    assert!(matches!(host.lease(&id, "agent/a/1"), Err(StoreError::NotHeld(_))));
    assert!(matches!(host.finish(&id, "agent/a/1", &RunOutcome::Finished), Err(StoreError::NotHeld(_))));
    assert_eq!(host.get(&id).expect("kept").status, RunStatus::Running);
}

#[test]
fn cancelling_reaches_a_worker_elsewhere_with_its_next_step() {
    let host = Runtime::in_memory(Catalog::builtin()).expect("opens");
    let id = host.enqueue(Request::new(flow())).expect("asked for");
    assert!(host.claim_id(&id, "agent/a/1").expect("claims"));
    host.cancel(&id, "pedido à mão").expect("asked to cancel");
    let held = host.renew(&id, "agent/a/1").expect("renews");
    assert_eq!(held.cancel.as_deref(), Some("pedido à mão"));
    let lease = host.lease(&id, "agent/a/1").expect("leased");
    assert_eq!(lease.cancel_requested.as_deref(), Some("pedido à mão"));
    let catalog =
        Catalog::builtin().with(Count("compile", Arc::default())).with(Count("test", Arc::default()));
    let run =
        lease.run(&catalog, Arc::new(ManualClock::at(0)), CancelToken::new(), |_| {}).expect("compiles");
    assert_eq!(host.finish(&id, "agent/a/1", &run.outcome).expect("ends"), RunStatus::Cancelled);
}
