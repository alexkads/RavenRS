//! The stories of `DropdownMenu`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #758: it is no longer written over.

use crate::book::button::{ICONS, icon};
#[allow(unused_imports)]
use crate::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use ravenrs_core::*;
#[allow(unused_imports)]
use ravenrs_layout::*;
#[allow(unused_imports)]
use ravenrs_reactive::*;
#[allow(unused_imports)]
use ravenrs_style::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `DropdownMenu`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_separator = args.bool("separator");
        let open = State::new(args.bool("open"));
        let arg_icon_only = args.bool("icon_only");
        let arg_size = match args.choice("size") {
            "Small" => ButtonSize::Small,
            "Large" => ButtonSize::Large,
            "Mini" => ButtonSize::Mini,
            _ => ButtonSize::Medium,
        };
        let arg_style = match args.choice("style") {
            "Primary" => ButtonStyle::Primary,
            "Ghost" => ButtonStyle::Ghost,
            "Danger" => ButtonStyle::Danger,
            _ => ButtonStyle::Secondary,
        };

        {
            let mut it = DropdownMenu::new(args.text("label"))
                .item(MenuItem::new("Edit").icon(IconName::Pencil).shortcut("⌘E"))
                .item(MenuItem::new("Share").icon(IconName::Share2))
                .item(MenuItem::new("Export").item(MenuItem::new("As PDF")).item(MenuItem::new("As CSV")))
                .item(MenuItem::new("Show grid").checked(true))
                .item(MenuItem::new("Label").element(Badge::new("Family").tone(Tone::Accent)))
                .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive());
            if let Some(name) = icon(args.choice("icon")) {
                it = it.icon(name);
            }
            if arg_separator {
                it = it.separator();
            }
            if arg_icon_only {
                it = it.icon_only();
            }
            it = it.size(arg_size.clone());
            it = it.style(arg_style.clone());
            it = it.open(open.binding());
            it
        }
    })
}

impl ComponentBook for DropdownMenu {
    fn book() -> Meta {
        Meta::new("Overlays/DropdownMenu", render)
            .summary("Button that opens a list of actions, with submenus and check marks.")
            .arg(ArgType::text("label", "Actions").description("The words on the button; start with a verb."))
            .arg(ArgType::choice("icon", ICONS, "none").description("An icon before the label."))
            .arg(ArgType::bool("separator", true))
            .arg(ArgType::bool("icon_only", false))
            .arg(ArgType::choice("size", &["Small", "Medium", "Large", "Mini"], "Medium"))
            .arg(ArgType::choice("style", &["Primary", "Secondary", "Ghost", "Danger"], "Secondary"))
            .arg(ArgType::bool("open", false).description("The list of actions is open."))
            .story("Example", Args::new())
            .story("Open", Args::new().with("open", true))
            .story("Size: Small", Args::new().with("size", ArgValue::choice("Small")))
            .story("Size: Large", Args::new().with("size", ArgValue::choice("Large")))
            .story("Size: Mini", Args::new().with("size", ArgValue::choice("Mini")))
            .story("Style: Primary", Args::new().with("style", ArgValue::choice("Primary")))
            .story("Style: Ghost", Args::new().with("style", ArgValue::choice("Ghost")))
            .story("Style: Danger", Args::new().with("style", ArgValue::choice("Danger")))
            .story(
                "Primary with icon",
                Args::new()
                    .with("style", ArgValue::choice("Primary"))
                    .with("label", "New project")
                    .with("icon", ArgValue::choice("plus")),
            )
    }
}
