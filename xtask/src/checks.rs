//! The three checks and the steps they share.

use crate::run::{Check, Done, PYTHON, failure};

/// The packages a change touches and reaches, as scripts/changed-packages.py says.
#[derive(Debug, Default)]
pub struct Selection {
    /// Something every package builds with changed: the whole workspace.
    pub all: bool,
    /// The packages whose files changed.
    pub touched: Vec<String>,
    /// Those and their direct dependents, whose tests the pull request runs.
    pub tested: Vec<String>,
    /// The tested packages with doctests.
    pub documented: Vec<String>,
    /// Every package the change reaches, at any depth, which clippy proves.
    pub reached: Vec<String>,
    /// The lint of the examples changed or reaches them.
    pub lint: bool,
    /// Direct dependents left to the release check: they would build a heavy crate the change does
    /// not touch (#348).
    pub deferred: Vec<String>,
}

impl Selection {
    /// Reads the `--shell` output of changed-packages.py: `NAME="a b c"` or `NAME=1`, one a line.
    pub fn parse(text: &str) -> Selection {
        let mut selection = Selection::default();
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else { continue };
            let value = value.trim().trim_matches('"');
            let list = || value.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
            match name.trim() {
                "ALL" => selection.all = value == "1",
                "TOUCHED" => selection.touched = list(),
                "TESTED" => selection.tested = list(),
                "DOCUMENTED" => selection.documented = list(),
                "REACHED" => selection.reached = list(),
                "LINT" => selection.lint = value == "1",
                "DEFERRED" => selection.deferred = list(),
                _ => {}
            }
        }
        selection
    }
}

/// `-p a -p b …` for cargo.
fn packages(names: &[String]) -> Vec<String> {
    names.iter().flat_map(|name| ["-p".to_owned(), name.clone()]).collect()
}

fn strs(owned: &[String]) -> Vec<&str> {
    owned.iter().map(String::as_str).collect()
}

/// The checks of the whole workspace that cost seconds, which every check runs first.
pub fn cheap(check: &mut Check) -> Done {
    check.step("rustfmt");
    check.run("cargo", &["fmt", "--all", "--check"])?;

    check.step(
        "the shell check does not come back: not the old scripts, no new *.sh, no citation of them (#1068)",
    );
    let listed = check.output("git", &["ls-files", "-z", "--cached", "--others", "--exclude-standard"])?;
    crate::guard::shell_check_stays_gone(&check.root, &listed)?;

    check.step(
        "modules do not use each other in loops longer than each crate allows (ravenrs-lint module_cycle)",
    );
    check
        .run("cargo", &["run", "-q", "-p", "ravenrs-lint", "--", "--workspace", "--only", "module_cycle"])?;

    check.step("the widgets' crates keep their layers: no family depends on another");
    check.run(PYTHON, &["scripts/check-widget-layers.py"])?;

    check.step("every test file runs: tests/suite/ modules are declared");
    check.run(PYTHON, &["scripts/check-test-suites.py"])?;

    check.step("the playground's live lists are current, and name nothing another feature leaves out (#828)");
    check.run(PYTHON, &["scripts/gen-live.py", "--self-test"])?;
    check.run(PYTHON, &["scripts/gen-live.py", "--check"])?;

    check.step("every tool in tools/ is a product: version, news, installer, icons, tests (tools/README.md)");
    check.run(PYTHON, &["scripts/check-tools.py"])?;

    check.step("the feature center shows every component with every feature (skill ravenrs-feature-center)");
    check.run(PYTHON, &["scripts/feature-center-coverage.py"])?;

    check.step(
        "every component follows RCB: ComponentBook with its stories, or pending with its issue (#754, #768)",
    );
    check.run(PYTHON, &["scripts/rcb-coverage.py", "--self-test"])?;
    check.run(PYTHON, &["scripts/rcb-coverage.py"])?;

    check.step(
        "every type keeps the testability contract of the RTS, or is pending with its issue (#734, #736)",
    );
    check.run(PYTHON, &["scripts/rts-coverage.py", "--self-test"])?;
    check.run(PYTHON, &["scripts/rts-coverage.py"])?;

    check.step("the components say no fixed words: what they show or read aloud comes from Labels (#775)");
    check.run(PYTHON, &["scripts/check-fixed-texts.py", "--self-test"])?;
    check.run(PYTHON, &["scripts/check-fixed-texts.py"])?;

    check.step("every app with a window keeps scenarios the Raven Test Studio finds and runs (CONTRIBUTING.md, #702)");
    check.run(PYTHON, &["scripts/e2e-scenarios.py", "--self-test"])?;
    check.run(PYTHON, &["scripts/e2e-scenarios.py"])
}

/// What the change touches and reaches. The selector's own tests run first: what it leaves out
/// goes unchecked.
pub fn select(check: &mut Check) -> Result<Selection, crate::run::Failed> {
    check.step("what the change reaches");
    if check.output(PYTHON, &["scripts/changed_packages_test.py"]).is_err() {
        check.run(PYTHON, &["scripts/changed_packages_test.py"])?;
    }
    let shell = check.output(PYTHON, &["scripts/changed-packages.py", "--shell"])?;
    Ok(Selection::parse(&shell))
}

fn clippy(check: &mut Check, selection: &[String]) -> Done {
    let mut args = vec!["clippy".to_owned()];
    args.extend(selection.iter().cloned());
    args.push("--all-targets".to_owned());
    if check.cargo_timings {
        args.push("--timings".to_owned());
    }
    args.extend(["--", "-D", "warnings"].map(str::to_owned));
    check.run("cargo", &strs(&args))
}

/// The tests of a selection on nextest, compiling and running apart so the table says where the
/// time went, then the doctests, which nextest does not run, through `cargo test --doc`.
///
/// `pr` and `release` run every test even after one fails (`--no-fail-fast`), so a red run lists
/// all that broke at once instead of the first; `dev` stops at the first, to answer fast while
/// one works.
fn tests(check: &mut Check, selection: &[String], documented: Option<&[String]>) -> Done {
    let whole_report = !check.mode.starts_with("dev");
    let mut compile = vec!["nextest".to_owned(), "run".to_owned()];
    compile.extend(selection.iter().cloned());
    let mut run = compile.clone();
    compile.push("--no-run".to_owned());
    if check.cargo_timings {
        compile.push("--timings".to_owned());
    }
    run.push("--no-tests=pass".to_owned());
    if whole_report {
        run.push("--no-fail-fast".to_owned());
    }
    check.substep("compile the tests", "cargo", &strs(&compile))?;
    check.substep("run the tests", "cargo", &strs(&run))?;
    let mut doc = match documented {
        None => vec!["test".to_owned(), "--doc".to_owned(), "--workspace".to_owned()],
        Some([]) => return Ok(()),
        Some(names) => [vec!["test".to_owned(), "--doc".to_owned()], packages(names)].concat(),
    };
    if whole_report {
        doc.push("--no-fail-fast".to_owned());
    }
    check.substep("doctests (compile and run)", "cargo", &strs(&doc))
}

fn public_api(check: &mut Check) -> Done {
    check.step("the public API of ravenrs-widgets is unchanged (rustdoc JSON, on nightly)");
    check.run(PYTHON, &["scripts/public-api.py", "--check"])
}

fn design_system(check: &mut Check) -> Done {
    // RCB measures every story again: only when the change reaches it (#1000).
    check.step("the design system in docs/design-system/export is what RCB writes (#1000)");
    check.run(PYTHON, &["scripts/gen-design-system.py", "--self-test"])?;
    check.run(PYTHON, &["scripts/gen-design-system.py", "--check"])
}

fn lint_examples(check: &mut Check) -> Done {
    // The examples are the showcase, and the house follows its own rules.
    check.step("ravenrs-lint on the examples");
    for example in ["family-finance", "component-gallery"] {
        check.run("cargo", &["run", "-q", "-p", "ravenrs-lint", "--", "-p", example])?;
    }
    Ok(())
}

/// The design system needs checking when the change reaches RCB, or touches the copy, its script
/// or a file RCB embeds from outside any package (the guide, include_str!, and the catalog).
fn design_reached(check: &Check, selection: &Selection) -> bool {
    if selection.reached.iter().any(|name| name == "raven-component-book") {
        return true;
    }
    let base = std::env::var("CHECK_BASE").unwrap_or_else(|_| "origin/master".to_owned());
    let Ok(fork) = check.output("git", &["merge-base", "HEAD", &base]) else { return false };
    let paths = [
        "docs/design-system/export",
        "scripts/gen-design-system.py",
        "docs/design-system/README.md",
        "site/components/index.html",
    ];
    let mut diff = vec!["diff", "--quiet", fork.trim(), "--"];
    diff.extend(paths);
    let changed = !check.command("git", &diff).status().is_ok_and(|s| s.success());
    let untracked = check
        .output("git", &["ls-files", "--others", "--exclude-standard", "--", "docs/design-system/export"])
        .is_ok_and(|out| !out.trim().is_empty());
    changed || untracked
}

/// `cargo xtask lint`: the fastest loop, with no tests at all. Formatting, the cheap checks and
/// clippy on the packages changed, the same as `dev` up to its tests. It never stands in for a
/// gate: the tests are `dev`'s while working and `pr`'s before a merge.
pub fn lint(check: &mut Check) -> Result<&'static str, crate::run::Failed> {
    cheap(check)?;
    let selection = select(check)?;
    if selection.all {
        println!("something every package builds with changed: clippy on the whole workspace");
        check.mode = "lint, ALL".to_owned();
        check.step("clippy (native), the whole workspace");
        clippy(check, &["--workspace".to_owned()])?;
        return Ok("Lint passed, no tests run (before the pull request: cargo xtask pr)");
    }
    if selection.touched.is_empty() {
        println!("no package changed against {}: nothing to lint", base_name());
        return Ok("Lint passed");
    }
    println!("changed: {}", selection.touched.join(" "));
    check.step("clippy (native), on the packages changed");
    clippy(check, &packages(&selection.touched))?;
    Ok(
        "Lint passed, no tests run (before a commit: cargo xtask dev; before the pull request: cargo xtask pr)",
    )
}

/// `cargo xtask dev`: while working, before a commit. Formatting, the cheap checks, then clippy,
/// the tests and the doctests of the packages changed only. Their dependents, the design system
/// and the public API are the pull request's.
pub fn dev(check: &mut Check) -> Result<&'static str, crate::run::Failed> {
    cheap(check)?;
    let selection = select(check)?;
    if selection.all {
        println!("something every package builds with changed: clippy proves the whole workspace compiles;");
        println!("its tests are the pull request's (cargo xtask pr)");
        check.mode = "dev, ALL".to_owned();
        check.step("clippy (native), the whole workspace");
        clippy(check, &["--workspace".to_owned()])?;
        return Ok("Dev checks passed (the tests: cargo xtask pr)");
    }
    if selection.touched.is_empty() {
        println!("no package changed against {}: nothing to build", base_name());
        return Ok("Dev checks passed");
    }
    println!("changed: {}", selection.touched.join(" "));
    println!(
        "left to cargo xtask pr: the {} other packages it reaches",
        selection.reached.len().saturating_sub(selection.touched.len())
    );
    let touched = packages(&selection.touched);
    check.step("clippy (native), on the packages changed");
    clippy(check, &touched)?;
    check.step("tests, on the packages changed");
    let documented: Vec<String> =
        selection.documented.iter().filter(|name| selection.touched.contains(name)).cloned().collect();
    tests(check, &touched, Some(documented.as_slice()))?;
    Ok("Dev checks passed (for the pull request: cargo xtask pr)")
}

fn base_name() -> String {
    std::env::var("CHECK_BASE").unwrap_or_else(|_| "origin/master".to_owned())
}

/// `cargo xtask pr`: the pull request's gate. Clippy on every package the change reaches, at any
/// depth; the tests of the packages changed and of their direct dependents; the design system and
/// the public API when the change reaches them.
pub fn pr(check: &mut Check) -> Result<&'static str, crate::run::Failed> {
    cheap(check)?;
    let selection = select(check)?;
    if selection.all {
        println!("something every package builds with changed: the whole workspace");
        check.mode = "pr, ALL".to_owned();
        lint_examples(check)?;
        public_api(check)?;
        design_system(check)?;
        check.step("clippy (native), the whole workspace");
        clippy(check, &["--workspace".to_owned()])?;
        check.step("tests, the whole workspace");
        tests(check, &["--workspace".to_owned()], None)?;
        return Ok("Pull request checks passed (before a release: cargo xtask release)");
    }
    if selection.lint {
        lint_examples(check)?;
    }
    // rustdoc on nightly takes a while: only when the change reaches the facade it reads.
    if selection.reached.iter().any(|name| name == "ravenrs-widgets") {
        public_api(check)?;
    }
    if design_reached(check, &selection) {
        design_system(check)?;
    }
    if selection.reached.is_empty() {
        println!("no package changed against {}: nothing to build", base_name());
        return Ok("Pull request checks passed");
    }
    println!("changed:         {}", selection.touched.join(" "));
    println!("tested:          {}", selection.tested.join(" "));
    println!("linted, at most: {}", selection.reached.join(" "));
    if !selection.deferred.is_empty() {
        // Their tests would compile the local AI or the video encoder the change does not touch (#348).
        println!("tested in cargo xtask release: {}", selection.deferred.join(" "));
    }
    check.step("clippy (native), on every package the change reaches");
    clippy(check, &packages(&selection.reached))?;
    check.step("tests, on the packages changed and their direct dependents");
    tests(check, &packages(&selection.tested), Some(selection.documented.as_slice()))?;
    Ok("Pull request checks passed (before a release: cargo xtask release)")
}

/// The crates the wasm32 clippy covers together.
const WASM: &[&str] = &[
    "ravenrs",
    "ravenrs-widgets",
    "ravenrs-widgets-base",
    "ravenrs-widgets-kit",
    "ravenrs-widgets-grid",
    "ravenrs-widgets-lists",
    "ravenrs-widgets-scheduler",
    "ravenrs-widgets-dock",
    "ravenrs-widgets-imaging",
    "ravenrs-widgets-charts",
    "ravenrs-widgets-chat",
    "ravenrs-platform-web",
    "ravenrs-report",
    "ravenrs-report-pdf",
    "ravenrs-report-xlsx",
    "ravenrs-report-viewer",
    "ravenrs-grid-export",
    "ravenrs-app",
    "ravenrs-app-i18n",
    "ravenrs-app-a11y",
    "ravenrs-app-ui",
    "ravenrs-app-remote",
    "ravenrs-app-library",
    "ravenrs-app-report",
    "ravenrs-report-designer",
    "quick-app",
    "quick-plan",
    "raven-feature-center",
    "calculator",
    "hello-world",
    "component-gallery",
    "raven-report-designer",
    "ravenrs-sheet",
    "ravenrs-spreadsheet",
    "spreadsheet",
    "tutorial-tasks",
    "tasks-sqlite",
    "ravenrs-barcode",
    "ravenrs-pos",
    "ravenrs-report-label",
];

/// On their own: together with the rest, Cargo unifies features and rand_core asks for getrandom.
const WASM_ALONE: &[&str] = &["ravenrs-dfe", "ravenrs-nfe", "ravenrs-nfse", "ravenrs-pagamento"];

/// What the minimum Rust version does not build.
const NOT_ON_MSRV: &[&str] = &[
    "ravenrs-movie-video",
    "raven-channel-video",
    "tasks-sqlite",
    "ravenrs-sheet",
    "ravenrs-pdf",
    "ravenrs-word",
    "ravenrs-deck",
    "ravenrs-app-testkit",
    "ravenrs-spreadsheet",
    "spreadsheet",
    "ravenrs-app-sqlite",
    "ravenrs-flows-runtime",
    "raven-flows",
    "mini-pdv",
    "calculator",
    "hello-world",
    "quick-app",
    "quick-plan",
    "raven-feature-center",
    "component-gallery",
    "raven-report-designer",
    "grid-poc",
    "family-finance",
    "ravenrs-perf",
];

/// What the API docs leave out: applications, whose docs nobody reads.
const NOT_DOCUMENTED: &[&str] = &[
    "raven-channel-video",
    "mini-pdv",
    "calculator",
    "hello-world",
    "quick-app",
    "quick-plan",
    "raven-feature-center",
    "component-gallery",
    "raven-report-designer",
    "grid-poc",
    "family-finance",
    "ravenrs-perf",
    "xtask",
];

/// The examples whose web build the site publishes.
const WEB: &[&str] = &[
    "hello-world",
    "calculator",
    "quick-app",
    "quick-plan",
    "raven-feature-center",
    "component-gallery",
    "raven-report-designer",
];

fn with_flag(flag: &str, names: &[&str]) -> Vec<String> {
    names.iter().flat_map(|name| [flag.to_owned(), (*name).to_owned()]).collect()
}

/// `cargo xtask release`: before a release, everything. The whole workspace and all its tests,
/// wasm32, optional features, the minimum Rust version, dependencies, API docs and the web builds.
/// Benchmarks and the performance gate only with RAVENRS_GATE=1.
pub fn release(check: &mut Check) -> Result<&'static str, crate::run::Failed> {
    cheap(check)?;
    lint_examples(check)?;
    public_api(check)?;
    design_system(check)?;

    check.step("clippy (native)");
    clippy(check, &["--workspace".to_owned()])?;

    check.step("CLI templates: every model and target compiles");
    check.run("cargo", &["test", "-p", "cargo-ravenrs", "--test", "cli", "--", "--ignored"])?;

    check.step("clippy (wasm32)");
    for list in [WASM, WASM_ALONE] {
        let mut args = vec!["clippy".to_owned()];
        args.extend(with_flag("-p", list));
        args.extend(["--target", "wasm32-unknown-unknown", "--", "-D", "warnings"].map(str::to_owned));
        check.run("cargo", &strs(&args))?;
    }

    check.step("tests: unit, behavior, property, doc, GPU pixel and golden-image (GPU tests skip without an adapter)");
    tests(check, &["--workspace".to_owned()], None)?;

    check.step("ravenrs-dfe and ravenrs-nfe with the real network code, against local servers only");
    check.run("cargo", &["test", "-p", "ravenrs-dfe", "-p", "ravenrs-nfe", "--all-features"])?;

    check.step("optional features are optional");
    check.run("cargo", &["check", "-p", "ravenrs", "--no-default-features"])?;
    let families = [
        "ravenrs-widgets-base",
        "ravenrs-widgets-kit",
        "ravenrs-widgets-grid",
        "ravenrs-widgets-lists",
        "ravenrs-widgets-scheduler",
        "ravenrs-widgets-dock",
        "ravenrs-widgets-imaging",
        "ravenrs-widgets-charts",
        "ravenrs-widgets-chat",
        "ravenrs-widgets",
    ];
    let mut args = vec!["check".to_owned()];
    args.extend(with_flag("-p", &families));
    args.push("--no-default-features".to_owned());
    check.run("cargo", &strs(&args))?;
    let tree = check.output("cargo", &["tree", "-p", "ravenrs", "--no-default-features", "-e", "normal"])?;
    if tree.lines().any(|line| line.contains("ravenrs-graphics") || line.contains("tiny-skia")) {
        return Err(failure("an app without the charts feature must not build the rasterizer"));
    }

    check.step("minimum supported Rust version (1.88)");
    let toolchains = check.output("rustup", &["toolchain", "list"]).unwrap_or_default();
    if toolchains.lines().any(|line| line.starts_with("1.88")) {
        let mut args = vec!["+1.88".to_owned(), "check".to_owned(), "--workspace".to_owned()];
        args.extend(with_flag("--exclude", NOT_ON_MSRV));
        check.run("cargo", &strs(&args))?;
    } else {
        println!("skipped: install it with 'rustup toolchain install 1.88'");
    }

    check.step("dependencies: advisories, licenses, sources (cargo-deny)");
    if check.output("cargo", &["deny", "--version"]).is_ok() {
        check.run("cargo", &["deny", "check"])?;
    } else {
        println!("skipped: install it with 'cargo install cargo-deny --locked'");
    }

    check.step("every example downloads from the playground as a project that builds");
    check.run(
        "cargo",
        &["test", "-p", "component-gallery", "--test", "suite", "projects::", "--", "--ignored"],
    )?;

    if std::env::var("RAVENRS_GATE").is_ok_and(|v| v == "1") {
        check.step("benchmarks compile");
        check.run("cargo", &["bench", "--workspace", "--no-run", "--quiet"])?;
        check.step("performance gate: exact counters against perf/baseline.json");
        check.run("cargo", &["run", "--release", "-q", "-p", "ravenrs-perf", "--", "gate"])?;
    }

    check.step("API docs (broken links fail)");
    let mut args = ["doc", "--no-deps", "--workspace", "--features", "ravenrs-widgets/code", "--quiet"]
        .map(str::to_owned)
        .to_vec();
    args.extend(with_flag("--exclude", NOT_DOCUMENTED));
    let mut doc = check.command("cargo", &strs(&args));
    doc.env("RUSTDOCFLAGS", "-D warnings");
    check.run_command(doc, "cargo", &strs(&args))?;
    check.run(PYTHON, &["scripts/check-component-list.py"])?;

    check.step("web builds");
    for example in WEB {
        crate::web::build(check, example, &[])?;
    }
    Ok("All checks passed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selection_is_read_from_the_selector_as_it_writes_it() {
        let text = "ALL=0\nTOUCHED=\"ravenrs-widgets-base\"\nTESTED=\"ravenrs-widgets ravenrs-widgets-base\"\n\
                    DOCUMENTED=\"ravenrs-widgets-base\"\nREACHED=\"ravenrs ravenrs-widgets ravenrs-widgets-base\"\n\
                    LINT=0\nDEFERRED=\"\"\n";
        let selection = Selection::parse(text);
        assert!(!selection.all);
        assert_eq!(selection.touched, ["ravenrs-widgets-base"]);
        assert_eq!(selection.tested.len(), 2);
        assert_eq!(selection.reached.len(), 3);
        assert!(selection.deferred.is_empty());
        assert!(Selection::parse("ALL=1\n").all);
    }

    #[test]
    fn packages_become_one_flag_each() {
        assert_eq!(packages(&["a".to_owned(), "b".to_owned()]), ["-p", "a", "-p", "b"]);
    }
}
