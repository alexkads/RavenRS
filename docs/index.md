# RavenRS documentation

Build native and browser applications with one declarative UI in Rust. RavenRS draws on the GPU;
views share the same components, reactive state and design tokens on macOS, Windows, Linux and the web.

## Start with an application

```bash
npx create-ravenrs minha-loja
```

The guided setup prepares Rust, generates your project and configures coding assistants with
the API, skill and instructions from the same RavenRS release.

[Create your first project](GETTING_STARTED.md) to choose a template, run your app and build for the web.
Already have a project? [Configure the CLI and AI tools](../crates/cargo-ravenrs/README.md).

## Follow a path

- **Getting started:** install the tools, understand the project and make a window.
- **Build applications:** compose screens, use design tokens and generate forms from your records.
- **Reports:** design, render, export and serve reports.
- **Reference:** inspect component APIs, quality requirements and release compatibility.

Use the navigation on the left to explore a topic. Search with `Ctrl+K` or `⌘K`.
Each page includes an index of its sections, copyable examples and a link to its Markdown source.

## One source for every platform

A `View` is a typed Rust builder. `State<T>` drives updates, the layout engine places controls,
and the renderer draws the result. The same UI opens in a native window or on a browser canvas.
You can start with high-level components and implement a custom view when you need more control.

[Explore the architecture](../ARCHITECTURE.md), [browse the components](PARITY.md),
or [build a record application](APP-FRAMEWORK.md).
