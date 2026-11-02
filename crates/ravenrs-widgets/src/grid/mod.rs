//! An enterprise-style data grid: [`DataGrid`] over [`GridData`] with
//! [`GridColumn`]s. The query pipeline in [`query`] is pure and usable on its own.

mod layout;
mod model;
pub mod query;
mod remote;
mod view;

pub use layout::{ColumnLayout, FilterLayout, GridLayout, GridScenario, SortLayout, SummaryLayout};
pub use remote::{GridPage, GridRequest, GridResponder, RemoteRows};

pub use model::{
    CellAlign, CellFormat, CellTemplate, GridColumn, GridData, NumberFormat, Row, SortDirection, Summary,
    Value,
};
pub use query::{
    GridExportFormat, GridQuery, GridTable, GridTableColumn, GridTableKind, QueryResult, ViewRow, to_table,
    to_tsv,
};
pub use view::{CellEdit, DataGrid, GridCellStyle, GridRowAction, SelectionMode};
