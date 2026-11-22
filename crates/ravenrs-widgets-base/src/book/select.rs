//! The stories of `Select`.

use ravenrs_reactive::State;

use super::button::{SIZES, size};
use super::{ArgType, ArgValue, Args, ComponentBook, Meta};
use crate::{AnyView, Select};

fn select(args: &Args) -> AnyView {
    let options: Vec<String> = args.text("options").split(',').map(|s| s.trim().to_owned()).collect();
    let chosen = State::new((args.number("selected") as usize).min(options.len().saturating_sub(1)));
    let mut select = Select::new(chosen.binding(), options)
        .label(args.text("label"))
        .size(size(args.choice("size")))
        .disabled(args.bool("disabled"))
        .read_only(args.bool("read_only"));
    if args.bool("fill") {
        select = select.fill();
    }
    if args.bool("searchable") {
        select = select.searchable();
    }
    Box::new(select)
}

impl ComponentBook for Select {
    fn book() -> Meta {
        Meta::new("Forms/Select", select)
            .summary("Picks one option from a list that opens under the field, with search when it is long.")
            .arg(ArgType::text("label", "Date range"))
            .arg(
                ArgType::text("options", "Last 7 days, Last 30 days, This year")
                    .description("The options, separated by commas."),
            )
            .arg(ArgType::number("selected", 0.0, 0.0, 10.0, 1.0))
            .arg(ArgType::choice("size", SIZES, "Medium"))
            .arg(ArgType::bool("searchable", false).description("A search box at the top of the list."))
            .arg(
                ArgType::bool("fill", false)
                    .description("Fills the width of its column, as the fields around it."),
            )
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .story("Default", Args::new())
            .story(
                "Small and searchable",
                Args::new()
                    .with("label", "Currency")
                    .with("options", "Real, Euro, Dollar")
                    .with("size", ArgValue::choice("Small"))
                    .with("searchable", true),
            )
            .story("Disabled", Args::new().with("disabled", true))
            .story("Read only", Args::new().with("read_only", true))
            .story("Mini", Args::new().with("size", ArgValue::choice("Mini")))
            .story("Large", Args::new().with("size", ArgValue::choice("Large")))
            .story("Fill", Args::new().with("fill", true))
    }
}
