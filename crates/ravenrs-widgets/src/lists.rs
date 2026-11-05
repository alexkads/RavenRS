//! Long lists and the navigation built on them: [`VirtualList`] underneath, and
//! [`Filmstrip`], [`PagedDocumentView`] and [`FileBrowser`] on top of it. [`SortableList`] and
//! [`DocumentTabs`] share the arithmetic of a reorder drag.

mod filmstrip;
mod paged;
mod selection;
mod virtual_list;

pub use filmstrip::{Filmstrip, FilmstripCell};
pub use paged::PagedDocumentView;
pub use virtual_list::VirtualList;
