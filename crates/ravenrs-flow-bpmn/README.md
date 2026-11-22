# ravenrs-flow-bpmn

BPMN 2.0 for the [RavenRS](https://github.com/alexkads/RavenRS) diagram components, as data and functions only: `FlowCanvas`, `FlowToolbox`, `FlowInspector` and `FlowLibrary` are the library's own, and this crate hands them what BPMN is.

- `notation()` is BPMN as a `FlowNotation`: elements grouped for a palette, how tasks look, which connections may be made and how each is drawn, the next element from the keys, the checks of a process, and the BPMN XML format.
- `xml` writes a process as BPMN 2.0 XML with its drawing (BPMNDI) and reads what other tools write: elements, lanes, labels and bend points.
- `rules` says what is wrong with a process.
- `plan` is what a local model is asked and how its answer becomes a process. The model only writes a small plan the app checks, never the XML.

`examples/bpmn-editor` is an editor made only of library components with this notation.
