//! The stories of `TreeView`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

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

/// The gallery's example of `TreeView`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_filter = args.text("filter");
        let arg_label = args.text("label");
        let arg_row_height = args.number("row_height") as f32;

        let selected = State::new(Some(String::from("invoices")));
        let opened = State::new(std::collections::BTreeSet::from([String::from("sales")]));
        {
            let mut it = TreeView::new(vec![
                TreeNode::new("sales", "Sales")
                    .icon(IconName::Folder)
                    .child(TreeNode::new("invoices", "Invoices").icon(IconName::FileText))
                    .child(TreeNode::new("quotes", "Quotes").icon(IconName::FileText)),
                TreeNode::new("stock", "Stock")
                    .icon(IconName::Folder)
                    .child(TreeNode::new("items", "Items").icon(IconName::FileText)),
                TreeNode::new("reports", "Reports").icon(IconName::FileText),
            ])
            .selected(selected.binding())
            .expanded(opened.binding());
            if !arg_filter.is_empty() {
                it = it.filter(arg_filter.clone());
            }
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it = it.row_height(arg_row_height);
            it
        }
    })
}

impl ComponentBook for TreeView {
    fn book() -> Meta {
        Meta::new("Data/TreeView", render)
            .summary("Items inside items: opens, selects, checks and filters.")
            .arg(ArgType::text("filter", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::number("row_height", 32.0, 0.0, 128.0, 1.0))
            .story("Example", Args::new())
    }
}
