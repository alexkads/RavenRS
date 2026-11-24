//! The stories of `InputOtp`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `InputOtp`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let code = State::new(args.text("code"));
        {
            let mut it = InputOtp::new(code.binding(), 6).disabled(args.bool("disabled"));
            it
        }
    })
}

impl ComponentBook for InputOtp {
    fn book() -> Meta {
        Meta::new("Forms/InputOtp", render)
            .summary("Separate boxes for a one-time code.")
            .arg(ArgType::text("code", "").description("What was typed so far."))
            .arg(
                ArgType::bool("disabled", false)
                    .description("Locked while the code is verified: no typing, no completing again."),
            )
            .story("Example", Args::new())
            .story("Verifying", Args::new().with("code", "482913").with("disabled", true))
    }
}
