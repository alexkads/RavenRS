//! An editor for expressions in the criteria language: the text, what can go in it
//! grouped and searchable, a description of the chosen item, and the problems found
//! while typing.

use std::fmt;

use ravenrs_criteria::{Category, Criteria, FieldType, FunctionInfo, Problem, Schema, functions, validate};
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{SPACING, Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId};
use crate::text_field::{Cursor, byte_index};
use crate::{
    Alert, AnyView, Button, ButtonSize, FilterField, FilterFieldKind, HStack, IconName, ScrollView,
    SidebarItem, Text, TextArea, TextField, Tone, VStack, View, ViewExt,
};

/// The function groups, in the order of [`Labels::expression_groups`](crate::Labels::expression_groups)
/// after fields and parameters.
const FUNCTION_GROUPS: [Category; 6] = [
    Category::Logical,
    Category::String,
    Category::Math,
    Category::DateTime,
    Category::Reporting,
    Category::Aggregate,
];
const OPERATORS: usize = 8;
const CONSTANTS: usize = 9;

/// Where the expression editor's groups and lists stand.
const LISTS_HEIGHT: f32 = 340.0;
const GROUPS_WIDTH: f32 = 140.0;
const ITEMS_WIDTH: f32 = 200.0;

/// An expression editor, as report designers have: the expression in several lines, the
/// fields, parameters, functions, operators and constants that can go in it, a search,
/// the signature and description of the chosen one, and a check as the text changes.
///
/// A click on an item shows what it is; a second click, or the insert button, puts it
/// where the caret is. Syntax errors say their line and column; fields, parameters and
/// functions the editor does not know are listed apart, since the expression may still
/// run where they exist.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ExpressionEditor, FilterField, FilterFieldKind, Ui};
///
/// let expression = State::new("[Quantity] * [UnitPrice]".to_owned());
/// let editor = ExpressionEditor::new(expression.binding())
///     .field(FilterField::new("Quantity", "Quantity", FilterFieldKind::Number))
///     .field(FilterField::new("UnitPrice", "Unit price", FilterFieldKind::Number))
///     .parameter(FilterField::new("MinTotal", "Least total", FilterFieldKind::Number));
/// let mut ui = Ui::new(editor, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ExpressionEditor {
    value: Binding<String>,
    fields: Vec<FilterField>,
    parameters: Vec<FilterField>,
    rows: usize,
}

impl fmt::Debug for ExpressionEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionEditor")
            .field("fields", &self.fields.len())
            .field("parameters", &self.parameters.len())
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ExpressionEditor {
    /// An editor of `value`, offering the built-in functions, operators and constants.
    pub fn new(value: Binding<String>) -> Self {
        Self { value, fields: Vec::new(), parameters: Vec::new(), rows: 5 }
    }

    /// Offers a field, written `[name]`; a dotted name reads a nested value.
    pub fn field(mut self, field: FilterField) -> Self {
        self.fields.push(field);
        self
    }

    /// Offers these fields.
    pub fn fields(mut self, fields: impl IntoIterator<Item = FilterField>) -> Self {
        self.fields.extend(fields);
        self
    }

    /// Offers a parameter, written `?name`.
    pub fn parameter(mut self, parameter: FilterField) -> Self {
        self.parameters.push(parameter);
        self
    }

    /// Offers these parameters.
    pub fn parameters(mut self, parameters: impl IntoIterator<Item = FilterField>) -> Self {
        self.parameters.extend(parameters);
        self
    }

    /// How many lines of the expression show before it scrolls (5 by default).
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// What the names offered declare, for the check.
    fn schema(&self) -> Schema {
        let kind = |k: FilterFieldKind| match k {
            FilterFieldKind::Text => FieldType::Text,
            FilterFieldKind::Number => FieldType::Number,
            FilterFieldKind::Boolean => FieldType::Boolean,
            FilterFieldKind::Date => FieldType::Date,
        };
        let schema = self.fields.iter().fold(Schema::new(), |s, f| s.field(&f.name, kind(f.kind)));
        self.parameters.iter().fold(schema, |s, p| s.parameter(&p.name, kind(p.kind)))
    }

    /// The items of group `group`.
    fn items(&self, group: usize, labels: &crate::Labels) -> Vec<Item> {
        let named = |f: &FilterField, insert: String| Item {
            title: f.caption.clone(),
            insert,
            back: 0,
            signature: String::new(),
            help: String::new(),
        };
        match group {
            0 => self.fields.iter().map(|f| named(f, format!("[{}]", f.name))).collect(),
            1 => self.parameters.iter().map(|p| named(p, format!("?{}", p.name))).collect(),
            OPERATORS => labels
                .expression_operators
                .iter()
                .map(|(op, help)| {
                    let (insert, back) = match op.as_str() {
                        "Between" | "In" => (format!(" {op}()"), 1),
                        _ => (format!(" {op} "), 0),
                    };
                    Item { title: op.clone(), insert, back, signature: String::new(), help: help.clone() }
                })
                .collect(),
            CONSTANTS => labels
                .expression_constants
                .iter()
                .map(|(value, help)| Item {
                    title: value.clone(),
                    insert: value.clone(),
                    back: usize::from(value == "''"),
                    signature: String::new(),
                    help: help.clone(),
                })
                .collect(),
            g => {
                let Some(category) = FUNCTION_GROUPS.get(g.wrapping_sub(2)) else { return Vec::new() };
                let mut list: Vec<FunctionInfo> =
                    functions::catalog().into_iter().filter(|f| f.category == *category).collect();
                list.sort_by_key(|f| f.name.to_lowercase());
                list.dedup_by_key(|f| f.name);
                list.into_iter()
                    .map(|f| Item {
                        title: f.name.to_owned(),
                        insert: format!("{}()", f.name),
                        back: 1,
                        signature: f.signature.to_owned(),
                        help: f.description.to_owned(),
                    })
                    .collect()
            }
        }
    }
}

/// Something an expression editor lists.
#[derive(Clone, Debug, PartialEq)]
struct Item {
    title: String,
    /// What goes into the text.
    insert: String,
    /// How many characters before the end of `insert` the caret lands, such as inside
    /// a function's parentheses.
    back: usize,
    signature: String,
    help: String,
}

/// Puts `item` where the caret is, over the selection if there is one.
fn insert(value: &Binding<String>, cursor: &State<Cursor>, item: &Item) {
    let text = value.get();
    let chars = text.chars().count();
    let c = cursor.get();
    let caret = c.caret.min(chars);
    let (start, end) = c.selection().map_or((caret, caret), |(a, b)| (a.min(chars), b.min(chars)));
    let (a, b) = (byte_index(&text, start), byte_index(&text, end));
    value.set(format!("{}{}{}", &text[..a], item.insert, &text[b..]));
    cursor.set(Cursor::at(start + item.insert.chars().count() - item.back));
}

/// The line and column, from 1, of byte `offset` in `text`.
fn line_and_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

/// How a problem the check found reads.
fn problem_name(problem: &Problem) -> String {
    match problem {
        Problem::UnknownField(n) | Problem::UnknownCollection(n) => format!("[{n}]"),
        Problem::UnknownParameter(n) => format!("?{n}"),
        Problem::UnknownFunction(n) => format!("{n}()"),
        other => other.to_string(),
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl ExpressionEditor {
    /// What the check says about `text`, if anything.
    fn status(&self, text: &str, labels: &crate::Labels) -> Option<Alert> {
        if text.trim().is_empty() {
            return None;
        }
        match Criteria::parse(text) {
            Err(error) => {
                let (line, column) = line_and_column(text, error.offset);
                let at = labels
                    .expression_error_at
                    .replace("{line}", &line.to_string())
                    .replace("{column}", &column.to_string());
                Some(Alert::new(at, error.message).tone(Tone::Danger).compact())
            }
            Ok(criteria) => {
                let problems = validate(&criteria, &self.schema());
                if problems.is_empty() {
                    Some(Alert::message(labels.expression_valid.clone()).tone(Tone::Success).compact())
                } else {
                    let names: Vec<String> = problems.iter().map(problem_name).collect();
                    Some(
                        Alert::new(labels.expression_unknown.clone(), names.join(", "))
                            .tone(Tone::Warning)
                            .compact(),
                    )
                }
            }
        }
    }
}

impl View for ExpressionEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let cursor: State<Cursor> = cx.use_state(|| Cursor::at(usize::MAX));
        let group: State<usize> = cx.use_state(|| 0);
        let chosen: State<Option<usize>> = cx.use_state(|| None);
        let query: State<String> = cx.use_state(String::new);

        let text = self.value.get();
        let status = self.status(&text, &labels);

        // The groups, with how many items each has.
        let open = group.get();
        let groups: Vec<AnyView> = labels
            .expression_groups
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let (g, c) = (group.clone(), chosen.clone());
                Box::new(
                    SidebarItem::text(name.clone())
                        .compact()
                        .count(self.items(i, &labels).len().to_string())
                        .selected(i == open)
                        .on_click(move || {
                            g.set(i);
                            c.set(None);
                        }),
                ) as AnyView
            })
            .collect();

        // The items of the open group that match the search, or of every group while searching.
        let needle = query.get().trim().to_lowercase();
        let pool: Vec<Item> = if needle.is_empty() {
            self.items(open, &labels)
        } else {
            (0..labels.expression_groups.len()).flat_map(|g| self.items(g, &labels)).collect()
        };
        let shown: Vec<Item> = pool
            .into_iter()
            .filter(|item| {
                needle.is_empty()
                    || item.title.to_lowercase().contains(&needle)
                    || item.insert.to_lowercase().contains(&needle)
            })
            .collect();
        let picked = chosen.get().and_then(|i| shown.get(i)).cloned();
        let rows: Vec<AnyView> = if shown.is_empty() {
            vec![Box::new(
                Text::new(labels.expression_no_match.clone())
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            )]
        } else {
            shown
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let (c, value, caret, it) =
                        (chosen.clone(), self.value.clone(), cursor.clone(), item.clone());
                    Box::new(
                        SidebarItem::text(item.title.clone())
                            .compact()
                            .selected(picked.as_ref() == Some(item) && chosen.get() == Some(i))
                            .on_click(move || {
                                // A second click on the chosen item puts it in.
                                if c.get() == Some(i) {
                                    insert(&value, &caret, &it);
                                } else {
                                    c.set(Some(i));
                                }
                            }),
                    ) as AnyView
                })
                .collect()
        };

        let description: AnyView = match &picked {
            Some(item) => {
                let (value, caret, it) = (self.value.clone(), cursor.clone(), item.clone());
                let signature = if item.signature.is_empty() {
                    item.insert.trim().to_owned()
                } else {
                    item.signature.clone()
                };
                Box::new(
                    VStack::new((
                        Text::new(signature).font_weight(Weight::SEMIBOLD),
                        (!item.help.is_empty()).then(|| {
                            Text::new(item.help.clone())
                                .style(TextStyle::Callout)
                                .foreground(Semantic::InkMuted)
                        }),
                        Button::new(labels.expression_insert.clone())
                            .icon(IconName::Plus)
                            .size(ButtonSize::Small)
                            .on_click(move || insert(&value, &caret, &it)),
                    ))
                    .spacing(SPACING.s2)
                    .align(Align::Start),
                )
            }
            None => Box::new(VStack::new(Vec::<AnyView>::new())),
        };

        let editor = VStack::new((
            TextArea::new(self.value.clone()).rows(self.rows).cursor(cursor.clone()),
            status,
            HStack::new((
                ScrollView::new((VStack::new(groups).spacing(2.0).align(Align::Stretch),))
                    .width(GROUPS_WIDTH),
                VStack::new((
                    TextField::new(query.binding())
                        .placeholder(labels.expression_search.clone())
                        .icon(IconName::Search)
                        .compact(),
                    ScrollView::new((VStack::new(rows).spacing(2.0).align(Align::Stretch),)).grow(1.0),
                ))
                .spacing(SPACING.s2)
                .align(Align::Stretch)
                .width(ITEMS_WIDTH),
                ScrollView::new((description,)).grow(1.0),
            ))
            .spacing(SPACING.s3)
            .align(Align::Stretch)
            .height(LISTS_HEIGHT),
        ))
        .spacing(SPACING.s3)
        .align(Align::Stretch);
        cx.child(&editor)
    }
}
