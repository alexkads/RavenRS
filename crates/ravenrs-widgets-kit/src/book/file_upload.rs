//! The stories of `FileUpload`, from the gallery's example. Reviewed: read-only with no files
//! shows nothing, so it has no story of its own.

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

/// The gallery's example of `FileUpload`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_most_files = args.number("most_files") as usize;
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_read_only = args.bool("read_only");

        let files = State::new(Vec::<PickedFile>::new());
        {
            let mut it = FileUpload::new(files.binding()).accept(["pdf", "png", "jpg"]);
            it = it.most_files(arg_most_files);
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            it = it.read_only(arg_read_only);
            it
        }
    })
}

impl ComponentBook for FileUpload {
    fn book() -> Meta {
        Meta::new("Actions and inputs/FileUpload", render)
            .summary("Files from the chooser or dropped, with a reason when refused.")
            .arg(ArgType::number("most_files", 5.0, 0.0, 20.0, 1.0))
            .arg(ArgType::text("label", "Receipts"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::bool("read_only", false))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
    }
}
