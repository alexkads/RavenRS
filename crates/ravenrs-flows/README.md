# ravenrs-flows

Runs flows drawn on a RavenRS `FlowCanvas`: automations where lists of items pass from node to node.

The `FlowCanvas` draws a diagram, and its debugger (`FlowRun`) moves tokens through it a step at a time. This crate runs the flow for real. Each node is of a kind with code behind it, the items that reach a node are what it works on, and what it gives goes on to the next nodes. It has no screen and no GPU, so a flow runs the same in the app, on a server or on a worker thread.

## What is in it

| Piece | What it does |
|---|---|
| `FlowDefinition`, `NodeDef`, `Connection` | The flow as the app keeps it: nodes with kind, version, title, parameters, disabled state, error policy and pinned items, and connections between named ports. Written as JSON with a format version; a newer version is refused, so an older build never drops what it does not know. |
| `NodeKind`, `NodeSpec`, `Catalog` | The kinds of node. A kind says its ports, the parameters it requires, and whether it is a trigger, a loop or a branch, and it runs on what reached its inputs. The catalog keeps the kinds by name and version. |
| `Plan` | The flow checked against the catalog and put in order. It reports everything wrong at once (an unknown kind, a port the kind lacks, a missing parameter, two nodes with one id, a connection to nowhere) and refuses a cycle unless it goes through a loop node. It also lists the nodes no trigger leads to. |
| `Executor`, `Run` | Runs a plan in memory, in order, on an injected clock. It can run the whole flow, run from one node on (the nodes upstream do not run again, and their items come from an earlier run), or run one node alone. In a test run, pinned items take the place of running the node. |
| `Item`, `ItemRef` | What passes along a connection: a JSON value, the files it carries and where it came from. `Item::derive` makes a new item from another; an item passed on unchanged is traced to where it was. |
| `to_graph`, `from_graph`, `Catalog::run_rules` | The bridge to the canvas. A flow is drawn as a `FlowGraph`, the drawing is read back into the flow (geometry stays in the graph, setup in the definition), and the debugger gets rules from the catalog. |

## How a run treats each node

- A **trigger** runs with no input. Any other node runs only when items reached it; otherwise it is *skipped*.
- A **disabled** node passes the items of its first input to its first output unchanged.
- In a **test** (`Mode::Test`), a node with **pinned** items gives them instead of running. When the flow is working (`Mode::Active`), they are ignored.
- A node that **fails** stops the run (`ErrorPolicy::Stop`), or gives nothing while the run goes on (`ErrorPolicy::Continue`). A node that answers on a port its kind does not have also fails.
- The same flow, items and clock always give the same run.

## Example

```rust
use ravenrs_flows::*;
use serde_json::json;

struct Manual;
impl NodeKind for Manual {
    fn spec(&self) -> NodeSpec {
        NodeSpec::new("manual", 1).output("main").trigger()
    }
    fn execute(&self, cx: &NodeContext<'_>) -> Result<Ports, NodeError> {
        let items = cx.parameter("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        Ok(Ports::from([("main".into(), items.into_iter().map(Item::new).collect())]))
    }
}

let catalog = Catalog::new().with(Manual);
let flow = FlowDefinition::new("f1", "Orders")
    .node(NodeDef::new("start", "manual").parameter("items", json!([{ "id": 1 }])));
let plan = Plan::compile(&flow, &catalog).expect("the catalog knows every kind");
let run = Executor::new(&plan, &catalog).clock(FixedClock(0)).run();
assert_eq!(run.output("start", "main").len(), 1);
```

## Not here yet

Each of these has its own issue, under [#523](https://github.com/alexkads/RavenRS/issues/523):

- Loop operators, merge modes, batches and the other control and transform nodes: [#534](https://github.com/alexkads/RavenRS/issues/534). Until then, a run does not follow the connections that come back to a loop node (`Plan::back_connections`).
- A durable run, with a queue, checkpoints, resuming and retention: [#535](https://github.com/alexkads/RavenRS/issues/535).
- Cancellation, time limits and retries: [#536](https://github.com/alexkads/RavenRS/issues/536).
- The schema of a kind's parameters, beyond the names it requires: [#530](https://github.com/alexkads/RavenRS/issues/530).
- The domain data of a node inside the `FlowGraph` itself: [#529](https://github.com/alexkads/RavenRS/issues/529). Until then, `from_graph` takes the flow the drawing came from, to keep each node's setup.

License: MIT OR Apache-2.0.
