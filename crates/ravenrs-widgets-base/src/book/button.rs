//! The stories of `Button` and `IconButton`.

use ravenrs_reactive::State;

use super::{ArgType, ArgValue, Args, ComponentBook, Meta};
use crate::{AnyView, Button, ButtonSize, ButtonStyle, HStack, IconButton, IconName};

/// The styles, as the book offers them.
pub(super) const STYLES: &[&str] = &["Primary", "Secondary", "Ghost", "Danger"];
/// The sizes, as the book offers them.
pub(super) const SIZES: &[&str] = &["Mini", "Small", "Medium", "Large"];
/// A few icons to try on a component; any `IconName` works.
pub(super) const ICONS: &[&str] =
    &["none", "plus", "check", "trash-2", "pencil", "search", "settings", "download"];

pub(super) fn style(name: &str) -> ButtonStyle {
    match name {
        "Secondary" => ButtonStyle::Secondary,
        "Ghost" => ButtonStyle::Ghost,
        "Danger" => ButtonStyle::Danger,
        _ => ButtonStyle::Primary,
    }
}

pub(super) fn size(name: &str) -> ButtonSize {
    match name {
        "Mini" => ButtonSize::Mini,
        "Small" => ButtonSize::Small,
        "Large" => ButtonSize::Large,
        _ => ButtonSize::Medium,
    }
}

pub(super) fn icon(name: &str) -> Option<IconName> {
    IconName::ALL.iter().copied().find(|icon| icon.name() == name)
}

fn button(args: &Args) -> AnyView {
    let clicks = State::new(0);
    let mut button = Button::new(args.text("label"))
        .style(style(args.choice("style")))
        .size(size(args.choice("size")))
        .disabled(args.bool("disabled"))
        .loading(args.bool("loading"))
        .selected(args.bool("selected"))
        .on_click(move || clicks.update(|n| *n += 1));
    if args.bool("full_width") {
        button = button.full_width();
    }
    if let Some(icon) = icon(args.choice("icon")) {
        button = button.icon(icon);
    }
    if !args.text("shortcut").is_empty() {
        button = button.shortcut(args.text("shortcut"));
    }
    Box::new(button)
}

fn icon_button(args: &Args) -> AnyView {
    let icon = icon(args.choice("icon")).unwrap_or(IconName::Pencil);
    Box::new(
        IconButton::new(icon)
            .label(args.text("label"))
            .style(style(args.choice("style")))
            .size(size(args.choice("size")))
            .disabled(args.bool("disabled"))
            .selected(args.bool("selected")),
    )
}

fn all_styles(_: &Args) -> AnyView {
    let row: Vec<AnyView> =
        STYLES.iter().map(|name| Box::new(Button::new(*name).style(style(name))) as AnyView).collect();
    Box::new(HStack::new(row).spacing(8.0).wrap())
}

fn all_sizes(_: &Args) -> AnyView {
    let row: Vec<AnyView> = SIZES
        .iter()
        .map(|name| Box::new(Button::new(*name).style(ButtonStyle::Secondary).size(size(name))) as AnyView)
        .collect();
    Box::new(HStack::new(row).spacing(8.0).wrap())
}

impl ComponentBook for Button {
    fn book() -> Meta {
        Meta::new("Actions and inputs/Button", button)
            .summary("Clickable, focusable button with styles, sizes, icons, loading and selected states.")
            .arg(ArgType::text("label", "Save").description("The words on it; start with a verb."))
            .arg(
                ArgType::choice("style", STYLES, "Primary")
                    .description("Primary for the one action of a window."),
            )
            .arg(ArgType::choice("size", SIZES, "Medium"))
            .arg(ArgType::choice("icon", ICONS, "none").description("An icon before the label."))
            .arg(ArgType::bool("disabled", false))
            .arg(
                ArgType::bool("loading", false).description("A spinner in place of the icon while it works."),
            )
            .arg(ArgType::bool("selected", false).description("Pressed, for a button that shows a state."))
            .arg(ArgType::bool("full_width", false).description("Fills the width of its column."))
            .arg(ArgType::text("shortcut", "").description("The key shown at the end, as F8."))
            .story("Primary", Args::new())
            .story(
                "Secondary",
                Args::new().with("style", ArgValue::choice("Secondary")).with("label", "Cancel"),
            )
            .story("Ghost", Args::new().with("style", ArgValue::choice("Ghost")).with("label", "Show more"))
            .story(
                "Danger with icon",
                Args::new()
                    .with("style", ArgValue::choice("Danger"))
                    .with("label", "Delete")
                    .with("icon", ArgValue::choice("trash-2")),
            )
            .story(
                "Loading",
                Args::new()
                    .with("style", ArgValue::choice("Secondary"))
                    .with("label", "Upload")
                    .with("loading", true),
            )
            .story("Disabled", Args::new().with("disabled", true))
            .story(
                "With shortcut",
                Args::new()
                    .with("style", ArgValue::choice("Ghost"))
                    .with("label", "Open")
                    .with("shortcut", "F8"),
            )
            .story_with("All styles", Args::new(), all_styles)
            .story_with("All sizes", Args::new(), all_sizes)
    }
}

impl ComponentBook for IconButton {
    fn book() -> Meta {
        Meta::new("Actions and inputs/IconButton", icon_button)
            .summary("Square, icon-only button for toolbars; its label is read aloud and shown as a tooltip.")
            .arg(ArgType::choice("icon", &ICONS[1..], "pencil"))
            .arg(ArgType::text("label", "Edit").description("Read by screen readers and shown on hover."))
            .arg(ArgType::choice("style", STYLES, "Ghost"))
            .arg(ArgType::choice("size", SIZES, "Medium"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("selected", false))
            .story("Ghost", Args::new())
            .story(
                "Primary",
                Args::new()
                    .with("style", ArgValue::choice("Primary"))
                    .with("icon", ArgValue::choice("plus"))
                    .with("label", "New"),
            )
            .story("Selected", Args::new().with("selected", true))
    }
}
