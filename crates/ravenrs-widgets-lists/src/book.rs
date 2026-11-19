//! The stories of the components of this crate, for RavenComponentBook: see
//! `ravenrs_widgets_base::book`.

#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ComponentBook, Meta};

/// The stories of the components of this crate that give them, in the order the book lists
/// them.
pub fn stories() -> Vec<Meta> {
    generated()
}

// rcb-scaffold: begin
mod data_inspector;
mod document_tabs;
mod file_browser;
mod filmstrip;
mod paged_document_view;
mod sortable_list;
mod tree_view;
mod virtual_list;
mod year_view;
#[allow(unused_imports)]
use ravenrs_widgets_base::AnyView;

/// The stories written by scripts/rcb-scaffold.py, in name order.
pub(crate) fn generated() -> Vec<Meta> {
    let mut all = Vec::new();
    all.push(<crate::DataInspector as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::DocumentTabs as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::FileBrowser as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::Filmstrip as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::PagedDocumentView as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::SortableList as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::TreeView as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::VirtualList as ravenrs_widgets_base::book::ComponentBook>::book());
    all.push(<crate::YearView as ravenrs_widgets_base::book::ComponentBook>::book());
    all
}
// rcb-scaffold: end
