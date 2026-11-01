# ravenrs-text

Text shaping and line layout for RavenRS with Parley; Inter and a Lucide icon font embedded.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS). Most apps depend on the [`ravenrs`](https://crates.io/crates/ravenrs) facade instead, which re-exports it.

```rust
use ravenrs_text::{TextAttrs, TextSystem};

let mut text = TextSystem::new();
let mut layout = text.layout("Hello, RavenRS", &TextAttrs::default());
let size = layout.measure(Some(80.0));
assert!(size.width <= 80.5);
```

## License

MIT OR Apache-2.0.
