# ravenrs-style

Design tokens and the Raven Dark, Raven Light and Raven Glass themes.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS). Most apps depend on the [`ravenrs`](https://crates.io/crates/ravenrs) facade instead, which re-exports it.

```rust
use ravenrs_style::{Semantic, Theme};

let theme = Theme::raven_dark();
let accent = theme.color(Semantic::Accent);
assert!(accent.contrast_ratio(theme.color(Semantic::OnAccent)) >= 4.5);
```

## License

MIT OR Apache-2.0.
