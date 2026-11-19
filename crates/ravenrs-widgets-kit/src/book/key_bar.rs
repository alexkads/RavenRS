//! The stories of `KeyBar`, the counter's bar of function keys. Written by hand: the gallery's
//! example keeps its state in a way the scaffold does not read.

use ravenrs_core::Key;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, Hotkey};

use crate::{KeyBar, KeyBarItem};

fn render(args: &Args) -> AnyView {
    let mut pay = KeyBarItem::new(Hotkey::key(Key::Function(9)), args.text("primary_label"), || {});
    if args.bool("primary") {
        pay = pay.primary();
    }
    Box::new(
        KeyBar::new()
            .item(KeyBarItem::new(Hotkey::key(Key::Function(1)), "Help", || {}))
            .item(
                KeyBarItem::new(Hotkey::key(Key::Function(6)), "Remove item", || {})
                    .disabled(args.bool("disabled")),
            )
            .item(pay),
    )
}

impl ComponentBook for KeyBar {
    fn book() -> Meta {
        Meta::new("Display and feedback/KeyBar", render)
            .summary("The function keys of a counter, each with what it does, the main one stressed.")
            .arg(
                ArgType::text("primary_label", "Pay")
                    .description("The words of the key that closes the sale."),
            )
            .arg(
                ArgType::bool("primary", true)
                    .description("Stresses the key, as the main action of the screen."),
            )
            .arg(
                ArgType::bool("disabled", true)
                    .description("Greys out Remove item, as when the sale is empty."),
            )
            .story("Example", Args::new())
            .story("All keys available", Args::new().with("disabled", false))
            .story("No main key", Args::new().with("primary", false))
    }
}
