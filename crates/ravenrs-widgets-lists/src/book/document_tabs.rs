//! The stories of `DocumentTabs`, from the component gallery's example, by scripts/rcb-scaffold.py.
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
use ravenrs_widgets_kit::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `DocumentTabs`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");

        let tabs = State::new(vec![
            DocumentTab::new(1, "Wedding").icon(IconName::Camera),
            // ravenrs-lint: allow(raw_color) the tab keeps the color its owner picked for that document
            DocumentTab::new(2, "Portraits").unsaved(true).color(Color::rgb(0.25, 0.55, 0.95)),
            DocumentTab::new(3, "Studio with a long name that does not fit"),
            DocumentTab::new(4, "Home").closable(false),
        ]);
        let active = State::new(1_u64);
        view_fn(move || {
            let (closed, moved, renamed) = (tabs.clone(), tabs.clone(), tabs.clone());
            {
                let mut it = DocumentTabs::new(tabs.get(), active.binding())
                    .on_close(move |id| closed.update(|list| list.retain(|tab| tab.id != id)))
                    .on_reorder(move |from, to| {
                        moved.update(|list| {
                            let tab = list.remove(from);
                            list.insert(to, tab);
                        });
                    })
                    .on_rename(move |id, name| {
                        renamed.update(|list| {
                            if let Some(tab) = list.iter_mut().find(|tab| tab.id == id) {
                                tab.title = name;
                            }
                        });
                    })
                    .on_new(|| {})
                    .fade(Semantic::SurfaceElevated);
                if !arg_label.is_empty() {
                    it = it.label(arg_label.clone());
                }
                it
            }
        })
    })
}

impl ComponentBook for DocumentTabs {
    fn book() -> Meta {
        Meta::new("Navigation/DocumentTabs", render)
            .summary("Tabs for open documents that close, move, rename and say what was not saved.")
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
    }
}
