//! The stories of `LogView`, written by hand for #824: a build log with colors, a section that
//! folds and the search.

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

#[allow(unused_variables, clippy::redundant_clone)]
fn render(args: &Args) -> AnyView {
    let view = LogView::new(sample())
        .follow_end(args.bool("follow_end"))
        .line_numbers(args.bool("line_numbers"))
        .searchable(args.bool("searchable"))
        .height(args.number("height") as f32)
        .label(args.text("label"))
        .on_line(|number| println!("line {number}"));
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
            .story("Example", Args::new())
            .story("No numbers", Args::new().with("line_numbers", false))
            .story("No search", Args::new().with("searchable", false))
            .story("From the top", Args::new().with("follow_end", false))
    }
}
