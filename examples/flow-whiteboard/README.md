# flow-whiteboard

A diagramming whiteboard made only with the library's `FlowCanvas`, `FlowToolbox`, `FlowInspector` and a `SegmentedControl`: a flowchart and a mind map, each on a board of its own. It is here to check that the diagram component is enough for this kind of product.

```bash
cargo run -p flow-whiteboard
```

## What it shows

- **A flowchart** with step lines that turn at right angles, a decision that leads two ways, documents, data and a note, and a loop back that goes round the shapes through **bend points**.
- **A mind map** arranged as a tree from the middle idea, with **Arrange** keeping it so after changes.
- **Growing from the keyboard**: Cmd/Ctrl+Enter makes the next shape (a process in the flowchart, an idea in the mind map), already open to type its title.
- **Shapes to drag in** from a `FlowToolbox`, and their properties in a `FlowInspector`.
- **Saving the board** as PNG, SVG or PDF with `ravenrs-flow-export`.

## What the component did not cover

Each of these is an issue of its own:

- Ports that nothing joins show all the time; a whiteboard shows them only by the shape under the pointer.
- No color per shape or per line, so groups of ideas cannot be told apart by color.
- A mind map grows only to one side; growing to both sides of the middle idea needs a layout of its own.
