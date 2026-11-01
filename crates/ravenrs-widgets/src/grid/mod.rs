//! An enterprise-style data grid: [`DataGrid`] over [`GridData`] with
//! [`GridColumn`]s. The query pipeline in [`query`] is pure and usable on its own.

mod model;
pub mod query;
mod view;

pub use model::{
    CellAlign, CellFormat, CellTemplate, GridColumn, GridData, NumberFormat, Row, SortDirection, Summary,
    Value,
};
pub use query::{GridQuery, QueryResult, ViewRow, to_tsv};
pub use view::{DataGrid, SelectionMode};
