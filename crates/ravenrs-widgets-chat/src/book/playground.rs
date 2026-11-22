//! The stories of `Playground` (feature `code`). Written by hand: the gallery's example runs its
//! code with a closure, which is the runner here too.

use ravenrs_style::TextStyle;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, Text};

use crate::{CodeDiagnostic, CodeLanguage, Playground};

fn render(args: &Args) -> AnyView {
    let run = |code: &str| -> Result<AnyView, Vec<CodeDiagnostic>> {
        match code.trim().strip_prefix("say ") {
            Some(words) => Ok(Box::new(Text::new(words.to_owned()).style(TextStyle::Title))),
            None => Err(vec![CodeDiagnostic::error(1, 1, 1, "Start with `say`").hint("Write `say hello`")]),
        }
    };
    let mut playground = Playground::new(run)
        .language(CodeLanguage::Plain)
        .example("Hello", "say hello")
        .example("Goodbye", "say goodbye")
        .output_open(args.bool("output_open"))
        .height(args.number("height") as f32);
    if !args.text("title").is_empty() {
        playground = playground.title(args.text("title"));
    }
    Box::new(playground)
}

impl ComponentBook for Playground {
    fn book() -> Meta {
        Meta::new("Forms/Playground", render)
            .summary("Code beside what it makes, run as it is typed, with examples to start from.")
            .arg(ArgType::text("title", "").description("Empty: no title."))
            .arg(ArgType::bool("output_open", true).description("The panel of what the code printed."))
            .arg(ArgType::number("height", 380.0, 240.0, 720.0, 10.0))
            .story("Examples", Args::new())
            .story("With title", Args::new().with("title", "Try it"))
            .story("Output closed", Args::new().with("output_open", false))
    }
}
