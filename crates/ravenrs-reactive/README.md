# ravenrs-reactive

Single-threaded reactive state for RavenRS: `State`, `Binding`, `Derived` and dependency-tracking observers.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS). Most apps depend on the [`ravenrs`](https://crates.io/crates/ravenrs) facade instead, which re-exports it.

```rust
use ravenrs_reactive::{Derived, State};

let count = State::new(2);
let c = count.clone();
let doubled = Derived::new(move || c.get() * 2);
count.set(21);
assert_eq!(doubled.get(), 42);
```

## License

MIT OR Apache-2.0.
