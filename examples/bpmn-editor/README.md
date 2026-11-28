# bpmn-editor

A BPMN 2.0 editor made only of RavenRS components, as a real test of the diagram component:
nothing on its screen is drawn by the example, which only joins the parts.

```bash
cargo run -p bpmn-editor --release
```

## What it is made of

- `FlowLibrary` keeps the processes by name in `~/.ravenrs/bpmn-editor/processos`, and opens and
  saves them as BPMN 2.0 XML with their drawing (BPMNDI), which other BPMN tools read and write.
- `FlowToolbox`, `FlowCanvas` and `FlowInspector` take BPMN from `ravenrs_flow_bpmn::notation()`:
  events, tasks, gateways, data, notes and lanes; sequence flows, associations and message flows
  drawn as BPMN draws them; what may be joined; the next element from the keys.
- Check, in the canvas's controls, says what is wrong (no start or end, a step that leads
  nowhere, a decision with one way or unnamed ways, what the start never reaches) and writes it
  on the element.
- Debug runs the process a step at a time: tokens leave the start, a way is chosen at each
  decision, parallel gateways wait for every way, breakpoints stop the run (F9), and dead ends,
  joins that never complete and loops are said on the element where they happen.
- `Chat` talks to a model running on this machine (`ravenrs-ai`, chosen and downloaded in
  `ModelManager`): a description becomes a process, which Cmd/Ctrl+Z undoes, and the open process
  can be reviewed. The model only writes a small plan; the app reads it even when its JSON is
  broken, mends it out loud and checks it, so a model never writes the BPMN.

## Tested

- `cargo test -p ravenrs-flow-bpmn`: the XML against itself and against another tool's file,
  the checks, the plans and the debugger through the screen.
- `cargo test -p bpmn-editor`: the editor through its interface.
- With a local model on disk, `RAVENRS_MODELS=<folder> cargo test -p bpmn-editor --test local_ai
  -- --ignored --nocapture`. Measured with LFM3 1.7B on an Apple M2 Pro: a reimbursement process
  drafted in about 12 seconds, its broken JSON mended, and the checks pointing at the three
  mistakes the model made; the review in about 13 seconds.

## In the browser, on Linux and on Windows

In the browser, the diagrams are kept in the browser's own storage:

```bash
cargo xtask web bpmn-editor
python3 -m http.server -d examples/bpmn-editor/web 8080
```

On Linux and Windows, run `cargo run -p bpmn-editor --release` and check that you can open "Pedido entre cliente e loja"; Verificar says nothing is wrong; Depurar runs a step at a time; Salvar em arquivo as BPMN 2.0 and open the file again; on the desktop, pick a model and ask for a process.

The local AI runs only on the desktop: in the browser the chat says so.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: the approved purchase opens first, and the order between customer and shop opens from the list. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/bpmn-editor --run-all --no-cache --format json --out target/e2e/bpmn-editor
```

The app answers the Studio when opened with `--e2e-serve`, on data in memory: the files it keeps on disk are never opened. Its name on the channel, which the `open` block of a scenario uses, is "Editor BPMN" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

