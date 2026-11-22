//! The stories of `SidebarBrand`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #757: it is no longer written over.

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
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `SidebarBrand`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_subtitle = args.text("subtitle");

        {
            let mut it = SidebarBrand::new(Icon::new(IconName::Wallet), "Raven");
            it = it.subtitle(arg_subtitle.clone());
            it
        }
    })
}

impl ComponentBook for SidebarBrand {
    fn book() -> Meta {
        Meta::new("Navigation/SidebarBrand", render)
            .summary("The name of an app at the top of a sidebar.")
            .arg(ArgType::text("subtitle", "Family finances"))
            .story("Example", Args::new())
    }
}
