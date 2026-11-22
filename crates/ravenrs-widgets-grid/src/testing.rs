//! [`DataGridDriver`]: what a test does to a `DataGrid` as a person does it, by the row's key
//! and the column, never by pixels or by reading the grid's state.
//!
//! It only uses `ravenrs-e2e`'s public locators and actions on the ids the grid gives a test:
//! `column:<column>` on each header, `filter:<column>` on each field of the filter row and,
//! with [`DataGrid::key_column`](crate::DataGrid::key_column), `row:<key>` on each row and
//! `cell:<key>:<column>` on each cell. A column is named by its field, or by its title when it
//! has none.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_widgets_grid::SortDirection;
//! use ravenrs_widgets_grid::testing::DataGridDriver;
//! # fn run(e2e: &mut E2e) -> ravenrs_e2e::E2eResult {
//! let mut grid = DataGridDriver::new(e2e);
//! grid.edit_cell("42", "name", "Ana")?;
//! grid.sort_by("amount", SortDirection::Descending)?;
//! grid.filter("name", "an")?;
//! # Ok(()) }
//! ```

use ravenrs_e2e::{E2e, E2eResult, ErrorCode, Locator, Query};

use crate::SortDirection;

/// Edits, sorts and filters a `DataGrid` in an [`E2e`] session.
pub struct DataGridDriver<'a> {
    e2e: &'a mut E2e,
    within: Option<Query>,
}

impl std::fmt::Debug for DataGridDriver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataGridDriver").field("within", &self.within).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<'a> DataGridDriver<'a> {
    /// Drives the grid on the screen; with more than one, say which with [`Self::within`].
    pub fn new(e2e: &'a mut E2e) -> Self {
        Self { e2e, within: None }
    }

    /// Only the grid inside the part `grid` finds.
    pub fn within(mut self, grid: Query) -> Self {
        self.within = Some(grid);
        self
    }

    fn locate(&mut self, id: String) -> Locator<'_> {
        let within = self.within.clone();
        let locator = self.e2e.by_test_id(id);
        match within {
            Some(parent) => locator.within(parent),
            None => locator,
        }
    }

    /// Opens the cell of the row whose key is `row_key`, in `column`, with a double click,
    /// writes `text` in place of what it has and confirms it with Enter. A cell of a column
    /// that is not editable fails with `E2E_DISABLED`.
    pub fn edit_cell(&mut self, row_key: &str, column: &str, text: &str) -> E2eResult {
        self.locate(format!("cell:{row_key}:{column}")).fill(text)
    }

    /// Sorts by `column` alone, in `direction`, clicking its header as a person does: a click
    /// goes from no order to ascending, to descending, and back to none.
    pub fn sort_by(&mut self, column: &str, direction: SortDirection) -> E2eResult {
        let clicks = match direction {
            SortDirection::Ascending => 1,
            SortDirection::Descending => 2,
        };
        self.restart_sort(column)?;
        for _ in 0..clicks {
            self.locate(format!("column:{column}")).click()?;
        }
        Ok(())
    }

    /// Takes `column` out of the order, back to how the data came.
    pub fn clear_sort(&mut self, column: &str) -> E2eResult {
        self.restart_sort(column)
    }

    /// Clicks the header until it says nothing about an order: two clicks at most.
    fn restart_sort(&mut self, column: &str) -> E2eResult {
        let id = format!("column:{column}");
        for _ in 0..3 {
            if self.locate(id.clone()).node()?.description.is_none() {
                return Ok(());
            }
            self.locate(id.clone()).click()?;
        }
        Err(self
            .e2e
            .failure(ErrorCode::Expectation, format!("#{id}: the header keeps an order after three clicks"))
            .hint("a column that cannot be sorted does not change on a click"))
    }

    /// Writes `text` in the filter row's field of `column`, in place of what it has; an empty
    /// text clears the filter. The grid must show its filter row.
    pub fn filter(&mut self, column: &str, text: &str) -> E2eResult {
        self.locate(format!("filter:{column}")).fill(text)
    }
}
