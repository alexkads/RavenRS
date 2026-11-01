# ravenrs-layout

Flexbox and grid layout for RavenRS, a typed facade over Taffy.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS). Most apps depend on the [`ravenrs`](https://crates.io/crates/ravenrs) facade instead, which re-exports it.

```rust
use ravenrs_layout::{Axis, LayoutStyle, LayoutTree};

use ravenrs_core::Size;
let mut tree = LayoutTree::<()>::new();
let a = tree.node(LayoutStyle { height: Some(20.0), ..LayoutStyle::leaf() }, &[]).unwrap();
let root = tree.node(LayoutStyle { spacing: 8.0, ..LayoutStyle::stack(Axis::Vertical) }, &[a]).unwrap();
tree.compute(root, Size::new(200.0, 200.0), |_, _| Size::ZERO).unwrap();
assert_eq!(tree.frame(a).unwrap().height(), 20.0);
```

## License

MIT OR Apache-2.0.
