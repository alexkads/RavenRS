# ravenrs-flows

Runs flows drawn on a RavenRS `FlowCanvas`: automations where lists of items pass from node to node.

The `FlowCanvas` draws a diagram, and its debugger (`FlowRun`) moves tokens through it a step at a time. This crate runs the flow for real. Each node is of a kind with code behind it, the items that reach a node are what it works on, and what it gives goes on to the next nodes. It has no screen and no GPU, so a flow runs the same in the app, on a server or on a worker thread.

## What is in it

| Piece | What it does |
|---|---|
| `FlowDefinition`, `NodeDef`, `Connection` | The flow as the app keeps it: nodes with kind, version, title, parameters, disabled state, error policy and pinned items, and connections between named ports. Written as JSON with a format version; a newer version is refused, so an older build never drops what it does not know. |
| `NodeKind`, `NodeSpec`, `Catalog` | The kinds of node. A kind says its ports, the parameters it requires, and whether it is a trigger, a loop or a branch, and it runs on what reached its inputs. The catalog keeps the kinds by name and version. |
| `Plan` | The flow checked against the catalog and put in order. It reports everything wrong at once (an unknown kind, a port the kind lacks, a missing parameter, two nodes with one id, a connection to nowhere) and refuses a cycle unless it goes through a loop node. It also lists the nodes no trigger leads to. |
| `Executor`, `Run` | Runs a plan in memory, in order, on an injected clock. It can run the whole flow, run from one node on (the nodes upstream do not run again, and their items come from an earlier run), or run one node alone. In a test run, pinned items take the place of running the node. A loop goes round its body while the loop node sends items into it, up to `LOOP_LIMIT` turns (`Executor::loop_limit`). |
| `nodes`, `Catalog::builtin` | The built-in kinds, listed below. |
| `Item`, `ItemRef` | What passes along a connection: a JSON value, the files it carries and where it came from. `Item::derive` makes a new item from another; an item passed on unchanged is traced to where it was. |
| `to_graph`, `from_graph`, `Catalog::run_rules`, `card`, `port_name` | The bridge to the canvas. A flow is drawn as a `FlowGraph` with each node's setup in its card's `data` (under `GRAPH_KEY`), so copying, pasting and undoing on the canvas keep it; `from_graph` reads the drawing back on its own, and the debugger gets rules from the catalog. `card` draws a node of a kind with its ports, the same for `to_graph` and for an app's toolbox. On the canvas a port's id is unique within its node, so an input that shares its name with an output (`main`, in most kinds) is drawn as `in:main`, and `port_name` gives the name back. |

## How a run treats each node

- A **trigger** runs with no input. Any other node runs only when items reached it; otherwise it is *skipped*.
- A **disabled** node passes the items of its first input to its first output unchanged.
- In a **test** (`Mode::Test`), a node with **pinned** items gives them instead of running. When the flow is working (`Mode::Active`), they are ignored.
- A node that **fails** stops the run (`ErrorPolicy::Stop`), or gives nothing while the run goes on (`ErrorPolicy::Continue`). A node that answers on a port its kind does not have also fails.
- The same flow, items and clock always give the same run.

## The built-in kinds

`Catalog::builtin()` holds them all, each at version 1. Conditions and values are written in the criteria language of `ravenrs-criteria`, such as `[paid] = true` or `[qty] * [price]`, and read each item's JSON. The language is parsed into a tree and runs no code.

| Kind | What it does | Main parameters |
|---|---|---|
| `manual` | Starts a run with the items it is given. | `items` |
| `if` | Sends each item to `true` or `false`. A side no item takes gives nothing, so a merge after it goes on with the other. | `condition` |
| `switch` | One output per case and `otherwise` for the rest. | `value`, `cases` |
| `merge` | Joins inputs `a` and `b`: `append` (the default), `position`, `key` (`key`, or `key_a` and `key_b`, and `keep_unmatched`) or `first`. | `mode` |
| `batches` | A loop. Each turn it sends the next batch to `batch`; what the body sends back reaches `back`; with no batch left, everything that came back goes out on `done`. | `size` |
| `set` | Sets fields from expressions, or keeps only them. | `fields`, `keep_only` |
| `wait` | Waits on the run's clock and passes its items on; stops early when cancelled. In memory: a wait that outlives the app needs [#535](https://github.com/alexkads/RavenRS/issues/535). | `ms` |
| `delay` | Waits in a way that outlives the app: the run stops here, and a durable run (`ravenrs-flows-runtime`) starts it again when the time comes. | `ms` |
| `wait_event` | Waits for an event (an approval, a reply) and gives the items it brought; with `keep`, a gate: the items that waited go on, each with the event under `as`. | `event`, `keep`, `as` |
| `subflow` | Runs another flow with the items that came. Made apart, with the flows it may call: `Subflow::new(flows, catalog)`. | `flow`, `input`, `output` |
| `filter` | Keeps the items whose condition is true and sends the rest to `discarded`. | `condition` |
| `sort` | Orders by fields, each rising or falling. Ties keep the order they came in. | `by` |
| `group` | One item per group, with `sum`, `count`, `min`, `max`, `avg` and `list`. | `by`, `aggregates` |
| `split` | One item per entry of a list field. An item whose field is not a list goes to the error output. | `field` |
| `join` | One item holding a list of all that came, or of one field. | `field`, `as` |

## When something fails

Every node has an error output, `ERROR_PORT` (`"error"`), besides its kind's. A kind sends the items that failed one by one there (`Item::failed` keeps the item and adds why), and the node's `on_error` decides the rest:

- `Stop` (the default): a failure of the node, or of any of its items, stops the run.
- `Continue`: the failed items go to the error output and the others where they were going. A node that fails as a whole gives nothing.
- `Route`: like `Continue`, but a node that fails as a whole sends every item that reached it to the error output, with the reason.

## Stopping, time limits and trying again

- **Cancelling**: give the executor a `CancelToken` (`Executor::cancel_with`), keep a clone and call `cancel` from any thread. Each node then runs on a thread of its own, so even a node that never looks at its token is left behind and the run ends `RunOutcome::Cancelled`. A node that calls something slow passes its token on (`NodeContext::token`); a subflow gets a child token, so it stops with the flow that called it.
- **Time limits**: `NodeDef::timeout_ms` for each try of a node, and `Executor::time_limit_ms` for the whole run. A node over its limit is `NodeStatus::TimedOut`; a run over its limit ends `RunOutcome::TimedOut`. Long nodes call `NodeContext::check` between steps and wait with `NodeContext::sleep`, which stops early.
- **Trying again**: `NodeDef::retry` with `Retry::new(tries, wait_ms)`, `backoff` and `on_timeout`. A kind says what kind of failure it was: `NodeError::retryable` is tried again, `NodeError::new` is not, and `NodeError::unknown` (a call whose answer was lost) is never repeated until `NodeKind::reconcile` says it did not happen; otherwise the node is `NodeStatus::Uncertain`.
- **The clock**: waits and retries wait on the run's `Clock`. `ManualClock` moves instead of waiting, so tests of them take no time. A node that blocks a thread is stopped in real time.

Rust cannot kill a thread: a node left behind keeps running until it looks at its token. Stopping a child process for real is the job of the process runner ([#558](https://github.com/alexkads/RavenRS/issues/558)), which takes the same token.

## Keeping a run

`Executor::on_step` hears each step as it happens: a node started (`Step::Started`), its part is over (`Step::Settled`), or a loop ended (`Step::LoopDone`). `Executor::resume(checkpoint, in_progress)` goes on from what was written:
- the nodes in the checkpoint keep their part;
- the nodes that were running are asked to reconcile;
- a waiting node runs again to see whether its wait is over (`Executor::woken`, `Executor::events`).

[`ravenrs-flows-runtime`](../ravenrs-flows-runtime) does this on SQLite.

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

- The schema of a kind's parameters, beyond the names it requires: [#530](https://github.com/alexkads/RavenRS/issues/530).

License: MIT OR Apache-2.0.
