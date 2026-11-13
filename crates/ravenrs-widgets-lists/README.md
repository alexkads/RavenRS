# ravenrs-widgets-lists

RavenRS long lists and what is built on them: `VirtualList`, which builds only the rows in view, `Filmstrip`, `PagedDocumentView`, `FileBrowser`, `YearView`, `SortableList`, `DocumentTabs`, and `TreeView`.

Applications do not depend on this crate. They use [`ravenrs-widgets`](https://crates.io/crates/ravenrs-widgets) or the [`ravenrs`](https://crates.io/crates/ravenrs) facade, which re-export every name of it by the same path. It is built on [`ravenrs-widgets-base`](https://crates.io/crates/ravenrs-widgets-base) and [`ravenrs-widgets-kit`](https://crates.io/crates/ravenrs-widgets-kit), and on no other family.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

## License

MIT OR Apache-2.0.
