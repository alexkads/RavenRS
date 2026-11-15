//! The acceptance of #536: cancelling, time limits and trying again, with no window. Waits and
//! retries run on a `ManualClock`, which moves instead of waiting; cancelling and time limits
//! that must stop a node blocking a thread use real time, a fraction of a second.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use ravenrs_flows::nodes::Subflow;
use ravenrs_flows::{
    CancelToken, Catalog, Clock, ERROR_PORT, ErrorPolicy, Executor, Failure, FlowDefinition, Item,
    ManualClock, NodeContext, NodeDef, NodeError, NodeKind, NodeSpec, NodeStatus, Plan, Ports, Reconciled,
    Retry, RunOutcome, SystemClock,
};
use serde_json::json;

fn main_port(items: Vec<Item>) -> Ports {
    Ports::from([("main".to_owned(), items)])
}

fn start() -> NodeDef {
    NodeDef::new("start", "manual").parameter("items", json!([{ "id": 1 }, { "id": 2 }]))
}

/// Waits as `wait` does, a little at a time on the run's clock, until it is stopped; then says
/// so, so the test can see the wait inside the subflow end.
struct Patient(Arc<AtomicBool>);
impl NodeKind for Patient {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("patient", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let stopped = loop {
            if let Err(e) = cx.sleep(10) {
                break e;
            }
        };
        self.0.store(true, Ordering::SeqCst);
        Err(stopped)
    }
}

#[test]
fn cancelling_a_run_ends_its_subflow_and_the_wait_inside_and_says_cancelled() {
    let stopped = Arc::new(AtomicBool::new(false));
    let inner_catalog = Catalog::builtin().with(Patient(stopped.clone()));
    let inner = FlowDefinition::new("inner", "Inner")
        .node(NodeDef::new("in", "manual"))
        .node(NodeDef::new("waiting", "patient"))
        .connect("in", "main", "waiting", "main");
    let catalog = Catalog::builtin().with(Subflow::new([inner], inner_catalog));
    let outer = FlowDefinition::new("outer", "Outer")
        .node(start())
        .node(NodeDef::new("call", "subflow").parameter("flow", json!("inner")))
        .connect("start", "main", "call", "main");
    let plan = Plan::compile(&outer, &catalog).expect("compiles");

    let token = CancelToken::new();
    let stopper = token.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        stopper.cancel("the person stopped it");
    });
    let began = Instant::now();
    let run = Executor::new(&plan, &catalog).clock(SystemClock).cancel_with(token).run();

    assert!(began.elapsed() < Duration::from_secs(5), "it did not wait the minute");
    assert_eq!(run.outcome, RunOutcome::Cancelled { reason: "the person stopped it".into() });
    assert_eq!(run.status("call"), Some(NodeStatus::Cancelled));
    let deadline = Instant::now() + Duration::from_secs(2);
    while !stopped.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(stopped.load(Ordering::SeqCst), "the node inside the subflow saw it was cancelled");
}

/// A call that hangs, looking at nothing.
struct Hangs;
impl NodeKind for Hangs {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("hangs", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        std::thread::sleep(Duration::from_secs(5));
        Ok(main_port(cx.input("main").to_vec()))
    }
}

#[test]
fn a_connector_slower_than_its_time_limit_is_left_and_marked_timed_out() {
    let catalog = Catalog::builtin().with(Hangs);
    let flow = |policy: ErrorPolicy| {
        FlowDefinition::new("f", "slow")
            .node(start())
            .node(NodeDef::new("call", "hangs").timeout_ms(100).on_error(policy))
            .node(NodeDef::new("log", "join").parameter("field", json!("error")))
            .connect("start", "main", "call", "main")
            .connect("call", ERROR_PORT, "log", "main")
    };
    let plan = Plan::compile(&flow(ErrorPolicy::Stop), &catalog).expect("compiles");
    let began = Instant::now();
    let run = Executor::new(&plan, &catalog).run();
    assert!(began.elapsed() < Duration::from_secs(2), "the run did not wait the five seconds");
    assert_eq!(run.status("call"), Some(NodeStatus::TimedOut));
    assert!(run.nodes["call"].error.as_deref().is_some_and(|e| e.contains("more than")));
    assert!(matches!(run.outcome, RunOutcome::Stopped { ref node, .. } if node == "call"));

    let plan = Plan::compile(&flow(ErrorPolicy::Route), &catalog).expect("compiles");
    let routed = Executor::new(&plan, &catalog).run();
    assert!(routed.succeeded());
    assert_eq!(routed.output("log", "main")[0].json["items"].as_array().map(Vec::len), Some(2));
}

/// Fails until its try `works_at`, in the way it is told.
struct Flaky {
    tries: Arc<AtomicU32>,
    works_at: u32,
    failure: Failure,
}
impl NodeKind for Flaky {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("flaky", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let n = self.tries.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(cx.attempt(), n);
        if n >= self.works_at {
            return Ok(main_port(cx.input("main").to_vec()));
        }
        Err(NodeError { failure: self.failure, ..NodeError::new(format!("try {n} failed")) })
    }
}

fn flaky_run(failure: Failure, works_at: u32, retry: Retry) -> (u32, ravenrs_flows::Run, i64) {
    let tries = Arc::new(AtomicU32::new(0));
    let catalog = Catalog::builtin().with(Flaky { tries: tries.clone(), works_at, failure });
    let flow = FlowDefinition::new("f", "flaky")
        .node(start())
        .node(NodeDef::new("call", "flaky").retry(retry))
        .connect("start", "main", "call", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let clock = ManualClock::at(0);
    let run = Executor::new(&plan, &catalog).clock(clock.clone()).run();
    (tries.load(Ordering::SeqCst), run, clock.now_ms())
}

#[test]
fn a_failure_worth_retrying_runs_three_times_with_the_waits_between_and_a_lasting_one_once() {
    let (tries, run, waited) = flaky_run(Failure::Retryable, 3, Retry::new(3, 100).backoff(2.0));
    assert_eq!(tries, 3);
    assert_eq!(run.status("call"), Some(NodeStatus::Done));
    assert_eq!(run.nodes["call"].attempts, 3);
    assert_eq!(waited, 300, "100 ms, then 200 ms");

    let (tries, run, _) = flaky_run(Failure::Retryable, 9, Retry::new(3, 100));
    assert_eq!(tries, 3);
    assert_eq!(run.status("call"), Some(NodeStatus::Failed));
    assert_eq!(run.nodes["call"].error.as_deref(), Some("try 3 failed"));

    let (tries, run, waited) = flaky_run(Failure::Permanent, 3, Retry::new(3, 100));
    assert_eq!(tries, 1, "trying again would not help");
    assert_eq!(run.status("call"), Some(NodeStatus::Failed));
    assert_eq!(waited, 0);
}

/// A charge whose answer is lost, and what looking it up finds.
struct Charge {
    tries: Arc<AtomicU32>,
    found: Reconciled,
}
impl NodeKind for Charge {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("charge", 1).input("main").output("main")
    }
    fn execute(&self, _: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let n = self.tries.fetch_add(1, Ordering::SeqCst) + 1;
        if n == 1 {
            Err(NodeError::unknown("the bank did not answer"))
        } else {
            Ok(main_port(vec![Item::new(json!({ "paid": n }))]))
        }
    }
    fn reconcile(&self, _: &NodeContext<'_>) -> Reconciled {
        self.found.clone()
    }
}

fn charge_run(found: Reconciled) -> (u32, ravenrs_flows::Run) {
    let tries = Arc::new(AtomicU32::new(0));
    let catalog = Catalog::builtin().with(Charge { tries: tries.clone(), found });
    let flow = FlowDefinition::new("f", "charge")
        .node(start())
        .node(NodeDef::new("pay", "charge").retry(Retry::new(3, 10)).on_error(ErrorPolicy::Continue))
        .connect("start", "main", "pay", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).clock(ManualClock::at(0)).run();
    (tries.load(Ordering::SeqCst), run)
}

#[test]
fn a_result_no_one_knows_is_not_repeated_until_reconciling_says_it_did_not_happen() {
    let (tries, run) = charge_run(Reconciled::Unknown);
    assert_eq!(tries, 1, "not charged twice");
    assert_eq!(run.status("pay"), Some(NodeStatus::Uncertain));

    let (tries, run) = charge_run(Reconciled::NotDone);
    assert_eq!(tries, 2, "it did not happen, so it was tried again");
    assert_eq!(run.output("pay", "main")[0].get("paid"), Some(&json!(2)));

    let found = Ports::from([("main".to_owned(), vec![Item::new(json!({ "paid": "found" }))])]);
    let (tries, run) = charge_run(Reconciled::Done(found));
    assert_eq!(tries, 1);
    assert!(run.nodes["pay"].reconciled);
    assert_eq!(run.output("pay", "main")[0].get("paid"), Some(&json!("found")));
}

#[test]
fn a_run_longer_than_its_limit_ends_timed_out_on_the_test_clock() {
    let catalog = Catalog::builtin();
    let flow = FlowDefinition::new("f", "long")
        .node(start())
        .node(NodeDef::new("wait", "wait").parameter("ms", json!(10_000)))
        .node(NodeDef::new("after", "join"))
        .connect("start", "main", "wait", "main")
        .connect("wait", "main", "after", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let clock = ManualClock::at(1_000);
    let began = Instant::now();
    let run = Executor::new(&plan, &catalog).clock(clock.clone()).time_limit_ms(500).run();
    assert!(began.elapsed() < Duration::from_secs(2));
    assert_eq!(run.outcome, RunOutcome::TimedOut { reason: "the run took more than 500 ms".into() });
    assert_eq!(run.status("wait"), Some(NodeStatus::TimedOut));
    assert_eq!(run.status("after"), None);
    assert!(clock.now_ms() <= 1_520, "the wait stopped at the limit, not after ten seconds");
}

#[test]
fn a_wait_of_a_minute_ends_as_soon_as_the_run_is_cancelled() {
    let catalog = Catalog::builtin();
    let flow = FlowDefinition::new("f", "wait")
        .node(start())
        .node(NodeDef::new("wait", "wait").parameter("ms", json!(60_000)))
        .connect("start", "main", "wait", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let token = CancelToken::new();
    let stopper = token.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        stopper.cancel("enough");
    });
    let began = Instant::now();
    let run = Executor::new(&plan, &catalog).clock(SystemClock).cancel_with(token).run();
    assert!(began.elapsed() < Duration::from_secs(2));
    assert_eq!(run.status("wait"), Some(NodeStatus::Cancelled));
    assert_eq!(run.outcome, RunOutcome::Cancelled { reason: "enough".into() });
}

#[test]
fn a_wait_waits_on_the_run_clock_and_passes_its_items_on() {
    let catalog = Catalog::builtin();
    let flow = FlowDefinition::new("f", "wait")
        .node(start())
        .node(NodeDef::new("wait", "wait").parameter("ms", json!(250)))
        .connect("start", "main", "wait", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let clock = ManualClock::at(0);
    let run = Executor::new(&plan, &catalog).clock(clock.clone()).run();
    assert!(run.succeeded());
    assert_eq!(run.output("wait", "main").len(), 2);
    assert_eq!(clock.now_ms(), 250);
}

#[test]
fn a_subflow_runs_with_the_items_that_came_and_gives_what_it_ends_with() {
    let inner = FlowDefinition::new("double", "Double")
        .node(NodeDef::new("in", "manual"))
        .node(NodeDef::new("x2", "set").parameter("fields", json!({ "id": "[id] * 2" })))
        .connect("in", "main", "x2", "main");
    let catalog = Catalog::builtin().with(Subflow::new([inner], Catalog::builtin()));
    let flow = FlowDefinition::new("f", "outer")
        .node(start())
        .node(NodeDef::new("call", "subflow").parameter("flow", json!("double")))
        .node(
            NodeDef::new("missing", "subflow")
                .parameter("flow", json!("nope"))
                .on_error(ErrorPolicy::Continue),
        )
        .connect("start", "main", "call", "main")
        .connect("start", "main", "missing", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).clock(ManualClock::at(0)).run();
    let ids: Vec<_> = run.output("call", "main").iter().map(|i| i.json["id"].clone()).collect();
    assert_eq!(ids, [json!(2), json!(4)]);
    assert!(run.nodes["missing"].error.as_deref().is_some_and(|e| e.contains("nope")));
}

#[test]
fn a_child_token_stops_with_its_parent_and_the_first_reason_stays() {
    let parent = CancelToken::new();
    let child = parent.child();
    assert!(!child.is_cancelled());
    child.cancel("only the child");
    assert!(child.is_cancelled() && !parent.is_cancelled());

    let other = parent.child();
    parent.cancel("everything");
    parent.cancel("said twice");
    assert!(other.is_cancelled());
    assert_eq!(other.reason().as_deref(), Some("everything"));
    assert_eq!(child.reason().as_deref(), Some("only the child"));
}

#[test]
fn the_wait_before_each_try_grows_by_the_backoff() {
    let retry = Retry::new(4, 100).backoff(3.0);
    assert_eq!([retry.wait_before(2), retry.wait_before(3), retry.wait_before(4)], [100, 300, 900]);
    assert_eq!(Retry::new(0, 5).tries, 1, "at least the first try");
    let json = serde_json::to_string(&NodeDef::new("n", "k").retry(retry).timeout_ms(50)).expect("writes");
    let back: NodeDef = serde_json::from_str(&json).expect("reads");
    assert_eq!(back.retry, Some(retry));
    assert_eq!(back.timeout_ms, Some(50));
}
