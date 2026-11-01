//! The report definition: page, bands, groups, controls and styles. It is plain
//! data (serde), saved as JSON (`*.rvreport.json`) and built in code with the
//! fluent methods below. Units are points (1/72 inch).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A banded report definition.
///
/// ```
/// use ravenrs_report::{Band, Control, Report};
///
/// let report = Report::new("Customers")
///     .page_header(Band::new(24.0).control(Control::label("Name").at(0.0, 0.0, 200.0, 20.0)))
///     .detail(Band::new(20.0).control(Control::field("[Name]").at(0.0, 0.0, 200.0, 20.0)));
/// let json = report.to_json();
/// assert_eq!(ravenrs_report::Report::from_json(&json).expect("round trip"), report);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Report {
    /// The report's name, also the document title.
    pub name: String,
    /// Paper size and margins.
    pub page: PageSettings,
    /// Culture for numbers and dates, such as `pt-BR` or `en-US`.
    pub culture: String,
    /// Where the data comes from, saved with the layout. The engine renders
    /// whatever data it is given; designers and the report server load it from here.
    pub data_sources: Vec<DataSource>,
    /// The data source that feeds the report (the first one when unset).
    pub data_source: Option<String>,
    /// Path to the rows in the data (`orders`, `report.lines`). Without it, an
    /// array of data is the rows and an object is a single row.
    pub data_member: Option<String>,
    /// Rows for which this expression is false are left out.
    pub filter: Option<String>,
    /// Sorting of detail rows within their groups.
    pub sort: Vec<SortField>,
    /// Values the user supplies when running the report (`?Name` in expressions).
    pub parameters: Vec<Parameter>,
    /// Fields computed from other fields, usable like data fields.
    pub calculated_fields: Vec<CalculatedField>,
    /// Named styles that controls refer to.
    pub styles: BTreeMap<String, Style>,
    /// Printed once, at the top of the first page.
    pub report_header: Option<Band>,
    /// Printed at the top of every page.
    pub page_header: Option<Band>,
    /// Groups, outermost first.
    pub groups: Vec<Group>,
    /// Printed once per row.
    pub detail: Band,
    /// Nested reports printed after each row's detail (master-detail).
    pub detail_reports: Vec<DetailReport>,
    /// Printed once, after the last row.
    pub report_footer: Option<Band>,
    /// Printed at the bottom of every page.
    pub page_footer: Option<Band>,
}

/// A named source of report data, stored with the layout.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DataSource {
    /// Name, referred to by [`Report::data_source`].
    pub name: String,
    /// How the data is obtained.
    #[serde(flatten)]
    pub kind: DataSourceKind,
}

/// How a data source obtains its data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum DataSourceKind {
    /// Data stored in the layout itself (samples, small lookup tables).
    Json {
        /// The data.
        data: serde_json::Value,
    },
    /// JSON fetched over HTTP(S) when the report runs.
    Url {
        /// The address; `{?Name}` inserts a parameter value.
        url: String,
        /// Extra request headers. Secrets belong in the server's configuration, not here.
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    /// A SQL query run by the report server against a named connection.
    Sql {
        /// A connection name from the server's configuration (never a connection string with a password).
        connection: String,
        /// The query; `@Name` binds a report parameter.
        query: String,
    },
}

impl Default for DataSourceKind {
    fn default() -> Self {
        Self::Json { data: serde_json::Value::Null }
    }
}

/// Paper size and margins, in points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PageSettings {
    /// Paper width.
    pub width: f32,
    /// Paper height.
    pub height: f32,
    /// Top margin.
    pub margin_top: f32,
    /// Right margin.
    pub margin_right: f32,
    /// Bottom margin.
    pub margin_bottom: f32,
    /// Left margin.
    pub margin_left: f32,
}

impl Default for PageSettings {
    fn default() -> Self {
        Self::a4()
    }
}

impl PageSettings {
    /// A4 portrait (210 × 297 mm) with 1.5 cm margins.
    pub fn a4() -> Self {
        Self {
            width: 595.28,
            height: 841.89,
            margin_top: 42.52,
            margin_right: 42.52,
            margin_bottom: 42.52,
            margin_left: 42.52,
        }
    }

    /// US Letter portrait (8.5 × 11 in) with 0.5 in margins.
    pub fn letter() -> Self {
        Self {
            width: 612.0,
            height: 792.0,
            margin_top: 36.0,
            margin_right: 36.0,
            margin_bottom: 36.0,
            margin_left: 36.0,
        }
    }

    /// The same paper turned sideways.
    pub fn landscape(self) -> Self {
        Self { width: self.height.max(self.width), height: self.width.min(self.height), ..self }
    }

    /// Sets all four margins.
    pub fn margins(self, m: f32) -> Self {
        Self { margin_top: m, margin_right: m, margin_bottom: m, margin_left: m, ..self }
    }

    /// The width between the side margins, where bands are laid out.
    pub fn content_width(&self) -> f32 {
        (self.width - self.margin_left - self.margin_right).max(0.0)
    }
}

/// A sort key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SortField {
    /// The expression to sort by, such as `[Name]`.
    pub expression: String,
    /// Largest first.
    pub descending: bool,
}

/// What kind of value a parameter takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ParameterKind {
    /// Text.
    #[default]
    Text,
    /// A number.
    Number,
    /// A date.
    Date,
    /// Yes or no.
    Boolean,
}

/// A value supplied when the report runs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Parameter {
    /// Name, used as `?Name` in expressions.
    pub name: String,
    /// What the user is asked for.
    pub label: Option<String>,
    /// The kind of value.
    pub kind: ParameterKind,
    /// The value when none is supplied.
    pub default: Option<serde_json::Value>,
}

/// A field computed from an expression.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalculatedField {
    /// Name, used as `[Name]` in expressions.
    pub name: String,
    /// The expression, such as `[Quantity] * [UnitPrice]`.
    pub expression: String,
}

/// A grouping level with its header and footer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Group {
    /// Rows with the same value of this expression form a group.
    pub expression: String,
    /// Groups in descending order.
    pub descending: bool,
    /// Printed before each group.
    pub header: Option<Band>,
    /// Printed after each group; aggregates in it cover the group.
    pub footer: Option<Band>,
    /// Prints the header again at the top of each page the group continues on.
    pub repeat_header: bool,
    /// Starts each group (after the first) on a new page.
    pub page_break_before: bool,
}

/// Master-detail: a nested list printed after each row of the report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DetailReport {
    /// The array field of the master row that holds the nested rows.
    pub data_member: String,
    /// Nested rows for which this is false are left out.
    pub filter: Option<String>,
    /// Sorting of the nested rows.
    pub sort: Vec<SortField>,
    /// Printed before the nested rows (only if there are any).
    pub header: Option<Band>,
    /// Printed once per nested row; `[^.Field]` reads the master row.
    pub detail: Band,
    /// Printed after the nested rows; aggregates in it cover them.
    pub footer: Option<Band>,
}

/// A horizontal strip of the page holding controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Band {
    /// Height in points before any control grows.
    pub height: f32,
    /// The controls, positioned relative to the band's top-left corner.
    pub controls: Vec<Control>,
    /// The band is left out when this is false.
    pub visible: Option<String>,
    /// Starts a new page before the band.
    pub page_break_before: bool,
    /// Starts a new page after the band.
    pub page_break_after: bool,
    /// Keeps the band on the same page as the band that follows it (headers).
    pub keep_with_next: bool,
    /// Style name applied to odd rows of a detail band (1st, 3rd...).
    pub odd_style: Option<String>,
    /// Style name applied to even rows of a detail band.
    pub even_style: Option<String>,
}

impl Default for Band {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// A printable element of a band.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Control {
    /// What the control is and its content.
    #[serde(flatten)]
    pub kind: ControlKind,
    /// An optional name, shown in the designer and in error messages.
    pub name: Option<String>,
    /// Left edge, from the band's left.
    pub x: f32,
    /// Top edge, from the band's top.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height before growing.
    pub height: f32,
    /// A named style from [`Report::styles`].
    pub style_name: Option<String>,
    /// Style settings on top of the named style.
    pub style: Style,
    /// The control is left out when this is false.
    pub visible: Option<String>,
    /// Conditional formatting, applied in order when the condition holds.
    pub rules: Vec<FormattingRule>,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            kind: ControlKind::Label(Label::default()),
            name: None,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 18.0,
            style_name: None,
            style: Style::default(),
            visible: None,
            rules: Vec::new(),
        }
    }
}

/// The kinds of controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ControlKind {
    /// Text: fixed, or from an expression.
    Label(Label),
    /// A check mark from a boolean expression, with optional text.
    CheckBox(CheckBox),
    /// A straight line across the control's box.
    Line(Line),
    /// A rectangle or ellipse.
    Shape(Shape),
    /// A grid of cells, each a label.
    Table(Table),
    /// A container whose controls move together.
    Panel(Panel),
}

/// Text content of a label or table cell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Label {
    /// Fixed text, used when there is no expression.
    pub text: String,
    /// An expression whose value is shown, such as `[Total]`.
    pub expression: Option<String>,
    /// A format for the value: `{0:C2}`, or just `C2`.
    pub format: Option<String>,
    /// Grows downwards to fit wrapped text (true by default).
    pub can_grow: Option<bool>,
}

impl Label {
    /// Whether the label grows to fit its text.
    pub fn grows(&self) -> bool {
        self.can_grow.unwrap_or(true)
    }
}

/// A check box.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CheckBox {
    /// A boolean expression for the checked state.
    pub expression: String,
    /// Text after the box.
    pub text: String,
}

/// Which way a line runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum LineDirection {
    /// Across, centered vertically.
    #[default]
    Horizontal,
    /// Down, centered horizontally.
    Vertical,
    /// From top-left to bottom-right.
    Slant,
}

/// A line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Line {
    /// Its direction.
    pub direction: LineDirection,
    /// Thickness in points (1 by default).
    pub thickness: Option<f32>,
}

/// Shape kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ShapeKind {
    /// A rectangle, optionally rounded.
    #[default]
    Rectangle,
    /// An ellipse filling the box.
    Ellipse,
}

/// A rectangle or ellipse, filled with the style's background and outlined with its border.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Shape {
    /// The shape.
    pub shape: ShapeKind,
    /// Corner radius of a rectangle.
    pub radius: f32,
}

/// A table of label cells.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Table {
    /// The rows, top to bottom.
    pub rows: Vec<TableRow>,
}

/// A table row; its height grows to its tallest cell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableRow {
    /// Height before growing.
    pub height: f32,
    /// The cells, left to right.
    pub cells: Vec<TableCell>,
}

/// A table cell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableCell {
    /// Width in points.
    pub width: f32,
    /// The text.
    #[serde(flatten)]
    pub label: Label,
    /// A named style.
    pub style_name: Option<String>,
    /// Style settings on top of the named style.
    pub style: Style,
}

/// A container of controls.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Panel {
    /// The controls, positioned relative to the panel.
    pub controls: Vec<Control>,
}

/// Formatting applied when a condition holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormattingRule {
    /// A boolean expression, such as `[Total] > 1000`.
    pub condition: String,
    /// The settings applied.
    pub style: Style,
}

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum TextAlign {
    /// Left.
    #[default]
    Left,
    /// Centered.
    Center,
    /// Right (numbers).
    Right,
}

/// Vertical text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum VerticalAlign {
    /// Top.
    Top,
    /// Middle.
    #[default]
    Middle,
    /// Bottom.
    Bottom,
}

/// Appearance settings. Every field is optional so styles can be layered:
/// built-in defaults, then a named style, then the control's own settings,
/// then conditional rules.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Style {
    /// Font size in points.
    pub font_size: Option<f32>,
    /// Bold text.
    pub bold: Option<bool>,
    /// Text color, `#rrggbb` or `#rrggbbaa`.
    pub color: Option<String>,
    /// Fill color.
    pub background: Option<String>,
    /// Border color.
    pub border_color: Option<String>,
    /// Border width in points; 0 for none.
    pub border_width: Option<f32>,
    /// Which sides have a border: any of `top`, `right`, `bottom`, `left`, or `all`.
    pub borders: Option<String>,
    /// Inner padding in points.
    pub padding: Option<f32>,
    /// Horizontal alignment.
    pub text_align: Option<TextAlign>,
    /// Vertical alignment.
    pub vertical_align: Option<VerticalAlign>,
    /// Wraps long text onto more lines (true by default).
    pub word_wrap: Option<bool>,
}

impl Style {
    /// An empty style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Makes the text bold.
    pub fn bold(mut self) -> Self {
        self.bold = Some(true);
        self
    }

    /// Sets the text color (`#rrggbb`).
    pub fn color(mut self, color: &str) -> Self {
        self.color = Some(color.to_owned());
        self
    }

    /// Sets the fill color (`#rrggbb`).
    pub fn background(mut self, color: &str) -> Self {
        self.background = Some(color.to_owned());
        self
    }

    /// Adds a border of `width` points in `color` on `sides` (`all`, `bottom`, `top bottom`...).
    pub fn border(mut self, sides: &str, width: f32, color: &str) -> Self {
        self.borders = Some(sides.to_owned());
        self.border_width = Some(width);
        self.border_color = Some(color.to_owned());
        self
    }

    /// Sets the inner padding.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Sets the horizontal alignment.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.text_align = Some(align);
        self
    }

    /// Sets the vertical alignment.
    pub fn valign(mut self, align: VerticalAlign) -> Self {
        self.vertical_align = Some(align);
        self
    }

    /// Settings of `other` override these where `other` has them.
    pub fn merged(&self, other: &Style) -> Style {
        Style {
            font_size: other.font_size.or(self.font_size),
            bold: other.bold.or(self.bold),
            color: other.color.clone().or_else(|| self.color.clone()),
            background: other.background.clone().or_else(|| self.background.clone()),
            border_color: other.border_color.clone().or_else(|| self.border_color.clone()),
            border_width: other.border_width.or(self.border_width),
            borders: other.borders.clone().or_else(|| self.borders.clone()),
            padding: other.padding.or(self.padding),
            text_align: other.text_align.or(self.text_align),
            vertical_align: other.vertical_align.or(self.vertical_align),
            word_wrap: other.word_wrap.or(self.word_wrap),
        }
    }
}

impl Report {
    /// An empty report on A4 paper, in US English.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), culture: "en-US".into(), ..Self::default() }
    }

    /// Reads a report from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The report as indented JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Sets the paper.
    pub fn page(mut self, page: PageSettings) -> Self {
        self.page = page;
        self
    }

    /// Adds a data source and makes it the report's source.
    pub fn data_source(mut self, source: DataSource) -> Self {
        self.data_source = Some(source.name.clone());
        self.data_sources.push(source);
        self
    }

    /// The data of the report's source when it is stored in the layout.
    pub fn embedded_data(&self) -> Option<&serde_json::Value> {
        let source = match &self.data_source {
            Some(name) => self.data_sources.iter().find(|s| &s.name == name),
            None => self.data_sources.first(),
        }?;
        match &source.kind {
            DataSourceKind::Json { data } => Some(data),
            _ => None,
        }
    }

    /// Sets the culture (`pt-BR`, `en-US`).
    pub fn culture(mut self, culture: &str) -> Self {
        self.culture = culture.to_owned();
        self
    }

    /// Sets the path to the rows in the data.
    pub fn data_member(mut self, path: &str) -> Self {
        self.data_member = Some(path.to_owned());
        self
    }

    /// Keeps only rows where `expression` is true.
    pub fn filter(mut self, expression: &str) -> Self {
        self.filter = Some(expression.to_owned());
        self
    }

    /// Adds a sort key.
    pub fn sort_by(mut self, expression: &str, descending: bool) -> Self {
        self.sort.push(SortField { expression: expression.to_owned(), descending });
        self
    }

    /// Adds a parameter with a default value.
    pub fn parameter(mut self, name: &str, kind: ParameterKind, default: serde_json::Value) -> Self {
        self.parameters.push(Parameter { name: name.to_owned(), label: None, kind, default: Some(default) });
        self
    }

    /// Adds a calculated field.
    pub fn calculated(mut self, name: &str, expression: &str) -> Self {
        self.calculated_fields
            .push(CalculatedField { name: name.to_owned(), expression: expression.to_owned() });
        self
    }

    /// Adds a named style.
    pub fn style(mut self, name: &str, style: Style) -> Self {
        self.styles.insert(name.to_owned(), style);
        self
    }

    /// Sets the report header.
    pub fn report_header(mut self, band: Band) -> Self {
        self.report_header = Some(band);
        self
    }

    /// Sets the page header.
    pub fn page_header(mut self, band: Band) -> Self {
        self.page_header = Some(band);
        self
    }

    /// Adds a group level (outermost first).
    pub fn group(mut self, group: Group) -> Self {
        self.groups.push(group);
        self
    }

    /// Sets the detail band.
    pub fn detail(mut self, band: Band) -> Self {
        self.detail = band;
        self
    }

    /// Adds a master-detail report.
    pub fn detail_report(mut self, detail: DetailReport) -> Self {
        self.detail_reports.push(detail);
        self
    }

    /// Sets the report footer.
    pub fn report_footer(mut self, band: Band) -> Self {
        self.report_footer = Some(band);
        self
    }

    /// Sets the page footer.
    pub fn page_footer(mut self, band: Band) -> Self {
        self.page_footer = Some(band);
        self
    }
}

impl Group {
    /// Groups rows by `expression`.
    pub fn new(expression: &str) -> Self {
        Self { expression: expression.to_owned(), ..Self::default() }
    }

    /// Sets the header.
    pub fn header(mut self, band: Band) -> Self {
        self.header = Some(band);
        self
    }

    /// Sets the footer.
    pub fn footer(mut self, band: Band) -> Self {
        self.footer = Some(band);
        self
    }

    /// Repeats the header on every page the group continues on.
    pub fn repeat_header(mut self) -> Self {
        self.repeat_header = true;
        self
    }

    /// Starts each group on a new page.
    pub fn page_break_before(mut self) -> Self {
        self.page_break_before = true;
        self
    }
}

impl DetailReport {
    /// A nested list over the array field `data_member` of each master row.
    pub fn new(data_member: &str) -> Self {
        Self { data_member: data_member.to_owned(), ..Self::default() }
    }

    /// Sets the header.
    pub fn header(mut self, band: Band) -> Self {
        self.header = Some(band);
        self
    }

    /// Sets the detail band.
    pub fn detail(mut self, band: Band) -> Self {
        self.detail = band;
        self
    }

    /// Sets the footer.
    pub fn footer(mut self, band: Band) -> Self {
        self.footer = Some(band);
        self
    }
}

impl Band {
    /// An empty band `height` points tall.
    pub fn new(height: f32) -> Self {
        Self {
            height,
            controls: Vec::new(),
            visible: None,
            page_break_before: false,
            page_break_after: false,
            keep_with_next: false,
            odd_style: None,
            even_style: None,
        }
    }

    /// Adds a control.
    pub fn control(mut self, control: Control) -> Self {
        self.controls.push(control);
        self
    }

    /// Keeps the band on the page of the band after it.
    pub fn keep_with_next(mut self) -> Self {
        self.keep_with_next = true;
        self
    }

    /// Alternating row styles.
    pub fn zebra(mut self, odd: &str, even: &str) -> Self {
        self.odd_style = Some(odd.to_owned());
        self.even_style = Some(even.to_owned());
        self
    }
}

impl Control {
    /// A label with fixed text.
    pub fn label(text: impl Into<String>) -> Self {
        Self { kind: ControlKind::Label(Label { text: text.into(), ..Label::default() }), ..Self::default() }
    }

    /// A label showing the value of `expression`.
    pub fn field(expression: impl Into<String>) -> Self {
        Self {
            kind: ControlKind::Label(Label { expression: Some(expression.into()), ..Label::default() }),
            ..Self::default()
        }
    }

    /// A check box for a boolean expression.
    pub fn check_box(expression: &str, text: &str) -> Self {
        Self {
            kind: ControlKind::CheckBox(CheckBox {
                expression: expression.to_owned(),
                text: text.to_owned(),
            }),
            ..Self::default()
        }
    }

    /// A horizontal line.
    pub fn line() -> Self {
        Self { kind: ControlKind::Line(Line::default()), height: 2.0, ..Self::default() }
    }

    /// A rectangle.
    pub fn rect(radius: f32) -> Self {
        Self { kind: ControlKind::Shape(Shape { shape: ShapeKind::Rectangle, radius }), ..Self::default() }
    }

    /// A table.
    pub fn table(table: Table) -> Self {
        let height = table.rows.iter().map(|r| r.height).sum();
        let width = table.rows.first().map_or(0.0, |r| r.cells.iter().map(|c| c.width).sum());
        Self { kind: ControlKind::Table(table), width, height, ..Self::default() }
    }

    /// Positions and sizes the control.
    pub fn at(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
        self
    }

    /// Sets the value format (`C2`, `{0:dd/MM/yyyy}`...).
    pub fn format(mut self, format: &str) -> Self {
        if let ControlKind::Label(l) = &mut self.kind {
            l.format = Some(format.to_owned());
        }
        self
    }

    /// Uses a named style.
    pub fn style_name(mut self, name: &str) -> Self {
        self.style_name = Some(name.to_owned());
        self
    }

    /// Sets style settings on the control.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Prints the control only when `expression` is true.
    pub fn visible(mut self, expression: &str) -> Self {
        self.visible = Some(expression.to_owned());
        self
    }

    /// Adds conditional formatting.
    pub fn rule(mut self, condition: &str, style: Style) -> Self {
        self.rules.push(FormattingRule { condition: condition.to_owned(), style });
        self
    }

    /// Names the control.
    pub fn named(mut self, name: &str) -> Self {
        self.name = Some(name.to_owned());
        self
    }
}

impl Table {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a row.
    pub fn row(mut self, row: TableRow) -> Self {
        self.rows.push(row);
        self
    }
}

impl TableRow {
    /// An empty row `height` points tall.
    pub fn new(height: f32) -> Self {
        Self { height, cells: Vec::new() }
    }

    /// Adds a cell with fixed text.
    pub fn text(mut self, width: f32, text: &str) -> Self {
        self.cells.push(TableCell {
            width,
            label: Label { text: text.to_owned(), ..Label::default() },
            ..TableCell::default()
        });
        self
    }

    /// Adds a cell showing an expression, with an optional format.
    pub fn field(mut self, width: f32, expression: &str, format: Option<&str>) -> Self {
        self.cells.push(TableCell {
            width,
            label: Label {
                expression: Some(expression.to_owned()),
                format: format.map(str::to_owned),
                ..Label::default()
            },
            ..TableCell::default()
        });
        self
    }

    /// Styles the last cell added.
    pub fn cell_style(mut self, style: Style) -> Self {
        if let Some(c) = self.cells.last_mut() {
            c.style = style;
        }
        self
    }

    /// Uses a named style for every cell of the row.
    pub fn style_name(mut self, name: &str) -> Self {
        for c in &mut self.cells {
            c.style_name = Some(name.to_owned());
        }
        self
    }
}
