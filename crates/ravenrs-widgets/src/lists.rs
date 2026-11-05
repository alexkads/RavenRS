//! Long lists and the navigation built on them: [`VirtualList`] underneath, and
//! [`Filmstrip`], [`PagedDocumentView`] and [`FileBrowser`] on top of it. [`SortableList`] and
//! [`DocumentTabs`] share the arithmetic of a reorder drag.

mod document_tabs;
mod file_browser;
mod filmstrip;
mod paged;
pub(crate) mod reorder;
mod selection;
mod sortable;
mod virtual_list;
mod year_view;

pub use document_tabs::{DocumentTab, DocumentTabs};
pub use file_browser::{FileBrowser, FileEntry, FileSort, FileSortColumn};
pub use filmstrip::{Filmstrip, FilmstripCell};
pub use paged::PagedDocumentView;
pub use sortable::SortableList;
pub use virtual_list::VirtualList;
pub use year_view::{DayMark, YearView};
