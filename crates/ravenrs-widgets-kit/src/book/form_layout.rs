//! The stories of `FormLayout`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over. The `FormGroup` builders have
//! stories of their own here, since a group is only drawn inside a form (#768).

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

/// The gallery's example of `FormLayout`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_columns = args.number("columns") as usize;

        let (name, city, notes) =
            (State::new(String::new()), State::new(String::new()), State::new(String::new()));
        {
            let mut it = FormLayout::new().group(
                FormGroup::titled("Contact")
                    .field(FormField::new("Name", TextField::new(name.binding())).required())
                    .field(FormField::new("City", TextField::new(city.binding())))
                    .wide(FormField::new("Notes", TextArea::new(notes.binding()).rows(2))),
            );
            it = it.columns(arg_columns);
            it
        }
    })
}

/// Two groups: one under a title, one without; each `FormGroup` fills the form's columns.
fn groups(args: &Args) -> AnyView {
    let text = || State::new(String::new());
    let (name, email, street, city) = (text(), text(), text(), text());
    Box::new(
        FormLayout::new()
            .columns(args.number("columns") as usize)
            .group(
                FormGroup::titled("Contact")
                    .field(FormField::new("Name", TextField::new(name.binding())).required())
                    .field(FormField::new("Email", TextField::new(email.binding()))),
            )
            .group(
                FormGroup::new()
                    .field(FormField::new("Street", TextField::new(street.binding())))
                    .field(FormField::new("City", TextField::new(city.binding()))),
            ),
    )
}

/// A `FormGroup` with `columns(1)` in a form of two: the group keeps one field a row.
fn group_columns(_: &Args) -> AnyView {
    let text = || State::new(String::new());
    let (code, amount, reason, notes) = (text(), text(), text(), text());
    Box::new(
        FormLayout::new()
            .columns(2)
            .group(
                FormGroup::titled("Order")
                    .field(FormField::new("Code", TextField::new(code.binding())))
                    .field(FormField::new("Amount", TextField::new(amount.binding()))),
            )
            .group(
                FormGroup::titled("Return")
                    .columns(1)
                    .field(FormField::new("Reason", TextField::new(reason.binding())))
                    .field(FormField::new("Notes", TextArea::new(notes.binding()).rows(2))),
            ),
    )
}

/// What keeps the form from being saved, listed at the top.
fn errors(args: &Args) -> AnyView {
    let (name, city) = (State::new(String::new()), State::new(String::new()));
    Box::new(
        FormLayout::new()
            .columns(args.number("columns") as usize)
            .errors(["Name is required.", "City must have at least 2 letters."])
            .group(
                FormGroup::titled("Contact")
                    .field(FormField::new("Name", TextField::new(name.binding())).required())
                    .field(FormField::new("City", TextField::new(city.binding()))),
            ),
    )
}

/// A tab under the groups, for a long part such as the related records.
fn tabs(args: &Args) -> AnyView {
    let (name, city) = (State::new(String::new()), State::new(String::new()));
    Box::new(
        FormLayout::new()
            .columns(args.number("columns") as usize)
            .group(
                FormGroup::titled("Contact")
                    .field(FormField::new("Name", TextField::new(name.binding())))
                    .field(FormField::new("City", TextField::new(city.binding()))),
            )
            .tab("Orders", Text::new("The customer's orders.").foreground(Semantic::InkMuted))
            .tab("Notes", Text::new("Notes about the customer.").foreground(Semantic::InkMuted)),
    )
}

impl ComponentBook for FormLayout {
    fn book() -> Meta {
        Meta::new("Actions and inputs/FormLayout", render)
            .summary("Groups of fields in columns, tabs and a list of errors.")
            .arg(ArgType::number("columns", 2.0, 0.0, 10.0, 1.0))
            .story("Example", Args::new())
            .story_with("Groups, with and without a title", Args::new(), groups)
            .story_with("Group with its own columns", Args::new(), group_columns)
            .story_with("With errors", Args::new(), errors)
            .story_with("With tabs", Args::new(), tabs)
    }
}
