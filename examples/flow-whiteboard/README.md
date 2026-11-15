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
