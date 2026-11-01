//! Data grid data model: values, columns and the data source.

use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

use raven_reactive::State;

use crate::AnyView;

/// A cell value.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    Empty,
    Text(String),
    Number(f64),
    Bool(bool),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortDirection {
    Ascending,
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
pub enum Summary {
    Count,
    Sum,
    Average,
    Min,
    Max,
}

impl Summary {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellAlign {
    #[default]
    Start,
    Center,
    End,
}

/// How numbers are written: decimals, affixes and separators.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    pub decimals: usize,
    pub prefix: String,
    pub suffix: String,
    pub thousands: char,
    pub decimal: char,
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self { decimals: 0, prefix: String::new(), suffix: String::new(), thousands: ',', decimal: '.' }
    }
}

impl NumberFormat {
    pub fn format(&self, n: f64) -> String {
        if !n.is_finite() {
            return String::new();
        }
        let fixed = format!("{:.*}", self.decimals, n.abs());
        let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
        let mut grouped = String::with_capacity(int.len() + int.len() / 3);
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push(self.thousands);
            }
            grouped.push(c);
        }
        let sign = if n < 0.0 && fixed.chars().any(|c| c != '0' && c != '.') { "-" } else { "" };
        let mut out = format!("{sign}{}{grouped}", self.prefix);
        if !frac.is_empty() {
            out.push(self.decimal);
            out.push_str(frac);
        }
        out.push_str(&self.suffix);
        out
    }
}

pub type CellFormat = Rc<dyn Fn(&Value) -> String>;
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

    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(self.min_width);
        self
    }

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

    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    pub fn filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

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

    pub fn title(&self) -> &str {
        &self.title
    }

    /// The text shown for `value`.
    pub fn display(&self, value: &Value) -> String {
        if let Some(f) = &self.format {
            return f(value);
        }
        match value {
            Value::Empty => String::new(),
            Value::Text(s) => s.clone(),
            Value::Number(n) => self.number.as_ref().map_or_else(|| n.to_string(), |f| f.format(*n)),
            Value::Bool(b) => if *b { "Yes" } else { "No" }.to_owned(),
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
    pub fn new(rows: Vec<Row>) -> Self {
        Self { rows: State::new(Rc::new(rows)) }
    }

    /// The current rows (tracked: views reading it rebuild on change).
    pub fn rows(&self) -> Rc<Vec<Row>> {
        self.rows.get()
    }

    pub fn len(&self) -> usize {
        self.rows.peek().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set_cell(&self, row: usize, column: usize, value: Value) {
        self.rows.update(|rows| {
            if let Some(cell) = Rc::make_mut(rows).get_mut(row).and_then(|r| r.get_mut(column)) {
                *cell = value;
            }
        });
    }

    pub fn push(&self, row: Row) {
        self.rows.update(|rows| Rc::make_mut(rows).push(row));
    }

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
