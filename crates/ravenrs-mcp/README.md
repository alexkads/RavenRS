# ravenrs-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) server that gives coding assistants accurate answers about [RavenRS](https://github.com/alexkads/RavenRS). It is a development tool: your app does not depend on it.

It is read-only and works offline. It speaks MCP over stdio, opens no sockets and reads no files. Everything it knows is embedded at build time, so it always describes the RavenRS it was built from.

## Install

```bash
cargo install --git https://github.com/alexkads/RavenRS --locked ravenrs-mcp
```

To match the RavenRS your app uses, add `--tag vYY.R.P` for a release that includes the server. From a checkout, use `cargo install --path crates/ravenrs-mcp --locked`. The server is not on crates.io, because it embeds sources and docs from the whole workspace.

## Connect

The Claude Code plugin brings this server and the RavenRS skill together. `ravenrs-mcp` must be on your `PATH`.

```bash
claude plugin marketplace add alexkads/RavenRS
```

```bash
claude plugin install ravenrs@ravenrs
```

To add only the server to Claude Code:

```bash
claude mcp add ravenrs -- ravenrs-mcp
```

Any other MCP client needs the same command:

```json
{ "mcpServers": { "ravenrs": { "command": "ravenrs-mcp" } } }
```

## Tools

| Tool | What it answers | Arguments |
|---|---|---|
| `list_components` | Components, with group and summary | `group`, `query` |
| `component_api` | Docs, signatures, fields, variants, traits, source location and quality audit of any public item | `name`, `filter` |
| `component_examples` | Real code from the examples, rustdoc and the getting started guide | `name`, `limit` |
| `design_tokens` | Colors per theme, type scale, spacing, radii, shadows, durations and springs, with their Rust names | `category`, `query` |
| `parity_status` | Status of a feature in `docs/PARITY.md` and `docs/REPORTING-PARITY.md` | `document`, `query`, `status`, `limit` |
| `validate_expression` | Syntax and schema check of a `ravenrs-criteria` expression; never evaluates it | `expression`, `schema` |
| `expression_functions` | Functions and aggregates of the expression language | `category`, `query` |

## How it works

`build.rs` embeds the Rust sources of the workspace, and the docs are embedded with `include_str!`. On the first tool call, the server parses those sources with [`syn`](https://docs.rs/syn) into an index of public items and of call chains in the examples. The first call takes about 0.1 s in a release build, and later calls are immediate. Because the answers come from the sources, the rustdoc and the docs, there is nothing separate to keep up to date.

The index reads source text, so it has two limits. It does not resolve names or expand macros, except for the local macro that defines `VStack` and `HStack`, and it cannot see inside macro calls such as `vec![...]` in examples. A test checks that every component the README lists can be found.

It uses [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk), the official Rust SDK, because the protocol still changes between revisions. That adds `tokio` and about 60 other crates to this tool only.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).
