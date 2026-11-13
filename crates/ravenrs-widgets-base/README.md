# ravenrs-widgets-base

The base of the RavenRS widgets: the `Ui` runtime, `View` and `BuildCx`, motion, the services the platform provides (clipboard, links, files, sound), the value types the components share, and the basic controls every other component is made of (buttons, stacks, text fields, overlays).

Applications do not depend on this crate. They use [`ravenrs-widgets`](https://crates.io/crates/ravenrs-widgets) or the [`ravenrs`](https://crates.io/crates/ravenrs) facade, which re-export every name of it by the same path. The crate exists so a change to one family of components does not rebuild, or need reading, the rest.

Its modules are public only for the crates of the components, and are hidden from the documentation: they are not a stable API.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

## License

MIT OR Apache-2.0.
