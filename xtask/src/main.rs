//! `cargo xtask <task>`: the project's checks and builds, one subcommand per purpose (#1034).
//!
//! It is Rust, so it runs the same on macOS, Linux and Windows, and it uses the standard library
//! only, so it compiles in seconds before any check starts. The helper scripts it runs are Python.

mod cache;
mod checks;
mod guard;
mod run;
mod web;

use std::path::PathBuf;

const HELP: &str = "\
cargo xtask <task>: the project's checks and builds, one task per purpose.

  dev       while working, before a commit (minutes): formatting, the cheap checks of the
            workspace, then clippy and the tests of the packages changed only
  pr        the pull request's gate, on the developer's machine: clippy on every package the
            change reaches, the tests of those changed and of their direct dependents, the design
            system and the public API when the change reaches them
  release   before a release: the whole workspace and all its tests, wasm32, optional features,
            the minimum Rust version, dependencies, API docs and the web builds
            (RAVENRS_GATE=1 adds the benchmarks and the performance gate)
  web <name> [cargo flags]
            builds an example or a tool for the browser into its web/pkg

The checks take no arguments: each one does one thing. They compare with origin/master, or with
CHECK_BASE. RAVENRS_CHECK_TIMINGS=1 adds Cargo's per-crate report; RAVENRS_XCACHE=0 turns the
compiler cache off. See the skill ravenrs-check.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn dispatch(args: &[String]) -> i32 {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().map(PathBuf::from).unwrap_or_default();
    let (task, rest) = match args.split_first() {
        Some((task, rest)) => (task.as_str(), rest),
        None => ("help", &[][..]),
    };
    let check: fn(&mut run::Check) -> Result<&'static str, run::Failed> = match task {
        "dev" => checks::dev,
        "pr" => checks::pr,
        "release" => checks::release,
        "web" => return web_task(&root, rest),
        "help" | "--help" | "-h" => {
            print!("{HELP}");
            return 0;
        }
        other => {
            eprintln!("error: no task named {other}\n\n{HELP}");
            return 2;
        }
    };
    if !rest.is_empty() {
        eprintln!("error: cargo xtask {task} takes no arguments, and got: {}", rest.join(" "));
        eprintln!(
            "Each check does one thing: dev while working, pr for a pull request, release before a release."
        );
        return 2;
    }
    let mut state = match run::Check::new(task, &root) {
        Ok(state) => state,
        Err(failed) => {
            eprintln!("error: {failed}");
            return failed.code.max(1);
        }
    };
    if let Err(failed) = nextest_installed(&state) {
        eprintln!("error: {failed}");
        return 1;
    }
    let cache = match cache::Cache::turn_on(&mut state) {
        Ok(cache) => cache,
        Err(failed) => {
            eprintln!("error: the compiler cache: {failed}");
            return failed.code.max(1);
        }
    };
    let mut passed = "";
    let done = check(&mut state).map(|message| passed = message);
    state.finish(done, passed, &cache)
}

/// The checks run the tests on cargo-nextest, which every contributor installs once.
fn nextest_installed(check: &run::Check) -> Result<(), run::Failed> {
    check
        .output("cargo", &["nextest", "--version"])
        .map(|_| ())
        .map_err(|_| run::failure("the checks run the tests on cargo-nextest; install it once with: cargo install cargo-nextest --locked"))
}

fn web_task(root: &std::path::Path, rest: &[String]) -> i32 {
    let Some((name, flags)) = rest.split_first() else {
        eprintln!("error: cargo xtask web <name> [cargo flags]: which example or tool?");
        return 2;
    };
    let check = match run::Check::new("web", root) {
        Ok(check) => check,
        Err(failed) => {
            eprintln!("error: {failed}");
            return 1;
        }
    };
    let flags: Vec<&str> = flags.iter().map(String::as_str).collect();
    match web::build(&check, name, &flags) {
        Ok(()) => 0,
        Err(failed) => {
            eprintln!("error: {failed}");
            failed.code.max(1)
        }
    }
}
