//! The stories of `SplitButton`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `SplitButton`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_disabled = args.bool("disabled");
        let arg_size = match args.choice("size") {
            "Small" => ButtonSize::Small,
            "Large" => ButtonSize::Large,
            "Mini" => ButtonSize::Mini,
            _ => ButtonSize::Medium,
        };
        let arg_style = match args.choice("style") {
            "Secondary" => ButtonStyle::Secondary,
            "Ghost" => ButtonStyle::Ghost,
            "Danger" => ButtonStyle::Danger,
            _ => ButtonStyle::Primary,
        };

        {
            let mut it = SplitButton::new("Save")
                .icon(IconName::Check)
                .on_click(|| {})
                .item(MenuItem::new("Save and close").on_select(|| {}))
                .item(MenuItem::new("Save and new").on_select(|| {}));
            it = it.disabled(arg_disabled);
            it = it.size(arg_size.clone());
            it = it.style(arg_style.clone());
            it
        }
    })
}

impl ComponentBook for SplitButton {
    fn book() -> Meta {
        Meta::new("Actions and inputs/SplitButton", render)
            .summary("A main command with a menu of alternatives.")
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::choice("size", &["Small", "Medium", "Large", "Mini"], "Medium"))
            .arg(ArgType::choice("style", &["Primary", "Secondary", "Ghost", "Danger"], "Primary"))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Size: Small", Args::new().with("size", ArgValue::choice("Small")))
            .story("Size: Large", Args::new().with("size", ArgValue::choice("Large")))
            .story("Size: Mini", Args::new().with("size", ArgValue::choice("Mini")))
            .story("Style: Secondary", Args::new().with("style", ArgValue::choice("Secondary")))
            .story("Style: Ghost", Args::new().with("style", ArgValue::choice("Ghost")))
            .story("Style: Danger", Args::new().with("style", ArgValue::choice("Danger")))
    }
}
