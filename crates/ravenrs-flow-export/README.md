# ravenrs-flow-export

Saves a [RavenRS](https://github.com/alexkads/RavenRS) `FlowCanvas` diagram as a file: a PNG picture at twice its size, an SVG drawing, or a PDF page on white paper. The canvas hands over a picture of the diagram (or of what is selected) from its Export menu; this crate writes it, with the words drawn in the same fonts as the screen.

```rust,ignore
FlowCanvas::new(graph.binding())
    .on_export(ravenrs_flow_export::saver(file_picker, "Order flow"))
```

`ravenrs-widgets` knows nothing of fonts in pixels or of PDF, and the report engine knows nothing of diagrams. This crate is what joins them, as `ravenrs-grid-export` does for tables.
