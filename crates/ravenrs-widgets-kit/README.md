# ravenrs-widgets-kit

The RavenRS components of general use: navigation (`Tabs`, `Sidebar`, `Breadcrumb`), commands and toolbars, form editors and layouts, lookups, filters and expressions, status pieces, tiles, the theme editor, diagnostics and the keys of a counter. The families of components (grid, lists, dock, scheduler, imaging, charts, chat) are built on them and on [`ravenrs-widgets-base`](https://crates.io/crates/ravenrs-widgets-base).

Applications do not depend on this crate. They use [`ravenrs-widgets`](https://crates.io/crates/ravenrs-widgets) or the [`ravenrs`](https://crates.io/crates/ravenrs) facade, which re-export every name of it by the same path.

Its modules are public only for the crates of the families, and are hidden from the documentation: they are not a stable API.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

## License

MIT OR Apache-2.0.
