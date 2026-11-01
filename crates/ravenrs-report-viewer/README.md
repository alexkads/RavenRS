# ravenrs-report-viewer

Document viewer for [RavenRS Reporting](https://github.com/alexkads/RavenRS): shows a rendered report with page navigation, zoom, thumbnails and text search, and asks for its parameters. It works the same on the desktop and in the browser.

```rust
let state = ViewerState::new();
let viewer = DocumentViewer::new(document, &state);
```

Licensed under MIT or Apache-2.0.
