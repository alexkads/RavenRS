//! The stories of `ContextMenu`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #758: it is no longer written over. It opens at the pointer, on a right click; the book shows the area it belongs to.

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

/// The gallery's example of `ContextMenu`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_separator = args.bool("separator");

        {
            let mut it = ContextMenu::new(
                Text::new("Right-click in this box")
                    .foreground(Semantic::InkMuted)
                    .centered()
                    .height(90.0)
                    .background(Semantic::Control)
                    .corner_radius(12.0),
            )
            .item(MenuItem::new("Copy").icon(IconName::Copy).shortcut("⌘C"))
            .item(MenuItem::new("Sort by").item(MenuItem::new("Name")).item(MenuItem::new("Date")))
            .item(MenuItem::new("Delete").icon(IconName::Trash2).destructive());
            if arg_separator {
                it = it.separator();
            }
            it
        }
    })
}

impl ComponentBook for ContextMenu<AnyView> {
    fn book() -> Meta {
        Meta::new("Overlays/ContextMenu", render)
            .summary("Menu opened at the pointer on right click.")
            .arg(ArgType::bool("separator", false))
            .story("Example", Args::new())
    }
}
