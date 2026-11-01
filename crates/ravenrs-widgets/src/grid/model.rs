//! Data grid data model: values, columns and the data source.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

use ravenrs_reactive::State;

use crate::AnyView;

/// A cell value.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// No value; shown as an empty cell.
    #[default]
    Empty,
    /// A string.
    Text(String),
    /// A number.
    Number(f64),
    /// A yes/no value.
    Bool(bool),
}

impl Value {
    /// The numeric value: numbers as is, booleans as 0.0 or 1.0, otherwise `None`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }

    /// The value is [`Value::Empty`].
    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.to_owned())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Text(s)
    }
}
impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}
impl From<f32> for Value {
    fn from(n: f32) -> Self {
        Value::Number(f64::from(n))
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n as f64)
    }
}
impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Number(f64::from(n))
    }
}
impl From<u32> for Value {
    fn from(n: u32) -> Self {
        Value::Number(f64::from(n))
    }
}
impl From<usize> for Value {
    fn from(n: usize) -> Self {
        Value::Number(n as f64)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

/// One row: values in column order.
pub type Row = Vec<Value>;

/// Order of a sorted column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortDirection {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
}

impl SortDirection {
    pub(crate) fn apply(self, o: Ordering) -> Ordering {
        match self {
            SortDirection::Ascending => o,
            SortDirection::Descending => o.reverse(),
        }
    }
}

/// An aggregate shown in group rows and in the footer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Summary {
    /// Number of non-empty cells.
    Count,
    /// Sum of the numeric values.
    Sum,
    /// Mean of the numeric values.
    Average,
    /// Smallest numeric value.
    Min,
    /// Largest numeric value.
    Max,
}

impl Summary {
    /// Short caption shown before the aggregate ("Sum", "Avg"…).
    pub fn label(self) -> &'static str {
        match self {
            Summary::Count => "Count",
            Summary::Sum => "Sum",
            Summary::Average => "Avg",
            Summary::Min => "Min",
            Summary::Max => "Max",
        }
    }
}

/// Horizontal alignment of a column's cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CellAlign {
    /// Left-aligned (text default).
    #[default]
    Start,
    /// Centered.
    Center,
    /// Right-aligned (number default).
    End,
}

/// How numbers are written: decimals, affixes and separators.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    /// Digits after the decimal separator.
    pub decimals: usize,
    /// Text before the number (e.g. a currency symbol).
    pub prefix: String,
    /// Text after the number (e.g. `"%"`).
    pub suffix: String,
    /// Thousands separator.
    pub thousands: char,
    /// Decimal separator.
    pub decimal: char,
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self { decimals: 0, prefix: String::new(), suffix: String::new(), thousands: ',', decimal: '.' }
    }
}

impl NumberFormat {
    /// Formats `n`; non-finite numbers give an empty string.
    pub fn format(&self, n: f64) -> String {
        if !n.is_finite() {
            return String::new();
        }
        ravenrs_criteria::NumberStyle {
            decimals: self.decimals,
            group: Some(self.thousands),
            decimal: self.decimal,
            prefix: &self.prefix,
            suffix: &self.suffix,
        }
        .format(n)
    }
}

/// Turns a cell value into its display text.
pub type CellFormat = Rc<dyn Fn(&Value) -> String>;
/// Builds a custom view for a cell value.
pub type CellTemplate = Rc<dyn Fn(&Value) -> AnyView>;

/// A data grid column: title, width, alignment, formatting and capabilities.
#[derive(Clone)]
pub struct GridColumn {
    pub(crate) title: String,
    pub(crate) width: f32,
    pub(crate) min_width: f32,
    pub(crate) align: CellAlign,
    pub(crate) number: Option<NumberFormat>,
    pub(crate) summary: Option<Summary>,
    pub(crate) sortable: bool,
    pub(crate) filterable: bool,
    pub(crate) groupable: bool,
    pub(crate) editable: bool,
    pub(crate) visible: bool,
    pub(crate) format: Option<CellFormat>,
    pub(crate) template: Option<CellTemplate>,
}

impl fmt::Debug for GridColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GridColumn")
            .field("title", &self.title)
            .field("width", &self.width)
            .finish_non_exhaustive()
    }
}

impl GridColumn {
    /// A text column.
    pub fn text(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: 160.0,
            min_width: 60.0,
            align: CellAlign::Start,
            number: None,
            summary: None,
            sortable: true,
            filterable: true,
            groupable: true,
            editable: false,
            visible: true,
            format: None,
            template: None,
        }
    }

    /// A right-aligned number column with thousands separators.
    pub fn number(title: impl Into<String>) -> Self {
        Self {
            align: CellAlign::End,
            number: Some(NumberFormat::default()),
            width: 120.0,
            ..Self::text(title)
        }
    }

    /// A number column formatted as money: `currency("MRR", "R$ ", 2)`.
    pub fn currency(title: impl Into<String>, prefix: &str, decimals: usize) -> Self {
        let format = NumberFormat { decimals, prefix: prefix.to_owned(), ..NumberFormat::default() };
        Self { number: Some(format), ..Self::number(title) }
    }

    /// A yes/no column.
    pub fn boolean(title: impl Into<String>) -> Self {
        Self { align: CellAlign::Center, width: 100.0, ..Self::text(title) }
    }

    /// Sets the width in logical pixels, never below the column's minimum.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(self.min_width);
        self
    }

    /// Sets the cell alignment.
    pub fn align(mut self, align: CellAlign) -> Self {
        self.align = align;
        self
    }

    /// Decimal places and separators for number columns.
    pub fn number_format(mut self, format: NumberFormat) -> Self {
        self.number = Some(format);
        self
    }

    /// Shows this aggregate in group rows and the footer.
    pub fn summary(mut self, summary: Summary) -> Self {
        self.summary = Some(summary);
        self
    }

    /// Whether clicking the header sorts by this column (default true).
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Whether the filter row has a field for this column (default true).
    pub fn filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// Whether rows can be grouped by this column (default true).
    pub fn groupable(mut self, groupable: bool) -> Self {
        self.groupable = groupable;
        self
    }

    /// Cells can be edited (double-click, Enter or F2).
    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Starts hidden; users can show it from the column chooser.
    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    /// Custom text for each value (also used by filtering, search and export).
    pub fn format(mut self, f: impl Fn(&Value) -> String + 'static) -> Self {
        self.format = Some(Rc::new(f));
        self
    }

    /// Custom view for each cell (a badge, an avatar...).
    pub fn template(mut self, f: impl Fn(&Value) -> AnyView + 'static) -> Self {
        self.template = Some(Rc::new(f));
        self
    }

    /// The header title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The text shown for `value`.
    pub fn display(&self, value: &Value) -> String {
        self.display_cow(value).into_owned()
    }

    /// [`GridColumn::display`] without copying text that is shown as stored.
    pub(crate) fn display_cow<'a>(&self, value: &'a Value) -> Cow<'a, str> {
        if let Some(f) = &self.format {
            return Cow::Owned(f(value));
        }
        match value {
            Value::Empty => Cow::Borrowed(""),
            Value::Text(s) => Cow::Borrowed(s),
            Value::Number(n) => {
                Cow::Owned(self.number.as_ref().map_or_else(|| n.to_string(), |f| f.format(*n)))
            }
            Value::Bool(b) => Cow::Borrowed(if *b { "Yes" } else { "No" }),
        }
    }

    /// Formats an aggregate of this column.
    pub fn display_summary(&self, summary: Summary, value: f64) -> String {
        match summary {
            Summary::Count => NumberFormat::default().format(value),
            _ => match &self.number {
                Some(f) => f.format(value),
                None => NumberFormat { decimals: 2, ..NumberFormat::default() }.format(value),
            },
        }
    }

    /// Parses edited text into a value of this column's kind.
    pub fn parse(&self, text: &str, previous: &Value) -> Value {
        let t = text.trim();
        match previous {
            Value::Number(_) | Value::Empty if self.number.is_some() => {
                let f = self.number.clone().unwrap_or_default();
                let cleaned: String = t
                    .trim_start_matches(f.prefix.as_str())
                    .trim_end_matches(f.suffix.as_str())
                    .chars()
                    .filter(|c| *c != f.thousands)
                    .map(|c| if c == f.decimal { '.' } else { c })
                    .collect();
                cleaned.trim().parse::<f64>().map_or(previous.clone(), Value::Number)
            }
            Value::Bool(_) => {
                Value::Bool(matches!(t.to_lowercase().as_str(), "yes" | "true" | "1" | "y" | "sim"))
            }
            _ if t.is_empty() => Value::Empty,
            _ => Value::Text(t.to_owned()),
        }
    }
}

/// The rows behind a data grid. Cloning shares the same data; editing replaces
/// the row list copy-on-write, so query caches stay valid until a change.
#[derive(Clone, Debug)]
pub struct GridData {
    rows: State<Rc<Vec<Row>>>,
}

impl GridData {
    /// Data over these rows.
    pub fn new(rows: Vec<Row>) -> Self {
        Self { rows: State::new(Rc::new(rows)) }
    }

    /// The current rows (tracked: views reading it rebuild on change).
    pub fn rows(&self) -> Rc<Vec<Row>> {
        self.rows.get()
    }

    /// Number of rows (untracked).
    pub fn len(&self) -> usize {
        self.rows.peek().len()
    }

    /// There are no rows (untracked).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Replaces one cell; out-of-range indices are ignored.
    pub fn set_cell(&self, row: usize, column: usize, value: Value) {
        self.rows.update(|rows| {
            if let Some(cell) = Rc::make_mut(rows).get_mut(row).and_then(|r| r.get_mut(column)) {
                *cell = value;
            }
        });
    }

    /// Appends a row.
    pub fn push(&self, row: Row) {
        self.rows.update(|rows| Rc::make_mut(rows).push(row));
    }

    /// Replaces all rows.
    pub fn replace(&self, rows: Vec<Row>) {
        self.rows.set(Rc::new(rows));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format_groups_and_rounds() {
        let f = NumberFormat {
            decimals: 2,
            prefix: "R$ ".into(),
            thousands: '.',
            decimal: ',',
            ..Default::default()
        };
        assert_eq!(f.format(1234567.891), "R$ 1.234.567,89");
        assert_eq!(f.format(-12.5), "-R$ 12,50");
        assert_eq!(NumberFormat::default().format(999.0), "999");
        assert_eq!(NumberFormat::default().format(1000.0), "1,000");
        assert_eq!(NumberFormat::default().format(-0.2), "0");
    }

    #[test]
    fn parse_round_trips_formatted_numbers() {
        let col = GridColumn::currency("MRR", "$", 2);
        assert_eq!(col.parse("$1,234.50", &Value::Number(0.0)), Value::Number(1234.5));
        assert_eq!(
            col.parse("oops", &Value::Number(7.0)),
            Value::Number(7.0),
            "invalid input keeps the old value"
        );
        assert_eq!(GridColumn::boolean("Active").parse("yes", &Value::Bool(false)), Value::Bool(true));
    }
}
