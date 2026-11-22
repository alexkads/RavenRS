//! The stories of `TextField`.

use ravenrs_reactive::State;
use ravenrs_style::Semantic;

use super::button::{ICONS, SIZES, icon, size};
use super::{ArgType, ArgValue, Args, ComponentBook, Meta};
use crate::{AnyView, Badge, Text, TextField};

fn text_field(args: &Args) -> AnyView {
    let value = State::new(args.text("value"));
    let mut field = TextField::new(value.binding())
        .placeholder(args.text("placeholder"))
        .size(size(args.choice("size")))
        .disabled(args.bool("disabled"))
        .read_only(args.bool("read_only"));
    if args.bool("secure") {
        field = field.secure();
    }
    if args.bool("flat") {
        field = field.flat();
    }
    if args.bool("clearable") {
        field = field.clearable();
    }
    if let Some(icon) = icon(args.choice("icon")) {
        field = field.icon(icon);
    }
    if !args.text("prefix").is_empty() {
        field = field.prefix(Text::new(args.text("prefix")).foreground(Semantic::InkMuted));
    }
    if !args.text("suffix").is_empty() {
        field = field.suffix(Badge::new(args.text("suffix")));
    }
    Box::new(field)
}

impl ComponentBook for TextField {
    fn book() -> Meta {
        Meta::new("Forms/TextField", text_field)
            .summary("Single-line text input with placeholder, icon, prefix, suffix and a clear button.")
            .arg(ArgType::text("value", ""))
            .arg(ArgType::text("placeholder", "Full name"))
            .arg(ArgType::choice("size", SIZES, "Medium"))
            .arg(ArgType::choice("icon", ICONS, "none").description("An icon at the start."))
            .arg(ArgType::text("prefix", "").description("Words before the text, as a currency."))
            .arg(ArgType::text("suffix", "").description("A badge after the text, as a unit."))
            .arg(ArgType::bool("clearable", false).description("A button that empties it."))
            .arg(ArgType::bool("secure", false).description("Hides what is typed, for passwords."))
            .arg(ArgType::bool("flat", false).description("No fill or border, inside a grid or a toolbar."))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .story("Empty", Args::new())
            .story("Filled", Args::new().with("value", "Ana Lima"))
            .story(
                "With icon",
                Args::new()
                    .with("icon", ArgValue::choice("search"))
                    .with("placeholder", "Search")
                    .with("clearable", true),
            )
            .story(
                "Price",
                Args::new()
                    .with("value", "49,90")
                    .with("size", ArgValue::choice("Small"))
                    .with("prefix", "$")
                    .with("suffix", "per month"),
            )
            .story(
                "Password",
                Args::new().with("value", "secret").with("secure", true).with("placeholder", "Password"),
            )
            .story("Disabled", Args::new().with("value", "Ana Lima").with("disabled", true))
            .story("Read only", Args::new().with("value", "Ana Lima").with("read_only", true))
            .story("Mini", Args::new().with("size", ArgValue::choice("Mini")))
            .story("Large", Args::new().with("size", ArgValue::choice("Large")))
    }
}
