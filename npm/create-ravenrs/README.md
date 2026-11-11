# create-ravenrs

Prepare Rust and create a RavenRS app with matching MCP tools, a skill, and project instructions.
The package uses JavaScript built-ins only. The generator and MCP server live in `cargo-ravenrs`.

```sh
npx create-ravenrs minha-loja
# or
npm create ravenrs@latest minha-loja
```

For noninteractive setup:

```sh
npx create-ravenrs minha-loja --yes --template app --target both
```

Templates: `hello`, `app`, `app-sqlite`, `relatorios`. Targets: `desktop`, `web`, `both`.
The remaining flags are passed to `cargo ravenrs new`: `--no-ai`, `--no-git`, `--no-check`,
`--plugin`. Rustup, toolchain components, WASM target, and the CLI are installed only after
confirmation, or with `--yes`. An already matching installation is reused.

The CLI is compiled locally from the release tag matching this npm package:

```sh
cargo install --git https://github.com/alexkads/RavenRS --tag vYY.R.P --locked cargo-ravenrs
```

The terminal shows a RavenRS welcome panel, six numbered steps and highlighted next commands.
Color follows the terminal and respects `NO_COLOR`; redirected output has no ANSI codes.

The first installation takes a few minutes. There are no prebuilt binaries yet.
The Rust installer is downloaded from the official static.rust-lang.org rustup distribution.
Linux system packages and Windows MSVC Build Tools are reported with manual instructions;
the bootstrap never runs sudo or installs them. Build Tools should include the C++ workload
and Windows SDK. Web-only projects skip native Linux libraries.

```sh
cd minha-loja
npx create-ravenrs doctor
```

Doctor checks only: it never installs, downloads, or prompts, even with `--yes`.
It checks Rust, the template toolchain and components, CLI version and system libraries,
then runs `cargo ravenrs doctor` to check the project's version and AI configuration.
Re-running project creation preserves application source and skips installed prerequisites.

## Development and release

```sh
npm test
npm run prepare-release
npm pack --dry-run
npm run test:e2e:linux -- /path/to/RavenRS-with-cargo-ravenrs
```

`prepare-release` reads the workspace version and toolchain and records them in the package;
`prepack` rejects stale release metadata. Publish only after a Git release tag contains
`crates/cargo-ravenrs` and its lockfile. The `v27.1.22` tag predates this CLI, so this initial
package version is a development placeholder and must advance with the CLI's first release.
Publication is manual and requires Alex's explicit request.

The Linux test runs in a disposable Docker container (Docker Desktop's Linux VM on a Mac).
It starts without Rust, uses an isolated Git fixture for the unpublished CLI tag and local
crate patches, and runs npx, cargo check, a screen render test, repeat creation and doctor.
This validates the bootstrap before release; it does not prove that a public tag or npm
package has been published. Repeat the same flow without fixtures after the real release.
