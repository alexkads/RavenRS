# ravenrs-widgets-charts

RavenRS charts and diagrams: `Chart`, `BarChart`, `AreaChart`, `Barcode` and `SankeyChart` (feature `charts`), and `FlowCanvas`, diagrams of nodes and connections (feature `flow`). They are drawn with `ravenrs-graphics`, the same as reports draw them.

Applications do not depend on this crate. They use [`ravenrs-widgets`](https://crates.io/crates/ravenrs-widgets) or the [`ravenrs`](https://crates.io/crates/ravenrs) facade, which re-export every name of it by the same path. It is built on [`ravenrs-widgets-base`](https://crates.io/crates/ravenrs-widgets-base) and [`ravenrs-widgets-kit`](https://crates.io/crates/ravenrs-widgets-kit), and on no other family.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

## License

MIT OR Apache-2.0.
