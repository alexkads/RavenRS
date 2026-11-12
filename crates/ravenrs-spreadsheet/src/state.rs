//! The workbook a [`Spreadsheet`](crate::Spreadsheet) shows, shared with the application.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_reactive::State;
use ravenrs_sheet::base::{CellRef, RangeRef};
use ravenrs_sheet::engine::{DocState, Selection, Session, Trust, display, io};
use ravenrs_sheet::model::Workbook;
use serde_json::{Value as Json, json};

/// Where an edit in progress is typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditPlace {
    /// Over the cell itself.
    Cell,
    /// In the formula bar.
    Bar,
}

/// An edit in progress: what the person typed so far, for which cell.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Edit {
    pub(crate) cell: CellRef,
    pub(crate) text: String,
    pub(crate) place: EditPlace,
}

struct Inner {
    session: RefCell<Session>,
    /// Goes up after every change, so the views showing the workbook rebuild.
    version: State<u64>,
    edit: State<Option<Edit>>,
    /// The latest refusal from the engine, shown until the next command.
    error: State<Option<String>>,
    /// The name the workbook is saved under.
    file_name: RefCell<String>,
}

/// A workbook with its selection and undo history, shared between a [`Spreadsheet`] and the
/// application. Clones share the same workbook.
///
/// Every change goes through the engine's commands (see [`Self::execute`]), which is how the
/// toolbar, the keyboard and the application change it alike.
///
/// ```
/// use ravenrs_spreadsheet::SpreadsheetState;
///
/// let sheet = SpreadsheetState::new();
/// sheet.set_cell("A1", "2").unwrap();
/// sheet.set_cell("A2", "=A1*21").unwrap();
/// assert_eq!(sheet.cell_text("A2"), "42");
/// ```
///
/// [`Spreadsheet`]: crate::Spreadsheet
#[derive(Clone)]
pub struct SpreadsheetState {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for SpreadsheetState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpreadsheetState")
            .field("file_name", &self.inner.file_name.borrow())
            .finish_non_exhaustive()
    }
}

impl Default for SpreadsheetState {
    fn default() -> Self {
        Self::new()
    }
}

impl SpreadsheetState {
    /// A new, empty workbook with one sheet.
    pub fn new() -> Self {
        let mut session = Session::new();
        session.new_workbook();
        Self::from_session(session, "Book1.xlsx")
    }

    /// Opens a workbook from the bytes of a file: `.xlsx`, `.xlsm`, `.xls`, `.ods`, `.csv` or
    /// `.tsv`, told apart by `name`'s extension and by the content.
    pub fn open(name: &str, bytes: &[u8]) -> Result<Self, String> {
        let state = Self::new();
        state.replace(name, bytes)?;
        Ok(state)
    }

    /// Replaces the workbook with the one in a file's bytes, as [`Self::open`] reads them.
    /// Undo starts over.
    pub fn replace(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let (workbook, _warnings) = io::open_bytes(name, bytes).map_err(|e| e.to_string())?;
        let title = name.rsplit_once('.').map_or(name, |(stem, _)| stem).to_owned();
        {
            let mut session = self.inner.session.borrow_mut();
            while !session.documents().is_empty() {
                session.close_document(0);
            }
            let index =
                session.add_document(DocState::new(workbook, Some(name.to_owned()), title, Trust::FromFile));
            session.set_active(index);
        }
        *self.inner.file_name.borrow_mut() = xlsx_name(name);
        self.inner.edit.set(None);
        self.inner.error.set(None);
        self.changed();
        Ok(())
    }

    fn from_session(session: Session, file_name: &str) -> Self {
        Self {
            inner: Rc::new(Inner {
                session: RefCell::new(session),
                version: State::new(0),
                edit: State::new(None),
                error: State::new(None),
                file_name: RefCell::new(file_name.to_owned()),
            }),
        }
    }

    /// Runs one of the engine's commands, such as `cell.set`, `home.bold` or `edit.undo`,
    /// with its parameters, and gives what it answered. A refused command changes nothing and
    /// its reason shows under the formula bar.
    pub fn execute(&self, command: &str, params: Json) -> Result<Json, String> {
        let result = self.inner.session.borrow_mut().execute(command, params).map_err(|e| e.to_string());
        self.inner.error.set(result.as_ref().err().cloned());
        self.changed();
        result
    }

    /// Types `input` into a cell given as `A1`: a number, text, a date or a `=formula`, read as
    /// a person typing it would be.
    pub fn set_cell(&self, cell: &str, input: &str) -> Result<(), String> {
        self.execute("cell.set", json!({ "cell": cell, "input": input })).map(|_| ())
    }

    /// What a cell given as `A1` shows on the active sheet, with its number format applied.
    pub fn cell_text(&self, cell: &str) -> String {
        let Some(at) = CellRef::parse(cell) else { return String::new() };
        self.read(|wb| wb.active().map(|sheet| display::cell_text(wb, sheet, at)).unwrap_or_default())
    }

    /// What editing a cell given as `A1` starts from: its formula, or the value as typed.
    pub fn cell_input(&self, cell: &str) -> String {
        let Some(at) = CellRef::parse(cell) else { return String::new() };
        self.read(|wb| wb.active().map(|sheet| display::edit_text(wb, sheet, at)).unwrap_or_default())
    }

    /// The active cell, as `A1`.
    pub fn active_cell(&self) -> String {
        self.selection_now().active.a1()
    }

    /// The selection, as `A1:B2` (areas joined by commas).
    pub fn selection(&self) -> String {
        self.selection_now().a1()
    }

    /// Selects a cell or a range given as `A1` or `A1:C3`.
    pub fn select(&self, range: &str) -> Result<(), String> {
        self.execute("selection.set", json!({ "range": range })).map(|_| ())
    }

    /// The names of the sheets, in order.
    pub fn sheet_names(&self) -> Vec<String> {
        self.read(|wb| wb.sheets.iter().map(|s| s.name.clone()).collect())
    }

    /// The position of the sheet being shown.
    pub fn active_sheet(&self) -> usize {
        self.read(Workbook::active_sheet)
    }

    /// The workbook as an `.xlsx` file.
    pub fn to_xlsx(&self) -> Result<Vec<u8>, String> {
        self.read(|wb| io::save_bytes(wb, "book.xlsx").map_err(|e| e.to_string()))
    }

    /// The active sheet's values as CSV.
    pub fn to_csv(&self) -> Result<Vec<u8>, String> {
        self.read(|wb| io::save_bytes(wb, "book.csv").map_err(|e| e.to_string()))
    }

    /// The name Save offers, ending in `.xlsx`.
    pub fn file_name(&self) -> String {
        self.inner.file_name.borrow().clone()
    }

    /// Changes the name Save offers; `.xlsx` is added when missing.
    pub fn set_file_name(&self, name: &str) {
        *self.inner.file_name.borrow_mut() = xlsx_name(name);
    }

    /// Whether there is something to undo.
    pub fn can_undo(&self) -> bool {
        self.inner.session.borrow().active().is_some_and(|d| !d.undo.is_empty())
    }

    /// Whether there is something to redo.
    pub fn can_redo(&self) -> bool {
        self.inner.session.borrow().active().is_some_and(|d| !d.redo.is_empty())
    }

    /// Reads the workbook, for anything the methods here do not cover.
    pub fn read<R>(&self, f: impl FnOnce(&Workbook) -> R) -> R {
        let session = self.inner.session.borrow();
        match session.active() {
            Some(doc) => f(&doc.wb),
            None => f(&Workbook::new()),
        }
    }

    /// Goes up after every change: reading it in a view rebuilds that view on the next one.
    pub fn version(&self) -> u64 {
        self.inner.version.get()
    }

    /// Runs `f` with the engine's session, for commands with replies or state not covered here.
    /// Views showing the workbook rebuild afterwards.
    pub fn with_session<R>(&self, f: impl FnOnce(&mut Session) -> R) -> R {
        let result = f(&mut self.inner.session.borrow_mut());
        self.changed();
        result
    }

    /// The latest refusal from the engine, if the last command was refused.
    pub fn error(&self) -> Option<String> {
        self.inner.error.get()
    }

    pub(crate) fn show_error(&self, error: String) {
        self.inner.error.set(Some(error));
    }

    pub(crate) fn selection_now(&self) -> Selection {
        self.inner.session.borrow().active().map(|d| d.selection.clone()).unwrap_or_default()
    }

    pub(crate) fn edit(&self) -> &State<Option<Edit>> {
        &self.inner.edit
    }

    /// Starts editing the active cell, from its content or from `text`.
    pub(crate) fn begin_edit(&self, text: Option<String>, place: EditPlace) {
        let cell = self.selection_now().active;
        let text = text.unwrap_or_else(|| self.cell_input(&cell.a1()));
        self.inner.edit.set(Some(Edit { cell, text, place }));
    }

    /// Types the edit in progress into its cell, then moves the active cell by (`dr`, `dc`).
    pub(crate) fn commit_edit(&self, dr: i64, dc: i64) {
        let Some(edit) = self.inner.edit.get() else { return };
        self.inner.edit.set(None);
        let done = self.execute("cell.set", json!({ "cell": edit.cell.a1(), "input": edit.text }));
        if done.is_ok() && (dr != 0 || dc != 0) {
            self.move_by(dr, dc, false);
        }
    }

    pub(crate) fn cancel_edit(&self) {
        self.inner.edit.set(None);
    }

    pub(crate) fn move_by(&self, dr: i64, dc: i64, extend: bool) {
        let _ = self.execute("selection.move", json!({ "dr": dr, "dc": dc, "extend": extend }));
    }

    pub(crate) fn select_range(&self, range: RangeRef, active: CellRef) {
        let _ = self.execute("selection.set", json!({ "range": range.a1(), "active": active.a1() }));
    }

    fn changed(&self) {
        self.inner.version.update(|v| *v = v.wrapping_add(1));
    }
}

/// `name` with its extension changed to `.xlsx`.
fn xlsx_name(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let stem = if stem.trim().is_empty() { "Book1" } else { stem };
    format!("{stem}.xlsx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_recalculate_when_their_inputs_change() {
        let s = SpreadsheetState::new();
        s.set_cell("A1", "10").unwrap();
        s.set_cell("A2", "32").unwrap();
        s.set_cell("A3", "=SUM(A1:A2)").unwrap();
        assert_eq!(s.cell_text("A3"), "42");
        s.set_cell("A1", "20").unwrap();
        assert_eq!(s.cell_text("A3"), "52");
        assert_eq!(s.cell_input("A3"), "=SUM(A1:A2)");
    }

    #[test]
    fn a_workbook_survives_a_trip_through_xlsx() {
        let s = SpreadsheetState::new();
        s.set_cell("B2", "=6*7").unwrap();
        s.set_cell("C3", "olá").unwrap();
        let bytes = s.to_xlsx().unwrap();
        let back = SpreadsheetState::open("conta.xlsx", &bytes).unwrap();
        assert_eq!(back.cell_text("B2"), "42");
        assert_eq!(back.cell_text("C3"), "olá");
        assert_eq!(back.file_name(), "conta.xlsx");
    }

    #[test]
    fn undo_puts_the_cell_back_and_refusals_are_kept() {
        let s = SpreadsheetState::new();
        s.set_cell("A1", "1").unwrap();
        s.set_cell("A1", "2").unwrap();
        assert!(s.can_undo());
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.cell_text("A1"), "1");
        assert!(s.execute("no.such", json!({})).is_err());
        assert!(s.error().is_some());
        assert_eq!(xlsx_name("dados.csv"), "dados.xlsx");
    }
}
