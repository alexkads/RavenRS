# Versioning and releases

> Adopted on 2026-10-31. The first release under this model is 26.2.0, which follows 0.1.0.

RavenRS follows the release model of commercial component vendors, adapted so that every version is also valid [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html).

## The version number: `YY.R.P`

| Part | Meaning | Example |
|---|---|---|
| `YY` | Year of the major release. The only place where breaking changes happen. | `27` |
| `R` | Feature release within the year: 1 in the first half, 2 in the second. Additions only. | `27.1` |
| `P` | Maintenance release: fixes, performance and documentation. Monthly, or sooner for serious bugs. | `27.1.3` |

Vendors name releases the same way: DevExpress ships `25.1` and `25.2`, and Telerik ships `2025.1`. The difference is that in RavenRS, `27.1 → 27.2` never breaks your build. `ravenrs = "27"` in `Cargo.toml` follows every feature and maintenance release of that year automatically. Breaking changes wait for the next year's `.1` release.

## Calendar

A breaking change can bring a `YY.1.0` forward rather than slip into a feature release: 27.1.0 shipped in November 2026 for that reason.

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

1. `scripts/check.sh` passes, including the golden-image tests.
2. `cargo semver-checks` against the previous release agrees with the version bump.
3. Benchmarks are within the budgets in [QUALITY.md](QUALITY.md).
4. CHANGELOG has an entry, plus a migration guide for a major.
5. All crates are published in dependency order, then the tag `vYY.R.P`, the GitHub release, and the site and API docs are updated.
6. Every crate's documentation built on docs.rs (`https://docs.rs/crate/<name>/<version>/status.json`); it uses a recent nightly, so it can fail when stable passes.
