# flow-design-board

A design board made only with library components (`FlowCanvas`, `FlowToolbox`, `FlowInspector`, `FlowLibrary` and more): screens as frames, the pieces of each screen placed freely, resized, turned and lined up with the guides, and arrows between screens saying where a tap leads. It is here to check how far the diagram component goes as a board of this kind.

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

## What it needed, now in the library

Building it showed what the component lacked, and each piece went into the library first:
`FlowStyle` (fill, line and text colors and widths per piece), `FlowShape::Text` for words
alone, images through `FlowCanvas::content`, `FlowLayers` to pick, hide and reorder, guides that
line a piece up with the edges of its frame, and ports that show only under the pointer.

Still out, as they belong to a drawing tool rather than a diagram: drawn paths, boolean shapes
and distances shown while dragging.

## In the browser, on Linux and on Windows

In the browser, the diagrams are kept in the browser's own storage:

```bash
cargo xtask web flow-design-board
python3 -m http.server -d examples/flow-design-board/web 8080
```

On Linux and Windows, run `cargo run -p flow-design-board --release` and check that you can drag a shape onto a frame and turn it by its handle; hide a piece in Camadas and show it again; Exportar as PNG; Salvar, close and open again.

## End-to-end tests (Raven Test Studio)

The scenarios of this example are in `tests/e2e/`, as `.raventest` files the Raven Test Studio reads: the sample board has twelve pieces; named and saved, it shows on the list. The app brings them (`SCENARIOS`), and `tests/scenarios.rs` plays them on every `cargo test`.

To run them as the Studio does, with the app in a process of its own, no window and no cache:

```bash
raven-test-studio --project examples/flow-design-board --run-all --no-cache --format json --out target/e2e/flow-design-board
```

The app answers the Studio when opened with `--e2e-serve`, on data in memory: the files it keeps on disk are never opened. Its name on the channel, which the `open` block of a scenario uses, is "Design board" (`[package.metadata.ravenrs.e2e]` in `Cargo.toml`). A step that fails says the step, why, and the screen of the app at that moment; with `--film` the report keeps a film of the failure.

