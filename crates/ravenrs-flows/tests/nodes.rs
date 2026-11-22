//! The acceptance of #534: the built-in nodes and the run going round loops and sending failed
//! items to the error output, with no window.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use ravenrs_flows::{
    Catalog, ERROR_PORT, ErrorPolicy, Executor, FixedClock, FlowDefinition, Item, LOOP_LIMIT, NodeContext,
    NodeDef, NodeError, NodeKind, NodeSpec, NodeStatus, Plan, PlanError, Ports, Run,
};
use serde_json::{Value, json};

fn run_with(flow: &FlowDefinition, catalog: &Catalog) -> Run {
    let plan = Plan::compile(flow, catalog).expect("the flow compiles");
    Executor::new(&plan, catalog).clock(FixedClock(0)).run()
}

fn run(flow: &FlowDefinition) -> Run {
    run_with(flow, &Catalog::builtin())
}

fn start(id: &str, items: Value) -> NodeDef {
    NodeDef::new(id, "manual").parameter("items", items)
}

fn jsons(items: &[Item]) -> Vec<Value> {
    items.iter().map(|i| i.json.clone()).collect()
}

fn customers() -> Value {
    json!([
        { "id": 1, "name": "Ana", "city": "Recife", "paid": false, "total": 10 },
        { "id": 2, "name": "Bruno", "city": "Natal", "paid": false, "total": 5 },
        { "id": 3, "name": "Clara", "city": "Recife", "paid": false, "total": 7 }
    ])
}

#[test]
fn a_branch_false_for_every_item_lets_a_merge_that_waits_for_both_go_on_with_the_other() {
    let flow = FlowDefinition::new("f", "branch")
        .node(start("start", customers()))
        .node(NodeDef::new("if", "if").parameter("condition", json!("[paid] = true")))
        .node(NodeDef::new("thank", "set").parameter("fields", json!({ "note": "'thanks'" })))
        .node(NodeDef::new("remind", "set").parameter("fields", json!({ "note": "'please pay'" })))
        .node(NodeDef::new("merge", "merge"))
        .connect("start", "main", "if", "main")
        .connect("if", "true", "thank", "main")
        .connect("if", "false", "remind", "main")
        .connect("thank", "main", "merge", "a")
        .connect("remind", "main", "merge", "b");
    let run = run(&flow);
    assert_eq!(run.output("if", "true").len(), 0);
    assert_eq!(run.status("thank"), Some(NodeStatus::Skipped));
    let merged = run.output("merge", "main");
    assert_eq!(merged.len(), 3);
    assert!(merged.iter().all(|i| i.get("note") == Some(&json!("please pay"))));
}

#[test]
fn a_merge_by_key_matches_the_items_of_two_branches_by_the_customer_id() {
    let flow = FlowDefinition::new("f", "key")
        .node(start("people", customers()))
        .node(start(
            "orders",
            json!([{ "customer": 3, "order": "c1" }, { "customer": 1, "order": "a1" }, { "customer": 1, "order": "a2" }, { "customer": 9, "order": "z" }]),
        ))
        .node(NodeDef::new("merge", "merge").parameter("mode", json!("key")).parameter("key_a", json!("[id]")).parameter("key_b", json!("[customer]")))
        .connect("people", "main", "merge", "a")
        .connect("orders", "main", "merge", "b");
    let run = run(&flow);
    let merged = run.output("merge", "main");
    let pairs: Vec<(Value, Value)> =
        merged.iter().map(|i| (i.json["name"].clone(), i.json["order"].clone())).collect();
    assert_eq!(
        pairs,
        [(json!("Ana"), json!("a1")), (json!("Ana"), json!("a2")), (json!("Clara"), json!("c1"))]
    );
    assert_eq!(merged[0].from.len(), 2, "a matched item comes from both sides");
    assert_eq!(merged[0].from[0].node, "people");
    assert_eq!(merged[0].from[1].node, "orders");
}

#[test]
fn a_loop_in_batches_of_two_over_five_items_goes_round_three_times_and_ends_with_five() {
    let flow = FlowDefinition::new("f", "loop")
        .node(start("start", json!([{ "n": 1 }, { "n": 2 }, { "n": 3 }, { "n": 4 }, { "n": 5 }])))
        .node(NodeDef::new("loop", "batches").parameter("size", json!(2)))
        .node(NodeDef::new("work", "set").parameter("fields", json!({ "double": "[n] * 2" })))
        .node(NodeDef::new("after", "join").parameter("field", json!("double")))
        .connect("start", "main", "loop", "main")
        .connect("loop", "batch", "work", "main")
        .connect("work", "main", "loop", "back")
        .connect("loop", "done", "after", "main");
    let run = run(&flow);
    assert_eq!(run.nodes["work"].runs, 3, "the body went round three times");
    assert_eq!(run.nodes["loop"].runs, 4, "three batches and the turn that ends");
    assert_eq!(run.output("loop", "done").len(), 5);
    assert_eq!(jsons(run.output("after", "main")), [json!({ "items": [2, 4, 6, 8, 10] })]);
    assert_eq!(run.output("loop", "done")[4].from[0].node, "work", "each item came back from the body");
    assert!(run.succeeded());
}

#[test]
fn grouping_by_city_with_a_sum_gives_one_item_per_city() {
    let flow = FlowDefinition::new("f", "group")
        .node(start("start", customers()))
        .node(NodeDef::new("group", "group").parameter("by", json!(["city"])).parameter(
            "aggregates",
            json!([{ "field": "total", "op": "sum", "as": "total" }, { "op": "count", "as": "customers" }]),
        ))
        .connect("start", "main", "group", "main");
    let run = run(&flow);
    assert_eq!(
        jsons(run.output("group", "main")),
        [
            json!({ "city": "Recife", "total": 17, "customers": 2 }),
            json!({ "city": "Natal", "total": 5, "customers": 1 })
        ]
    );
    assert_eq!(run.output("group", "main")[0].from.len(), 2, "a group comes from its members");
}

#[test]
fn an_item_that_fails_goes_to_the_error_output_and_the_others_go_on_when_the_node_continues() {
    let flow = |policy: ErrorPolicy| {
        FlowDefinition::new("f", "split")
            .node(start(
                "start",
                json!([{ "tags": ["a", "b"] }, { "tags": "not a list" }, { "tags": ["c"] }]),
            ))
            .node(NodeDef::new("split", "split").parameter("field", json!("tags")).on_error(policy))
            .node(NodeDef::new("ok", "join").parameter("field", json!("tags")))
            .node(NodeDef::new("bad", "join"))
            .connect("start", "main", "split", "main")
            .connect("split", "main", "ok", "main")
            .connect("split", ERROR_PORT, "bad", "main")
    };
    let going_on = run(&flow(ErrorPolicy::Continue));
    assert!(going_on.succeeded());
    assert_eq!(going_on.status("split"), Some(NodeStatus::Done));
    assert_eq!(jsons(going_on.output("ok", "main")), [json!({ "items": ["a", "b", "c"] })]);
    let failed = going_on.output("split", ERROR_PORT);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].get("error"), Some(&json!("tags is not a list")));
    assert_eq!(failed[0].get("tags"), Some(&json!("not a list")), "it keeps what it had");
    assert_eq!(going_on.output("bad", "main").len(), 1);

    let stopped = run(&flow(ErrorPolicy::Stop));
    assert_eq!(stopped.stopped_at.as_deref(), Some("split"));
    assert!(stopped.nodes["split"].error.as_deref().is_some_and(|e| e.contains("tags is not a list")));
    assert_eq!(stopped.status("ok"), None);
}

/// A service that always refuses.
struct Refuses;
impl NodeKind for Refuses {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("refuses", 1).input("main").output("main")
    }
    fn execute(&self, _: &NodeContext<'_>) -> Result<Ports, NodeError> {
        Err(NodeError::new("the service is down"))
    }
}

#[test]
fn a_node_that_fails_as_a_whole_routes_every_item_to_its_error_output() {
    let catalog = Catalog::builtin().with(Refuses);
    let flow = |policy: ErrorPolicy| {
        FlowDefinition::new("f", "route")
            .node(start("start", customers()))
            .node(NodeDef::new("call", "refuses").on_error(policy))
            .node(NodeDef::new("log", "join").parameter("field", json!("error")))
            .connect("start", "main", "call", "main")
            .connect("call", ERROR_PORT, "log", "main")
    };
    let routed = run_with(&flow(ErrorPolicy::Route), &catalog);
    assert!(routed.succeeded());
    assert_eq!(routed.status("call"), Some(NodeStatus::Failed));
    assert_eq!(
        jsons(routed.output("log", "main")),
        [json!({ "items": ["the service is down", "the service is down", "the service is down"] })]
    );

    let going_on = run_with(&flow(ErrorPolicy::Continue), &catalog);
    assert!(going_on.succeeded());
    assert_eq!(going_on.status("log"), Some(NodeStatus::Skipped), "continue gives nothing");
}

#[test]
fn a_switch_sends_each_item_to_its_case_and_the_rest_to_otherwise() {
    let flow = FlowDefinition::new("f", "switch")
        .node(start("start", customers()))
        .node(
            NodeDef::new("by", "switch")
                .parameter("value", json!("[city]"))
                .parameter("cases", json!(["Recife", "Fortaleza"])),
        )
        .node(NodeDef::new("recife", "join").parameter("field", json!("name")))
        .connect("start", "main", "by", "main")
        .connect("by", "Recife", "recife", "main");
    let run = run(&flow);
    assert_eq!(jsons(run.output("recife", "main")), [json!({ "items": ["Ana", "Clara"] })]);
    assert_eq!(run.output("by", "Fortaleza").len(), 0);
    assert_eq!(run.output("by", "otherwise").len(), 1);

    let wrong = FlowDefinition::new("f", "x")
        .node(start("start", customers()))
        .node(
            NodeDef::new("by", "switch")
                .parameter("value", json!("[city]"))
                .parameter("cases", json!(["Recife"])),
        )
        .node(NodeDef::new("j", "join"))
        .connect("start", "main", "by", "main")
        .connect("by", "Natal", "j", "main");
    let errors = Plan::compile(&wrong, &Catalog::builtin()).expect_err("Natal is not a case");
    assert_eq!(errors, [PlanError::UnknownPort { node: "by".into(), port: "Natal".into() }]);
}

#[test]
fn sort_filter_set_split_and_join_do_what_they_say() {
    let flow = FlowDefinition::new("f", "ops")
        .node(start("start", customers()))
        .node(
            NodeDef::new("sort", "sort")
                .parameter("by", json!(["city", { "field": "total", "descending": true }])),
        )
        .node(NodeDef::new("big", "filter").parameter("condition", json!("[total] >= 7")))
        .node(
            NodeDef::new("short", "set")
                .parameter("fields", json!({ "who": "Upper([name])" }))
                .parameter("keep_only", json!(true)),
        )
        .node(NodeDef::new("all", "join").parameter("field", json!("who")).parameter("as", json!("names")))
        .connect("start", "main", "sort", "main")
        .connect("sort", "main", "big", "main")
        .connect("big", "main", "short", "main")
        .connect("short", "main", "all", "main");
    let run = run(&flow);
    let sorted: Vec<Value> = run.output("sort", "main").iter().map(|i| i.json["name"].clone()).collect();
    assert_eq!(
        sorted,
        [json!("Bruno"), json!("Ana"), json!("Clara")],
        "Natal first, then Recife by total down"
    );
    assert_eq!(run.output("big", "discarded").len(), 1);
    assert_eq!(jsons(run.output("short", "main")), [json!({ "who": "ANA" }), json!({ "who": "CLARA" })]);
    assert_eq!(jsons(run.output("all", "main")), [json!({ "names": ["ANA", "CLARA"] })]);
}

#[test]
fn a_merge_by_position_makes_pairs_and_first_takes_the_first_side_with_items() {
    let flow = |mode: &str| {
        FlowDefinition::new("f", "merge")
            .node(start("a", json!([{ "x": 1 }, { "x": 2 }])))
            .node(start("b", json!([{ "y": "um" }])))
            .node(NodeDef::new("merge", "merge").parameter("mode", json!(mode)))
            .connect("a", "main", "merge", "a")
            .connect("b", "main", "merge", "b")
    };
    assert_eq!(
        jsons(run(&flow("position")).output("merge", "main")),
        [json!({ "x": 1, "y": "um" }), json!({ "x": 2 })]
    );
    assert_eq!(jsons(run(&flow("first")).output("merge", "main")), [json!({ "x": 1 }), json!({ "x": 2 })]);
    assert_eq!(run(&flow("append")).output("merge", "main").len(), 3);
    let odd = run(&flow("sideways"));
    assert!(odd.nodes["merge"].error.as_deref().is_some_and(|e| e.contains("sideways")));
}

/// A loop that always sends its input into the body.
struct Forever;
impl NodeKind for Forever {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("forever", 1).input("main").input("back").output("again").loops()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        Ok(Ports::from([("again".to_owned(), cx.input("main").to_vec())]))
    }
}

#[test]
fn a_loop_that_never_ends_stops_at_the_limit_and_says_so() {
    let catalog = Catalog::builtin().with(Forever);
    let flow = FlowDefinition::new("f", "forever")
        .node(start("start", json!([{}])))
        .node(NodeDef::new("loop", "forever"))
        .node(NodeDef::new("body", "join"))
        .connect("start", "main", "loop", "main")
        .connect("loop", "again", "body", "main")
        .connect("body", "main", "loop", "back");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).clock(FixedClock(0)).loop_limit(50).run();
    assert_eq!(run.stopped_at.as_deref(), Some("loop"));
    assert_eq!(run.nodes["body"].runs, 50);
    const { assert!(LOOP_LIMIT >= 1_000, "the default lets long loops of batches end") };
    assert!(run.nodes["loop"].error.as_deref().is_some_and(|e| e.contains("went round")));
}

#[test]
fn one_turn_of_a_loop_alone_runs_only_the_loop_node() {
    let flow = FlowDefinition::new("f", "loop")
        .node(start("start", json!([{ "n": 1 }, { "n": 2 }])))
        .node(NodeDef::new("loop", "batches"))
        .node(NodeDef::new("work", "join"))
        .connect("start", "main", "loop", "main")
        .connect("loop", "batch", "work", "main")
        .connect("work", "main", "loop", "back");
    let catalog = Catalog::builtin();
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    assert_eq!(plan.loop_body("loop"), ["work"]);
    let executor = Executor::new(&plan, &catalog).clock(FixedClock(0));
    let first = executor.run();
    let alone = executor.run_node("loop", &first).expect("start ran before");
    assert_eq!(alone.nodes["loop"].runs, 1);
    assert_eq!(alone.output("loop", "batch").len(), 1);
    assert_eq!(alone.status("work"), None);
}
