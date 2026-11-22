//! The RavenRS `DataGrid`: `grid::query` (the pure pipeline of sorting, filtering, grouping and
//! summaries), `grid::model` and `grid::view`.
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

pub mod grid;
#[cfg(feature = "testing")]
pub mod testing;

pub use grid::{
    CellAlign, CellEdit, ColumnLayout, DataGrid, FilterLayout, GridCellStyle, GridColumn, GridCurrent,
    GridData, GridExportFormat, GridLayout, GridPage, GridRequest, GridResponder, GridRowAction,
    GridScenario, GridTable, GridTableColumn, GridTableKind, NumberFormat, RemoteRows, SortDirection,
    SortLayout, Value,
};
