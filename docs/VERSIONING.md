# Versioning and releases

> Adopted on 2026-10-31. The first release under this model is 26.2.0, which follows 0.1.0.

RavenRS follows the release model of commercial component vendors, adapted so that every version is also valid [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html).

## The version number: `YY.R.P`

| Part | Meaning | Example |
|---|---|---|
| `YY` | Year of the major release. The only place where breaking changes happen. | `27` |
| `R` | Feature release within the year: 1 in the first half, 2 in the second. Additions only. | `27.1` |
| `P` | Maintenance release: fixes, performance and documentation. Monthly, or sooner for serious bugs. | `27.1.3` |

Commercial component vendors name releases the same way, such as `25.1` and `25.2`, or `2025.1`. The difference is that in RavenRS, `27.1 → 27.2` never breaks your build. `ravenrs = "27"` in `Cargo.toml` follows every feature and maintenance release of that year automatically. Breaking changes wait for the next year's `.1` release, from 28.1.0 on: see below for what holds until then.

## Calendar

A breaking change can bring a `YY.1.0` forward rather than slip into a feature release: 27.1.0 shipped in November 2026 for that reason. That was the last time: **`YY` and `R` now follow the calendar only**. Until 27.2.0, in the second half of 2027, every release is the next `27.1.P`.

**Until 28.1.0, any release may hold breaking changes**, listed under *Breaking* in the CHANGELOG. The library is still taking shape, and holding its API still costs more than it saves. So until then `ravenrs = "27"` can break your build on `cargo update`: pin an exact version, `ravenrs = "=27.1.8"`, if that matters to you. The table below is what holds from 28.1.0 on.

| When | Release | What it may contain |
|---|---|---|
| First half of the year | `YY.1.0` | New components and features, **and** the year's breaking changes, each listed in a migration guide |
| Second half of the year | `YY.2.0` | New components and features only |
| Monthly | `YY.R.P` | Fixes only |

Each feature release has a public beta (`YY.R.0-beta.N`) about four weeks before the final release.

## Support

- The current year's line gets fixes. The previous year's line gets security and critical fixes for 12 months after the next major ships.
- **MSRV.** A feature release may raise the minimum Rust version to one at least six months old. A maintenance release never raises it.
- Deprecations are announced in a feature release with `#[deprecated]` and a replacement, and are removed no earlier than the next major.

## What counts as breaking

Breaking changes are the ones that Cargo SemVer and [`cargo semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks) treat as major. Enums that grow are `#[non_exhaustive]`, and builders have defaults, so new variants, fields and options are additions. Visual changes count too: a change that moves pixels in the golden-image tests ships only in a feature or major release, with a CHANGELOG entry.

## Release checklist

1. `scripts/check.sh --full` passes, including the golden-image tests.
2. `cargo semver-checks` runs against the previous release. Until 28.1.0, what it finds goes under *Breaking* in the CHANGELOG; from then on it has to agree with the version bump.
3. Benchmarks are within the budgets in [QUALITY.md](QUALITY.md).
4. CHANGELOG has an entry, plus a migration guide for a major.
5. All crates are published in dependency order, then the tag `vYY.R.P`, the GitHub release, and the site and API docs are updated.
6. Every crate's documentation built on docs.rs (`https://docs.rs/crate/<name>/<version>/status.json`); it uses a recent nightly, so it can fail when stable passes.

## npm preparer patches

`npm/create-ravenrs/release.json` pins the Rust CLI version and Git tag used by the preparer.
An npm-only documentation or installer patch can increment the npm patch version while
keeping that Rust pin. The major and minor versions must match and the npm patch cannot
be below the framework patch. `--version` reports both versions.

For an npm-only release, increment `package.json` and its lockfile, run the Node tests and
`npm run prepack --prefix npm/create-ravenrs`, inspect `npm pack --dry-run`, then publish
from the package directory. Do not create a Rust `vYY.R.P` tag for an npm-only patch.
For a new framework release, run `prepare-release` and ensure the npm version is unpublished
before publishing. The prepack guard checks the pinned workspace release and toolchain;
it does not query the registry for previously published versions.

## CLI patches without a framework release

`cargo-ravenrs` can use an explicit package version for a generator fix. Its framework version
comes from `ravenrs-project`, so templates, dependency pins, skill and MCP remain matched.
When the CLI version differs from the framework, publish an annotated `cargo-ravenrs-v<CLI>`
tag and record both versions in the npm `release.json` with `prepare-release`. The npm launcher
compares the installed CLI against `cliVersion`; doctor checks application crates against the
framework version. Advance the CLI version whenever its framework pin changes, and keep the
npm patch at least as high as the CLI patch. The prepare/prepack guard checks these constraints.
