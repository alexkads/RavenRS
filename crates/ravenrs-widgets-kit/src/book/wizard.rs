//! The stories of `Wizard`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Wizard`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_finish_label = args.text("finish_label");

        let step = State::new(args.number("step") as _);
        let name = State::new(String::new());
        let filled = name.clone();
        {
            let mut it = Wizard::new(
                step.binding(),
                vec![
                    WizardStep::new("Account", FormField::new("Name", TextField::new(name.binding()))).check(
                        move || {
                            if filled.get().trim().is_empty() {
                                Err("Fill in the name".to_owned())
                            } else {
                                Ok(())
                            }
                        },
                    ),
                    WizardStep::new("Opening balance", Text::new("How much it holds today")),
                    WizardStep::new("Review", Text::new("Everything is ready")),
                ],
            )
            .on_finish(|| {});
            if !arg_finish_label.is_empty() {
                it = it.finish_label(arg_finish_label.clone());
            }
            it
        }
    })
}

impl ComponentBook for Wizard {
    fn book() -> Meta {
        Meta::new("Actions and inputs/Wizard", render)
            .summary("A task in steps that can refuse to be left.")
            .arg(ArgType::text("finish_label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::number("step", 0.0, 0.0, 5.0, 1.0).description("How the example starts."))
            .story("Example", Args::new())
    }
}
