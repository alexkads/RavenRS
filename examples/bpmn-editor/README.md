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
