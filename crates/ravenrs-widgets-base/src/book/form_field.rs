//! The stories of `FormField`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #756: it is no longer written over.

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

/// The gallery's example of `FormField`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_required = args.bool("required");
        let arg_hint = args.text("hint");
        let arg_show_errors = args.bool("show_errors");

        let email = State::new(args.text("value"));
        {
            let mut it =
                FormField::new("Work email", TextField::new(email.binding()).placeholder("you@company.com"))
                    .validate(email.binding(), validate::email);
            if arg_required {
                it = it.required();
            }
            it = it.hint(arg_hint.clone());
            it = it.show_errors(arg_show_errors);
            if !args.text("server_error").is_empty() {
                it = it.error_text(Some(args.text("server_error")));
            }
            it
        }
    })
}

impl ComponentBook for FormField<AnyView> {
    fn book() -> Meta {
        Meta::new("Forms/FormField", render)
            .summary("Labeled field with a hint and validation messages.")
            .arg(
                ArgType::text("value", "")
                    .description("What is typed; an address with no domain shows the error."),
            )
            .arg(
                ArgType::text("server_error", "")
                    .description("An error found elsewhere, as by rules that read a database."),
            )
            .arg(ArgType::bool("required", true))
            .arg(ArgType::text("hint", "We send the invoice here."))
            .arg(ArgType::bool("show_errors", false))
            .story("Example", Args::new())
            .story("Filled", Args::new().with("value", "ana@company.com"))
            .story("Error", Args::new().with("value", "ana@").with("show_errors", true))
            .story(
                "Server error",
                Args::new()
                    .with("value", "ana@company.com")
                    .with("server_error", "This email already has an account."),
            )
    }
}
