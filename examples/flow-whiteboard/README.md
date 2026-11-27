# flow-whiteboard

A diagramming whiteboard made only with library components (`FlowCanvas`, `FlowToolbox`, `FlowInspector`, `FlowLibrary`, `SegmentedControl`): a flowchart and a mind map, each on a board of its own. It is here to check that the diagram component is enough for this kind of product.

```bash
cargo run -p flow-whiteboard
```

## What it shows

- **A flowchart** with step lines that turn at right angles, a decision that leads two ways, documents, data and a note, and a loop back that goes round the shapes through **bend points**.
- **A mind map** arranged as a tree from the middle idea, with **Arrange** keeping it so after changes.
- **Growing from the keyboard**: Cmd/Ctrl+Enter makes the next shape (a process in the flowchart, an idea in the mind map), already open to type its title.
- **Shapes to drag in** from a `FlowToolbox`, and their properties in a `FlowInspector`.
- **Saving the board** as PNG, SVG or PDF with `ravenrs-flow-export`.

## What it needed, now in the library

Building it showed what the component lacked, and each piece went into the library first:
`FlowCanvas::ports_on_hover`, colors per shape and line (`FlowStyle`, `FlowEdge::color`), and
`FlowLayout::mind_map`, which grows a mind map to both sides of its middle idea and turns the
ports of the left half to face it.

## In the browser, on Linux and on Windows

In the browser, the diagrams are kept in the browser's own storage:

```bash
cargo xtask web flow-whiteboard
python3 -m http.server -d examples/flow-whiteboard/web 8080
```

On Linux and Windows, run `cargo run -p flow-whiteboard --release` and check that you can switch between Fluxograma and Mapa mental; Cmd/Ctrl+Enter on an idea makes the next one; drag a bend point of the loop; Salvar, close and open again.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: the sample flowchart starts with the order; named and saved, it shows on the list, and the mind map opens. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/flow-whiteboard --run-all --no-cache --format json --out target/e2e/flow-whiteboard
```

The app answers the Studio when opened with `--e2e-serve`, on data in memory: the files it keeps on disk are never opened. Its name on the channel, which the `open` block of a scenario uses, is "Whiteboard" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

