# ravenrs-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) server that gives coding assistants accurate answers about [RavenRS](https://github.com/alexkads/RavenRS). It is a development tool: your app does not depend on it.

It has no side effects and works offline. It speaks MCP over stdio, opens no sockets and reads or writes no files: the report tools return layouts as JSON for the assistant to save. Everything it knows is embedded at build time, so it always describes the RavenRS it was built from.

## Install

```bash
cargo install --git https://github.com/alexkads/RavenRS --locked cargo-ravenrs
cargo ravenrs ai
cargo ravenrs doctor
```

To match the RavenRS your app uses, add `--tag vYY.R.P` for a release that includes the server. From a checkout, use `cargo install --path crates/cargo-ravenrs --locked`. The standalone `ravenrs-mcp` entry point remains available for existing clients. The server is not on crates.io, because it embeds sources and docs from the whole workspace.

## Connect

The Claude Code plugin brings this server and the RavenRS skill together. `cargo-ravenrs` must be on your `PATH`. New projects receive these files automatically with `cargo ravenrs new`.

```bash
claude plugin marketplace add alexkads/RavenRS
```

```bash
claude plugin install ravenrs@ravenrs
```

To add only the server to Claude Code:

```bash
claude mcp add ravenrs -- cargo ravenrs mcp
```

Any other MCP client needs the same command:

```json
{ "mcpServers": { "ravenrs": { "command": "cargo", "args": ["ravenrs", "mcp"] } } }
```

## Tools

| Tool | What it answers | Arguments |
|---|---|---|
| `list_components` | Components, with group, summary and the page of the feature center that shows each one | `group`, `query` |
| `component_api` | Docs, signatures, fields, variants, traits, source location, quality audit and feature center page of any public item | `name`, `filter` |
| `feature_center` | Where [`examples/feature-center`](../../examples/feature-center) shows a component with all its features: the page, its id and file, the features it calls and how to open that page alone; or the issue its page waits on | `name`, `query`, `status`, `limit` |
| `component_examples` | Real code from the examples, rustdoc and the guides | `name`, `limit` |
| `design_tokens` | Colors per theme, type scale, spacing, radii, shadows, durations and springs, with their Rust names | `category`, `query` |
| `parity_status` | Status of a feature in `docs/PARITY.md`, `docs/REPORTING-PARITY.md` or, with `document: application`, `docs/APP-PARITY.md` | `document`, `query`, `status`, `limit` |
| `search_api` | Public items of any crate by words in their name or docs, when you do not know the name | `query`, `krate`, `limit` |
| `guides` | The repository's guides: the documents, the sections of one, one section, or the sections that have some words. Includes the application framework, reports, printers, NF-e and its sandbox, the point of sale and what they do not prove | `document`, `section`, `query`, `limit` |
| `app_contracts` | Scenarios C01–C19 and groups M01–M19 of `docs/APP-CONTRACTS.md`, with the issue that owns each one | `query` |
| `validate_expression` | Syntax and schema check of a `ravenrs-criteria` expression; never evaluates it | `expression`, `schema` |
| `expression_functions` | Functions and aggregates of the expression language | `category`, `query` |
| `report_layout_format` | The `.rvreport.json` format: bands, each control with a JSON example, styles, data sources and a complete layout, all produced by `ravenrs-report` itself | |
| `create_report_layout` | A table layout from sample rows, or from fields and a data URL for the report server: title, columns with captions and formats, grouping, totals, culture, landscape, accent color, checked before it is returned | `title`, `data`, `fields`, `data_url`, `data_member`, `columns`, `group_by`, `totals`, `culture`, `landscape`, `accent`, `source_name` |
| `check_report_layout` | JSON errors with line and column, every expression checked against fields, calculated fields and parameters, and a render that reports pages and expression errors | `layout`, `data`, `parameters` |
| `report_layout_templates` | The ready layouts (invoice, sales by region, sales dashboard, customer list), or the JSON of one | `name` |

## Making a report with an assistant

The report tools were exercised end to end by an assistant driving the server over stdio, as a
client does (#48), from a project with only sample rows:

1. `report_layout_format`, read once (about 10 KB), to learn the bands and controls.
2. `create_report_layout` with `title`, the rows in `data`, `group_by`, `columns` with their
   formats (`N0`, `C2`, `dd/MM/yyyy`), `totals` and `culture: "pt-BR"`. It answers the layout,
   the fields it found, a `file_name` and its own check: here, 13 expressions and one page, with
   a header and a subtotal per group, a grand total and page numbers.
3. `check_report_layout` with the layout and the same rows before saving: `valid`, no problems.
   A broken file answers `json_error` with its line and column instead.
4. The layout saved as `.rvreport.json`. `check_report_layout` reads it with the same
   `ravenrs-report` parser the report designer uses to open a file.

Nothing in that run made the assistant guess wrong, so the descriptions and the instructions
stayed as they are. Run it again when a tool changes:

```text
Using only the ravenrs MCP tools, create a "Sales by region" report from the rows in
data.json, grouped by region, with totals, in pt-BR; check it with check_report_layout
using the same rows and save it as sales.rvreport.json.
```

## How it works

`build.rs` embeds the Rust sources of every crate and example, and the Markdown at the root, under `docs` and beside each crate. `feature_center` reads `examples/feature-center/recursos.json`, the manifest the check keeps exact, and takes each page's id from the `PAGE` and `ID` constants of its file. A new crate or document comes in by itself, and it is only read as text: nothing here links against the crates it describes, so a crate with heavy or platform-only dependencies adds none here. On the first tool call, the server parses those sources with [`syn`](https://docs.rs/syn) into an index of public items and of call chains in the examples. The first call takes about 0.1 s in a release build, and later calls are immediate. Because the answers come from the sources, the rustdoc and the docs, there is nothing separate to keep up to date.

The index reads source text, so it has two limits. It does not resolve names or expand macros, except for the local macro that defines `VStack` and `HStack`, and it cannot see inside macro calls such as `vec![...]` in examples. A test checks that every component the README lists can be found.

It uses [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk), the official Rust SDK, because the protocol still changes between revisions. That adds `tokio` and about 60 other crates to this tool only.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).
