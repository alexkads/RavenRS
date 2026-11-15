//! The acceptance of #525: a flow compiled against a catalog and run in memory, with no window.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ravenrs_flows::{
    Catalog, ErrorPolicy, Executor, FixedClock, FlowDefinition, Item, ItemRef, Mode, NodeContext, NodeDef,
    NodeError, NodeKind, NodeSpec, NodeStatus, Plan, PlanError, Ports, Run, RunError,
};
use ravenrs_graphics::flow::{FlowGate, FlowNode, FlowNodeStatus, FlowRun};
use serde_json::{Value, json};

fn main_port(items: Vec<Item>) -> Ports {
    Ports::from([("main".to_owned(), items)])
}

/// Starts with the items of its parameter `items`.
struct Manual;
impl NodeKind for Manual {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("manual", 1).title("Manual start").output("main").trigger()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let items = cx.parameter("items").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(main_port(items.into_iter().map(Item::new).collect()))
    }
}

/// Makes a new item from each, with `seen: true` added.
struct Set;
impl NodeKind for Set {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("set", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let made = cx
            .input("main")
            .iter()
            .map(|item| {
                let mut json = item.json.clone();
                json["seen"] = json!(true);
                item.derive(json)
            })
            .collect();
        Ok(main_port(made))
    }
}

/// Keeps the items whose `field` is `equals`, unchanged.
struct Filter;
impl NodeKind for Filter {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("filter", 1).input("main").output("main").requires("field").requires("equals")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let field = cx.parameter("field").and_then(Value::as_str).unwrap_or_default();
        let equals = cx.parameter("equals").cloned().unwrap_or(Value::Null);
        Ok(main_port(cx.input("main").iter().filter(|i| i.get(field) == Some(&equals)).cloned().collect()))
    }
}

/// Passes its items on and counts how many times it ran.
struct Counter(Arc<AtomicUsize>);
impl NodeKind for Counter {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("counter", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(main_port(cx.input("main").to_vec()))
    }
}

/// Always fails.
struct Broken;
impl NodeKind for Broken {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("broken", 1).input("main").output("main")
    }
    fn execute(&self, _: &NodeContext<'_>) -> Result<Ports, NodeError> {
        Err(NodeError::new("the service said no"))
    }
}

/// A loop head: it may be fed back from after it.
struct Again;
impl NodeKind for Again {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("again", 1).input("main").input("back").output("main").loops()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        Ok(main_port(cx.input("main").to_vec()))
    }
}

/// A branch: true and false.
struct Branch;
impl NodeKind for Branch {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("branch", 1).input("main").output("true").output("false").chooses()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let (yes, no): (Vec<Item>, Vec<Item>) =
            cx.input("main").iter().cloned().partition(|i| i.get("paid") == Some(&json!(true)));
        Ok(Ports::from([("true".to_owned(), yes), ("false".to_owned(), no)]))
    }
}

/// Answers on a port its kind does not have.
struct Stray;
impl NodeKind for Stray {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("stray", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        Ok(Ports::from([("elsewhere".to_owned(), cx.input("main").to_vec())]))
    }
}

/// Gives the run's clock.
struct Clocked;
impl NodeKind for Clocked {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("clocked", 1).input("main").output("main")
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        Ok(main_port(vec![Item::new(json!({ "now": cx.now_ms() }))]))
    }
}

fn catalog() -> Catalog {
    Catalog::new()
        .with(Manual)
        .with(Set)
        .with(Filter)
        .with(Broken)
        .with(Again)
        .with(Branch)
        .with(Stray)
        .with(Clocked)
}

fn three_orders() -> NodeDef {
    NodeDef::new("start", "manual").parameter(
        "items",
        json!([{ "id": "a", "paid": true }, { "id": "b", "paid": true }, { "id": "c", "paid": false }]),
    )
}

fn paid_filter() -> NodeDef {
    NodeDef::new("filter", "filter").parameter("field", json!("paid")).parameter("equals", json!(true))
}

fn run(flow: &FlowDefinition, catalog: &Catalog) -> Run {
    let plan = Plan::compile(flow, catalog).expect("the flow compiles");
    Executor::new(&plan, catalog).clock(FixedClock(1_000)).run()
}

#[test]
fn three_items_through_set_and_filter_leave_two_each_tracing_its_input() {
    let flow = FlowDefinition::new("f", "Paid")
        .node(three_orders())
        .node(NodeDef::new("set", "set"))
        .node(paid_filter())
        .connect("start", "main", "set", "main")
        .connect("set", "main", "filter", "main");
    let run = run(&flow, &catalog());

    let kept = run.output("filter", "main");
    assert_eq!(kept.len(), 2);
    for (i, item) in kept.iter().enumerate() {
        assert_eq!(item.at, Some(ItemRef::new("filter", "main", i)));
        assert_eq!(item.from, vec![ItemRef::new("set", "main", i)], "the filter passed the set's item on");
        assert_eq!(item.get("seen"), Some(&json!(true)));
    }
    let made = run.output("set", "main");
    assert_eq!(made[2].from, vec![ItemRef::new("start", "main", 2)], "the set made it from the start's item");
    assert_eq!(run.order, ["start", "set", "filter"]);
    assert!(run.succeeded());
}

#[test]
fn a_kind_the_catalog_lacks_does_not_compile_and_the_error_names_node_and_kind() {
    let flow =
        FlowDefinition::new("f", "x").node(three_orders()).node(NodeDef::new("mail", "smtp").version(2));
    let errors = Plan::compile(&flow, &catalog()).expect_err("smtp is not in the catalog");
    assert_eq!(errors, [PlanError::UnknownKind { node: "mail".into(), kind: "smtp".into(), version: 2 }]);
    let said = errors[0].to_string();
    assert!(said.contains("mail") && said.contains("smtp"), "{said}");
}

#[test]
fn a_cycle_with_no_loop_node_is_refused_with_its_path_and_with_one_it_compiles() {
    let round = |kind: &str| {
        FlowDefinition::new("f", "round")
            .node(three_orders())
            .node(NodeDef::new("head", kind))
            .node(NodeDef::new("work", "set"))
            .connect("start", "main", "head", "main")
            .connect("head", "main", "work", "main")
            .connect("work", "main", "head", if kind == "again" { "back" } else { "main" })
    };
    let errors = Plan::compile(&round("set"), &catalog()).expect_err("it goes round");
    assert_eq!(errors, [PlanError::Cycle { path: vec!["head".into(), "work".into(), "head".into()] }]);
    assert!(errors[0].to_string().contains("head → work → head"));

    let plan = Plan::compile(&round("again"), &catalog()).expect("a loop node closes it");
    assert_eq!(plan.order(), ["start", "head", "work"]);
    assert_eq!(plan.back_connections().len(), 1);
    assert_eq!(plan.back_connections()[0].from.node, "work");
}

#[test]
fn a_second_cycle_beside_the_loop_is_still_refused() {
    let flow = FlowDefinition::new("f", "two rounds")
        .node(three_orders())
        .node(NodeDef::new("head", "again"))
        .node(NodeDef::new("a", "set"))
        .node(NodeDef::new("b", "set"))
        .connect("start", "main", "head", "main")
        .connect("head", "main", "a", "main")
        .connect("a", "main", "b", "main")
        .connect("b", "main", "a", "main")
        .connect("b", "main", "head", "back");
    let errors = Plan::compile(&flow, &catalog()).expect_err("a and b go round without the loop");
    assert_eq!(errors, [PlanError::Cycle { path: vec!["a".into(), "b".into(), "a".into()] }]);
}

#[test]
fn a_run_from_a_node_reuses_what_the_node_before_gave_and_does_not_run_it_again() {
    let count = Arc::new(AtomicUsize::new(0));
    let catalog = catalog().with(Counter(count.clone()));
    let flow = FlowDefinition::new("f", "partial")
        .node(three_orders())
        .node(NodeDef::new("a", "counter"))
        .node(paid_filter())
        .connect("start", "main", "a", "main")
        .connect("a", "main", "filter", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let executor = Executor::new(&plan, &catalog).clock(FixedClock(0));
    let first = executor.run();
    assert_eq!(count.load(Ordering::SeqCst), 1);

    let again = executor.run_from("filter", &first).expect("a ran before");
    assert_eq!(count.load(Ordering::SeqCst), 1, "a did not run again");
    assert_eq!(again.status("a"), Some(NodeStatus::Reused));
    assert_eq!(again.status("filter"), Some(NodeStatus::Done));
    assert_eq!(again.output("filter", "main"), first.output("filter", "main"));
    assert_eq!(again.status("start"), None, "only what feeds the run is brought along");

    assert_eq!(
        executor.run_from("filter", &Run::default()),
        Err(RunError::NoPrevious { node: "a".into() }),
        "with no earlier run, a's items are not there"
    );
}

#[test]
fn one_node_alone_runs_with_what_reached_it_and_nothing_after_it() {
    let flow = FlowDefinition::new("f", "alone")
        .node(three_orders())
        .node(NodeDef::new("set", "set"))
        .node(paid_filter())
        .connect("start", "main", "set", "main")
        .connect("set", "main", "filter", "main");
    let catalog = catalog();
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let executor = Executor::new(&plan, &catalog).clock(FixedClock(0));
    let first = executor.run();
    let alone = executor.run_node("set", &first).expect("start ran before");
    assert_eq!(alone.order, ["start", "set"]);
    assert_eq!(alone.status("filter"), None);
    assert_eq!(alone.output("set", "main").len(), 3);
    assert_eq!(executor.run_node("nope", &first), Err(RunError::UnknownNode { node: "nope".into() }));
}

#[test]
fn pinned_items_are_used_in_a_test_and_ignored_when_the_flow_works() {
    let flow = FlowDefinition::new("f", "pinned")
        .node(three_orders().pin("main", vec![Item::new(json!({ "id": "fixed", "paid": true }))]))
        .node(paid_filter())
        .connect("start", "main", "filter", "main");
    let catalog = catalog();
    let plan = Plan::compile(&flow, &catalog).expect("compiles");

    let test = Executor::new(&plan, &catalog).mode(Mode::Test).clock(FixedClock(0)).run();
    assert_eq!(test.status("start"), Some(NodeStatus::Pinned));
    assert_eq!(test.output("filter", "main").len(), 1);
    assert_eq!(test.output("filter", "main")[0].get("id"), Some(&json!("fixed")));

    let working = Executor::new(&plan, &catalog).clock(FixedClock(0)).run();
    assert_eq!(working.status("start"), Some(NodeStatus::Done));
    assert_eq!(working.output("filter", "main").len(), 2);
}

#[test]
fn a_disabled_node_passes_its_first_input_on_unchanged() {
    let flow = FlowDefinition::new("f", "disabled")
        .node(three_orders())
        .node(paid_filter().disabled(true))
        .connect("start", "main", "filter", "main");
    let run = run(&flow, &catalog());
    assert_eq!(run.status("filter"), Some(NodeStatus::Disabled));
    assert_eq!(run.output("filter", "main").len(), 3);
    assert_eq!(run.output("filter", "main")[1].from, vec![ItemRef::new("start", "main", 1)]);
}

#[test]
fn a_failure_stops_the_run_unless_the_node_goes_on() {
    let failing = |policy: ErrorPolicy| {
        FlowDefinition::new("f", "failing")
            .node(three_orders())
            .node(NodeDef::new("call", "broken").on_error(policy))
            .node(NodeDef::new("set", "set"))
            .connect("start", "main", "call", "main")
            .connect("call", "main", "set", "main")
    };
    let stopped = run(&failing(ErrorPolicy::Stop), &catalog());
    assert_eq!(stopped.stopped_at.as_deref(), Some("call"));
    assert_eq!(stopped.nodes["call"].error.as_deref(), Some("the service said no"));
    assert_eq!(stopped.status("set"), None);

    let going_on = run(&failing(ErrorPolicy::Continue), &catalog());
    assert!(going_on.succeeded());
    assert_eq!(going_on.status("call"), Some(NodeStatus::Failed));
    assert_eq!(going_on.status("set"), Some(NodeStatus::Skipped), "nothing reached it");
}

#[test]
fn an_answer_on_a_port_the_kind_lacks_is_a_failure() {
    let flow = FlowDefinition::new("f", "stray")
        .node(three_orders())
        .node(NodeDef::new("s", "stray"))
        .connect("start", "main", "s", "main");
    let run = run(&flow, &catalog());
    assert_eq!(run.status("s"), Some(NodeStatus::Failed));
    assert!(run.nodes["s"].error.as_deref().is_some_and(|e| e.contains("elsewhere")));
}

#[test]
fn a_branch_feeds_only_the_way_its_items_took() {
    let flow = FlowDefinition::new("f", "branch")
        .node(three_orders())
        .node(NodeDef::new("if", "branch"))
        .node(NodeDef::new("yes", "set"))
        .node(NodeDef::new("no", "set"))
        .connect("start", "main", "if", "main")
        .connect("if", "true", "yes", "main")
        .connect("if", "false", "no", "main");
    let run = run(&flow, &catalog());
    assert_eq!(run.output("yes", "main").len(), 2);
    assert_eq!(run.output("no", "main").len(), 1);
}

#[test]
fn the_same_flow_and_clock_give_the_same_run() {
    let flow = FlowDefinition::new("f", "clock")
        .node(three_orders())
        .node(NodeDef::new("t", "clocked"))
        .connect("start", "main", "t", "main");
    let (one, two) = (run(&flow, &catalog()), run(&flow, &catalog()));
    assert_eq!(one, two);
    assert_eq!(one.output("t", "main")[0].get("now"), Some(&json!(1_000)));
    assert_eq!(one.started_ms, 1_000);
}

#[test]
fn what_is_wrong_with_a_flow_is_said_all_at_once() {
    let flow = FlowDefinition::new("f", "wrong")
        .node(three_orders())
        .node(NodeDef::new("filter", "filter"))
        .node(NodeDef::new("filter", "set"))
        .connect("start", "out", "filter", "main")
        .connect("start", "main", "ghost", "main");
    let errors = Plan::compile(&flow, &catalog()).expect_err("several things are wrong");
    assert!(errors.contains(&PlanError::DuplicateNode { node: "filter".into() }));
    assert!(
        errors.contains(&PlanError::MissingParameter { node: "filter".into(), parameter: "field".into() })
    );
    assert!(errors.contains(&PlanError::UnknownPort { node: "start".into(), port: "out".into() }));
    assert!(errors.contains(&PlanError::UnknownNode { node: "ghost".into() }));
}

#[test]
fn nodes_no_trigger_leads_to_are_listed_and_skipped() {
    let flow = FlowDefinition::new("f", "island")
        .node(three_orders())
        .node(NodeDef::new("set", "set"))
        .node(NodeDef::new("alone", "set"))
        .connect("start", "main", "set", "main");
    let catalog = catalog();
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    assert_eq!(plan.unreachable(), ["alone"]);
    let run = Executor::new(&plan, &catalog).clock(FixedClock(0)).run();
    assert_eq!(run.status("alone"), Some(NodeStatus::Skipped));
}

#[test]
fn the_json_reads_back_the_same_and_a_newer_format_is_refused() {
    let flow = FlowDefinition::new("f", "json")
        .node(three_orders().title("Orders").on_error(ErrorPolicy::Continue))
        .node(paid_filter().disabled(true).pin("main", vec![Item::new(json!({ "id": "x" }))]))
        .connect("start", "main", "filter", "main");
    let text = flow.to_json();
    assert_eq!(FlowDefinition::from_json(&text), Ok(flow));

    let newer = text.replacen("\"format\":1", "\"format\":9", 1);
    let refused = FlowDefinition::from_json(&newer).expect_err("written by a newer build");
    assert!(refused.to_string().contains('9'));
    assert!(FlowDefinition::from_json("not json").is_err());
}

#[test]
fn the_flow_drawn_on_the_canvas_reads_back_as_the_same_flow() {
    let catalog = catalog();
    let flow = FlowDefinition::new("f", "drawn")
        .node(three_orders())
        .node(NodeDef::new("if", "branch").title("Paid?"))
        .node(NodeDef::new("yes", "set").disabled(true))
        .node(paid_filter())
        .connect("start", "main", "if", "main")
        .connect("if", "true", "yes", "main")
        .connect("if", "false", "filter", "main");
    let graph = flow.to_graph(&catalog);

    let card = graph.find_node("if").expect("drawn");
    assert_eq!(card.title, "Paid?");
    assert_eq!(card.ports.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["main", "true", "false"]);
    assert_eq!(graph.find_node("start").map(|n| n.title.as_str()), Some("Manual start"));
    assert_eq!(graph.find_node("yes").map(|n| n.status.clone()), Some(FlowNodeStatus::Disabled));
    assert!(graph.find_node("if").expect("drawn").x > graph.find_node("start").expect("drawn").x);

    assert_eq!(FlowDefinition::from_graph(&graph, &flow, &catalog), flow);

    let mut moved = graph.clone();
    moved.nodes.retain(|n| n.id != "filter");
    moved.nodes.push(FlowNode::new("new", "set", "Set", 0.0, 0.0));
    let read = FlowDefinition::from_graph(&moved, &flow, &catalog);
    assert!(read.find("filter").is_none(), "a node gone from the drawing is gone");
    assert_eq!(read.connections.len(), 2, "and its connection with it");
    assert_eq!(read.find("start").map(|n| &n.parameters), Some(&three_orders().parameters), "setup kept");
    assert_eq!(read.find("new").map(|n| n.parameters.is_empty()), Some(true));
}

#[test]
fn the_canvas_debugger_runs_the_flow_by_the_catalog_rules() {
    let catalog = catalog();
    let flow = FlowDefinition::new("f", "debug")
        .node(three_orders())
        .node(NodeDef::new("if", "branch"))
        .node(NodeDef::new("yes", "set"))
        .node(NodeDef::new("no", "set"))
        .connect("start", "main", "if", "main")
        .connect("if", "true", "yes", "main")
        .connect("if", "false", "no", "main");
    let mut graph = flow.to_graph(&catalog);
    graph.nodes.push(FlowNode::new("note", "note", "A note", 0.0, 400.0));
    let rules = catalog.run_rules(&flow);
    let gate = |id: &str| (rules.gate)(graph.find_node(id).expect("drawn"));
    assert_eq!(gate("start"), FlowGate::Start);
    assert_eq!(gate("if"), FlowGate::Choose);
    assert_eq!(gate("yes"), FlowGate::End, "it leads nowhere in the flow");
    assert_eq!(gate("note"), FlowGate::Outside);

    let mut run = FlowRun::start(&graph, &rules);
    run.step(&graph, &rules, None);
    run.step(&graph, &rules, None);
    assert_eq!(run.choosing.as_deref(), Some("if"));
    let way = graph.edges.iter().find(|e| e.to.node == "yes").map(|e| e.id.clone()).expect("drawn");
    run.step(&graph, &rules, Some(&way));
    run.step(&graph, &rules, None);
    assert!(run.is_over() && run.failure().is_none(), "{:?}", run.events);
}
