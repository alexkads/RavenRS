# flow-design-board

A design board made only with the library's `FlowCanvas`, `FlowToolbox` and `FlowInspector`: screens as frames, the pieces of each screen placed freely, resized, turned and lined up with the guides, and arrows between screens saying where a tap leads. It is here to check how far the diagram component goes as a board of this kind.

```bash
cargo run -p flow-design-board
```

## What it shows

- **Frames** (groups) that hold their pieces and move with them, from a phone or a desktop model.
- **Pieces** in nine outlines, resized by the corner, turned by the handle, lined up with the guides (Alt does without) on a fine grid.
- **A note** beside a screen, outside its frame.
- **Where a tap leads**: a connection from a button to another screen.
- **The size, place and words** of the selected piece in a `FlowInspector`.
- **Saving the board** as PNG, SVG or PDF with `ravenrs-flow-export`.

## What the component did not cover

These are what keep it from being a design tool, each an issue of its own:

- No fill, line color or font per piece: every piece is drawn in the theme's colors.
- No free text, image or drawn path: a piece is always one of the outlines with a title.
- No list of layers to reorder and hide pieces (the order changes only with Cmd/Ctrl+] and [).
- No snapping to the edges of the frame a piece is in, and no distances shown while dragging.
