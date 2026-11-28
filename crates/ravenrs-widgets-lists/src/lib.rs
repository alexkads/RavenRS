//! RavenRS long lists and what is built on them: `VirtualList`, `Filmstrip`, `PagedDocumentView`,
//! `FileBrowser`, `SortableList`, `DocumentTabs`, `TreeView`, and `DataInspector`.
//!
//! Applications depend on `ravenrs-widgets` (or `ravenrs`), which re-exports all of this by the
//! same names.

// The base and the kit, by the names the components use: `crate::Button`, `crate::Tabs`,
// `crate::context::BuildCx`.
/// The stories of the components of this crate (feature `book`).
#[cfg(feature = "book")]
pub mod book;

#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;

#[doc(hidden)]
pub mod inspector;
#[doc(hidden)]
pub mod lists;
#[cfg(feature = "testing")]
pub mod testing;
#[doc(hidden)]
pub mod tree;

pub use inspector::{DataInspector, PathStep, SAMPLE, field_path};
pub use lists::{
    Chronology, ChronologyItem, DocumentTab, DocumentTabs, FileBrowser, FileEntry, FileSort, FileSortColumn,
    Filmstrip, FilmstripCell, LogBuffer, LogView, PagedDocumentView, SortableList, VirtualList,
};
pub use tree::{TreeNode, TreeView};
