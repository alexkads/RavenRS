# ravenrs-xcache

A compiler cache for Rust that worktrees share. It stands in front of rustc, keeps what each library compiled to, and puts it back wherever the same inputs come up, in any build directory. Each worktree keeps a build directory of its own, so no branch ever sees another's artifacts.

**A first version: libraries only.** It is not a full replacement for sccache yet. What it does not keep goes to rustc as it came, and `ravenrs-xcache stats` says how much that was and why. The plan is at the end.

## Using it

```bash
cargo build -p ravenrs-xcache --release
RUSTC_WRAPPER=$PWD/target/release/ravenrs-xcache CARGO_INCREMENTAL=0 cargo build
```

`CARGO_INCREMENTAL=0` matters: an incremental compile depends on what the last one left in the build directory, so it cannot be kept, and Cargo compiles a workspace's own crates incrementally unless told otherwise. In this repository, `RAVENRS_XCACHE=1 scripts/check.sh` does both.

There is no service to start and nothing is installed. The cache is `~/.cache/ravenrs/xcache`, or `RAVENRS_XCACHE_DIR`.

| Command | What it does |
|---|---|
| `ravenrs-xcache stats` | Hits, misses, what went past the cache and why, bytes, and the time spent making keys, looking up, restoring and compiling. `--reset` starts the count again. |
| `ravenrs-xcache explain <crate>` | What happened to a crate's last requests, and why. The name is as rustc writes it: `ravenrs_app`. |
| `ravenrs-xcache gc --max-size 20G` | Removes the results used longest ago until the cache fits. |
| `ravenrs-xcache clear` | Empties the cache. Always safe: the next build fills it again. |

`RAVENRS_XCACHE_DISABLE=1` runs rustc as if the wrapper were not there. `RAVENRS_XCACHE_VERIFY=0` restores without checking each file against its hash.

## How it works

**Taking the paths out.** rustc writes the paths it sees into what it produces and into the crate's identity, so the same crate built under two directories is two artifacts that do not even mix (rustc refuses with E0460). The wrapper adds `--remap-path-prefix` for the build directory and for the checkout, so rustc writes `/xcache/target` and `/xcache/cwd` instead. Measured on rustc 1.99: with that, the `.rlib` and `.rmeta` of a crate built under two roots are byte for byte the same. Registry crates keep their real path, which is the same for every worktree.

**The proof, per artifact.** A crate can still embed a path as data, such as `env!("OUT_DIR")` kept in a constant. Remapping does not touch that. So after compiling, the wrapper searches the artifacts for the real paths. If none is there, the artifact does not depend on where it was built, and is shared. If one is, the artifact is kept for the paths it was built under only, and `explain` says so. When the objects carry bitcode (LTO), the search proves nothing, and the result is kept for its own paths too.

**Two keys.** The first is made of what is known before compiling: the compiler (`rustc -vV`), the arguments with the roots replaced by markers, what each `--extern` is, and the build's variables (`CARGO_PKG_*`, `CARGO_MANIFEST_DIR`, `OUT_DIR`, the deployment targets). It leads to a manifest: for each way the crate was compiled before, the source files it read and the environment variables rustc said it read, taken from the dep-info of that compile. A hit needs every one of them to be as it was. rustc is not run a second time to find them out.

**Proc macros.** A proc macro links, so it is compiled every time. Compiled with the same remapping it is the same macro in every worktree, so the crates that use it are told apart by what the macro was built from, not by the bytes of a library the linker stamped with one build directory.

**Storage.** Files are named by the SHA-256 of their content and written to a temporary file, then renamed, so a build stopped halfway leaves nothing that reads as whole. A restore checks the size and the hash; a file that changed on disk is refused, the crate is compiled, and the good file replaces it. Restoring is a copy, which is a copy-on-write clone on APFS. The dep-info Cargo reads is kept with markers and written back with the paths of the worktree it is restored into.

## What goes past the cache

| What | Why | Planned |
|---|---|---|
| Executables, test binaries, `cdylib`, `dylib` | They link; the linker's inputs are not all known | yes |
| Proc macros | They link (but their users are shared, see above) | yes |
| Clippy on workspace crates | `clippy-driver` compiles in rustc's place | yes |
| Incremental compiles | They depend on the state in the build directory | no: turn it off for the build |
| rustdoc, build script runs | They do not go through `RUSTC_WRAPPER` | yes |
| LTO libraries in another build directory | The path search cannot read bitcode | to be studied |
| `--target` spec files, `--sysroot`, `-o`, response files | Not modelled yet | when needed |

## Known limits

- A proc macro that reads a file or a variable without rustc knowing (no `include_str!`, no `env!`) can change a crate without changing its key. Cargo itself would not rebuild in that case either.
- With the wrapper on, debug information names `/xcache/cwd` instead of the checkout. In lldb: `settings set target.source-map /xcache/cwd <checkout>`.
- Nothing limits the size of the cache by itself: run `gc`.
- Tested on macOS with rustc 1.99. Linux and Windows were not run.
- No remote cache, no C or C++, no distributed compilation.

## Where it goes from here

In order: Clippy on workspace crates; executables, tests and proc macros (linking); build script runs; rustdoc; `wasm-bindgen` and `wasm-opt` as a cache of tasks, keyed by their inputs, arguments and tool versions; a limit on parallel compiles under memory pressure; C and C++; a remote store. Test results are never kept in place of running the tests.

The design read sccache, ccache, Kache, zccache and Corgi for what each does best. No code was copied.

Licensed under MIT or Apache-2.0.
