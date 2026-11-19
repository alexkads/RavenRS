//! The stories of `CodeEditor` (feature `code`). Written by hand: its example in the gallery
//! lives behind the same feature.

use ravenrs_reactive::State;
use ravenrs_widgets_base::AnyView;
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};

use crate::{CodeEditor, CodeLanguage};

const SOURCE: &str = "fn greet(name: &str) -> String {\n    // A comment, a string and a number.\n    format!(\"Hello, {name}! {}\", 42)\n}\n";

fn render(args: &Args) -> AnyView {
    let source = State::new(SOURCE.to_owned());
    let language = if args.choice("language") == "Plain" { CodeLanguage::Plain } else { CodeLanguage::Rust };
    Box::new(
        CodeEditor::new(source.binding())
            .language(language)
            .line_numbers(args.bool("line_numbers"))
            .rows(args.number("rows") as usize)
            .read_only(args.bool("read_only")),
    )
}

impl ComponentBook for CodeEditor {
    fn book() -> Meta {
        Meta::new("Forms/CodeEditor", render)
            .summary(
                "Edits code with colors, line numbers, completions and the problems marked where they are.",
            )
            .arg(ArgType::choice("language", &["Rust", "Plain"], "Rust"))
            .arg(ArgType::bool("line_numbers", true))
            .arg(ArgType::number("rows", 12.0, 3.0, 40.0, 1.0))
            .arg(ArgType::bool("read_only", false))
            .story("Rust", Args::new())
            .story("Plain", Args::new().with("language", ArgValue::choice("Plain")))
            .story("Read only, no numbers", Args::new().with("read_only", true).with("line_numbers", false))
    }
}
