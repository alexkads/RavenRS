//! The stories of `ThemeSwitcher`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `ThemeSwitcher`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        {
            let mut it = ThemeSwitcher::new();
            it
        }
    })
}

impl ComponentBook for ThemeSwitcher {
    fn book() -> Meta {
        Meta::new("Actions and inputs/ThemeSwitcher", render)
            .summary("Switches between Raven Dark, Raven Light and Raven Glass.")
            .story("Example", Args::new())
    }
}
