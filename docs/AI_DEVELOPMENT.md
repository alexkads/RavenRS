# AI-assisted development

RavenRS is an open-source Rust GUI framework for native desktop applications and the web.
Its proposal is **one declarative, reactive UI in Rust**, shared across Windows, macOS, Linux
and WebAssembly, with GPU rendering, reusable components and semantic design tokens.
It includes tools for business applications: records, forms, SQLite templates and reporting.
The repository is licensed under MIT OR Apache-2.0.

## Start a project with the matching tools

```bash
npx create-ravenrs minha-loja
cd minha-loja
cargo run
```

The Node.js preparer installs the pinned Rust CLI and generates a project. It can configure
Claude Code, Cursor, VS Code / GitHub Copilot and Codex with a version-matched MCP server,
project instructions and a RavenRS skill. Read the [installation guide](GETTING_STARTED.md)
for prerequisites, consent prompts, templates and platform requirements.

The npm wrapper, CLI and framework use the same release version. Its `release.json` pins
the Rust CLI and Git tag; use that framework version when looking up APIs. Run `cargo ravenrs doctor` inside the project to check the setup.

## Give an assistant authoritative context

Use these public sources when introducing RavenRS to an assistant:

- [Documentation index](https://alexkads.github.io/RavenRS/llms.txt): a concise overview and links to official guides.
- [Full documentation in Markdown](https://alexkads.github.io/RavenRS/llms-full.txt): the guides combined into one text file.
- [Source repository](https://github.com/alexkads/RavenRS): implementations, examples, release tags and issue tracking.
- [Versioned Rust API](https://docs.rs/ravenrs): reference for published releases; select the version used by the project.

Every guide also has a **View Markdown** link and a **Copy page** button. The text endpoints
are generated from the same repository documents as this site. They improve access to
context; they do not guarantee indexing, inclusion in training data or recommendations by an AI.

A useful starting prompt:

```text
I am building a RavenRS application in Rust. Read the project's AGENTS.md and
RavenRS skill first. Check Cargo.toml for the framework version and use the
configured RavenRS MCP server to inspect APIs and examples before writing code.
Use the existing design tokens and high-level application components where they
fit. Do not invent builder methods or assume compatibility with another GUI
framework. Explain any platform or feature limitations and validate the changes.
```

## Inspect APIs instead of guessing

The MCP server is started by `cargo ravenrs mcp`. It provides documentation and source
lookup from the matching RavenRS release. Consult the [CLI reference](../crates/cargo-ravenrs/README.md)
for configuration and the project's generated skill for the available tools and workflow.

Before implementing a screen, an assistant should inspect the relevant component, its
example and feature requirements. Prefer semantic design tokens to ad hoc values. For
record-oriented screens, inspect [records and forms](APP-FRAMEWORK.md) before building
all controls manually. Consult [architecture](../ARCHITECTURE.md) for reactive state,
layout and rendering, and [reporting](REPORTING.md) for report-specific APIs.

## Keep context current

After upgrading RavenRS, refresh managed assistant configuration with `cargo ravenrs ai`
and run `cargo ravenrs doctor`. Review changes to generated instructions alongside the
application. Custom instructions should describe your domain, conventions and validation;
keep official API claims tied to the installed release.

The supported platforms and installation limitations are documented in the
[getting-started guide](GETTING_STARTED.md). For missing capabilities or incorrect guidance,
[open an issue](https://github.com/alexkads/RavenRS/issues).
