# ravenrs-core

Geometry, colors, input events and the `Scene` display list shared by every RavenRS crate.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS). Most apps depend on the [`ravenrs`](https://crates.io/crates/ravenrs) facade instead, which re-exports it.

```rust
use ravenrs_core::{Color, Quad, Rect, Scene};

let mut scene = Scene::new();
scene.push_quad(Quad::new(Rect::new(0.0, 0.0, 120.0, 40.0), Color::hex(0x4c66ee)).radius(10.0));
assert_eq!(scene.len(), 1);
```

## License

MIT OR Apache-2.0.
