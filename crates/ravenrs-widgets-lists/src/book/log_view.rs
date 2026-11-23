//! The stories of `LogView`, written by hand for #824: a build log with colors, a section that
//! folds and the search; with #919, sections that say how they went, the words that tone a line
//! (`PASS`, `SLOW`, `FAIL`), one section alone and "Download log".

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use ravenrs_widgets_base::*;

/// A short build: a checkout, a lint and tests, with the colors a test runner writes.
fn sample() -> LogBuffer {
    let log = LogBuffer::new();
    log.section("Checkout");
    log.push("$ git fetch origin +refs/pull/803/head");
    log.push("HEAD is now at a1b2c3d");
    log.section("Testes");
    log.push("$ cargo nextest run");
    log.push("    \u{1b}[32mPASS\u{1b}[0m [0.012s] plan::rejects_cycles");
    log.push("    \u{1b}[33mSLOW\u{1b}[0m [12.4s] registry::reloads_without_restart");
    log.push("    \u{1b}[1;31mFAIL\u{1b}[0m [0.210s] docker::cleans_up_on_cancel");
    log.push("    \u{1b}[32mPASS\u{1b}[0m [0.003s] shell::masks_secrets");
    log
}

/// The same build as a CI server keeps it: each stage says how it ran and how long it took, and
/// the test runner wrote no colors, so its first words give the tones.
fn stages() -> LogBuffer {
    let log = LogBuffer::new();
    let checkout = log.section("Checkout");
    log.section_detail(checkout, "git · 8 s");
    log.push("$ git fetch origin +refs/pull/803/merge");
    log.push("HEAD is now at a1b2c3d");
    let lint = log.section("Lint");
    log.section_detail(lint, "cargo clippy · 1 min 40 s");
    log.push("$ cargo clippy --workspace -- -D warnings");
    let tests = log.section("Testes");
    log.section_detail(tests, "container rust:1 · cargo nextest run");
    log.push("$ docker run --rm -v $WORKSPACE:/w -u 1000 rust:1 cargo nextest run");
    log.push("PASS [0.012s] ravenrs-flows plan::rejects_cycles");
    log.push("PASS [0.040s] ravenrs-flows-runtime claim::lease_is_exclusive");
    log.push("SLOW [12.4s] raven-ci registry::reloads_without_restart");
    log.push("PASS [0.003s] raven-ci-plugin-shell masks_secrets GITHUB_TOKEN=****");
    log.push("RUN raven-ci-plugin-kubernetes pending_pod_times_out");
    let image = log.section("Imagem e push");
    log.section_detail(image, "esperando");
    log
}

#[allow(unused_variables, clippy::redundant_clone)]
fn render(args: &Args) -> AnyView {
    let buffer = if args.bool("stages") { stages() } else { sample() };
    let mut view = LogView::new(buffer)
        .follow_end(args.bool("follow_end"))
        .line_numbers(args.bool("line_numbers"))
        .searchable(args.bool("searchable"))
        .height(args.number("height") as f32)
        .label(args.text("label"))
        .on_line(|number| println!("line {number}"));
    if args.bool("keywords") {
        view = view
            .keyword("PASS", Tone::Success)
            .keyword("SLOW", Tone::Warning)
            .keyword("FAIL", Tone::Danger)
            .keyword("RUN", Tone::Accent);
    }
    let only = args.text("only_section");
    if !only.is_empty() {
        view = view.only_section(ravenrs_reactive::Binding::constant(Some(only.to_owned())));
    }
    if args.bool("download") {
        view = view.download("build-42.log");
    }
    Box::new(view)
}

impl ComponentBook for LogView {
    fn book() -> Meta {
        Meta::new("Data/LogView", render)
            .summary("A build log as it comes: ANSI colors, follows the end, search, sections that fold and lines pointed at by number")
            .arg(ArgType::bool("follow_end", true))
            .arg(ArgType::bool("line_numbers", true))
            .arg(ArgType::bool("searchable", true))
            .arg(ArgType::number("height", 220.0, 80.0, 1040.0, 1.0))
            .arg(ArgType::text("label", "Build log"))
            .arg(ArgType::bool("stages", false))
            .arg(ArgType::bool("keywords", false))
            .arg(ArgType::text("only_section", ""))
            .arg(ArgType::bool("download", false))
            .story("Example", Args::new())
            .story("No numbers", Args::new().with("line_numbers", false))
            .story("No search", Args::new().with("searchable", false))
            .story("From the top", Args::new().with("follow_end", false))
            .story(
                "Build stages",
                Args::new().with("stages", true).with("keywords", true).with("download", true).with("height", 260.0),
            )
            .story(
                "One section alone",
                Args::new().with("stages", true).with("keywords", true).with("only_section", "Testes"),
            )
    }
}
