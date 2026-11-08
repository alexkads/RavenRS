//! Writing a filter without knowing the expression language.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::Insets;
use ravenrs_criteria::{BinaryOp, Criteria, Function, GroupOp, UnaryOp, Value as Literal};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};

use crate::context::{BuildCx, NodeId, Paint};
use crate::{
    Button, ButtonSize, ButtonStyle, IconButton, IconName, LookupItem, Select, TagBox, TextField, View,
};

/// What a field of a [`FilterBuilder`] holds, which says how its values are written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum FilterFieldKind {
    /// Text.
    #[default]
    Text,
    /// A number.
    Number,
    /// Yes or no.
    Boolean,
    /// A date, written `2026-11-02`.
    Date,
}

/// A field a [`FilterBuilder`] offers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FilterField {
    /// The name in the expression.
    pub name: String,
    /// What people see.
    pub caption: String,
    /// What it holds.
    pub kind: FilterFieldKind,
    /// Values to pick from for "is any of": the key as the expression writes it
    /// (`2026-11-02` for a date) and the label people see.
    pub values: Vec<LookupItem>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FilterField {
    /// A field with this name in the expression, and this caption on the screen.
    pub fn new(name: impl Into<String>, caption: impl Into<String>, kind: FilterFieldKind) -> Self {
        Self { name: name.into(), caption: caption.into(), kind, values: Vec::new() }
    }

    /// The values offered for "is any of" and "is none of". Others can still be typed.
    pub fn values(mut self, values: impl IntoIterator<Item = LookupItem>) -> Self {
        self.values = values.into_iter().collect();
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Contains,
    StartsWith,
    AnyOf,
    NoneOf,
    IsEmpty,
    IsNotEmpty,
}

const ALL_OPS: [Op; 12] = [
    Op::Equal,
    Op::NotEqual,
    Op::Greater,
    Op::GreaterOrEqual,
    Op::Less,
    Op::LessOrEqual,
    Op::Contains,
    Op::StartsWith,
    Op::AnyOf,
    Op::NoneOf,
    Op::IsEmpty,
    Op::IsNotEmpty,
];

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Op {
    fn takes_value(self) -> bool {
        !matches!(self, Op::IsEmpty | Op::IsNotEmpty)
    }

    /// It compares with a list of values rather than one.
    fn takes_list(self) -> bool {
        matches!(self, Op::AnyOf | Op::NoneOf)
    }

    fn fits(self, kind: FilterFieldKind) -> bool {
        match kind {
            FilterFieldKind::Text => true,
            FilterFieldKind::Boolean => {
                matches!(self, Op::Equal | Op::NotEqual | Op::IsEmpty | Op::IsNotEmpty)
            }
            _ => !matches!(self, Op::Contains | Op::StartsWith),
        }
    }

    fn label(self, labels: &crate::Labels) -> &str {
        match self {
            Op::Equal => &labels.filter_equal,
            Op::NotEqual => &labels.filter_not_equal,
            Op::Greater => &labels.filter_greater,
            Op::GreaterOrEqual => &labels.filter_greater_or_equal,
            Op::Less => &labels.filter_less,
            Op::LessOrEqual => &labels.filter_less_or_equal,
            Op::Contains => &labels.filter_contains,
            Op::StartsWith => &labels.filter_starts_with,
            Op::AnyOf => &labels.filter_any_of,
            Op::NoneOf => &labels.filter_none_of,
            Op::IsEmpty => &labels.filter_is_empty,
            Op::IsNotEmpty => &labels.filter_is_not_empty,
        }
    }
}

/// One line or group of the builder. What it cannot show as a line stays as `Raw`,
/// so opening someone's expression never loses any of it. `values` is what a list
/// comparison holds; the others use `value`.
#[derive(Clone, Debug, PartialEq)]
enum Node {
    Condition { field: String, op: Op, value: String, values: Vec<String> },
    Raw(String),
    Group { all: bool, items: Vec<Node> },
}

fn literal_text(value: &Literal) -> Option<String> {
    match value {
        Literal::Text(text) => Some(text.clone()),
        Literal::Number(n) => Some(n.to_string()),
        Literal::Bool(b) => Some(b.to_string()),
        Literal::Date(date) => Some(date.to_string()),
        _ => None,
    }
}

fn field_name(criteria: &Criteria) -> Option<String> {
    if let Criteria::Property(path) = criteria { Some(path.join(".")) } else { None }
}

fn read_node(criteria: &Criteria) -> Node {
    let condition = |field: &Criteria, op: Op, value: &Criteria| -> Option<Node> {
        let Criteria::Value(value) = value else { return None };
        Some(Node::Condition {
            field: field_name(field)?,
            op,
            value: literal_text(value)?,
            values: Vec::new(),
        })
    };
    let list = |field: &Criteria, op: Op, items: &[Criteria]| -> Option<Node> {
        let values = items
            .iter()
            .map(|item| if let Criteria::Value(value) = item { literal_text(value) } else { None })
            .collect::<Option<Vec<String>>>()?;
        Some(Node::Condition { field: field_name(field)?, op, value: String::new(), values })
    };
    let read = match criteria {
        Criteria::Group(op, items) => {
            return Node::Group { all: *op == GroupOp::And, items: items.iter().map(read_node).collect() };
        }
        Criteria::Binary(op, field, value) => {
            let op = match op {
                BinaryOp::Eq => Some(Op::Equal),
                BinaryOp::Ne => Some(Op::NotEqual),
                BinaryOp::Gt => Some(Op::Greater),
                BinaryOp::Ge => Some(Op::GreaterOrEqual),
                BinaryOp::Lt => Some(Op::Less),
                BinaryOp::Le => Some(Op::LessOrEqual),
                _ => None,
            };
            op.and_then(|op| condition(field, op, value))
        }
        Criteria::Function(function, arguments) => match (function, arguments.as_slice()) {
            (Function::Contains, [field, value]) => condition(field, Op::Contains, value),
            (Function::StartsWith, [field, value]) => condition(field, Op::StartsWith, value),
            _ => None,
        },
        Criteria::In(field, items) => list(field, Op::AnyOf, items),
        Criteria::IsNull(field) => field_name(field).map(|field| Node::Condition {
            field,
            op: Op::IsEmpty,
            value: String::new(),
            values: Vec::new(),
        }),
        Criteria::Unary(UnaryOp::Not, inner) => match inner.as_ref() {
            Criteria::IsNull(field) => field_name(field).map(|field| Node::Condition {
                field,
                op: Op::IsNotEmpty,
                value: String::new(),
                values: Vec::new(),
            }),
            Criteria::In(field, items) => list(field, Op::NoneOf, items),
            _ => None,
        },
        _ => None,
    };
    read.unwrap_or_else(|| Node::Raw(criteria.to_string()))
}

/// What the expression says, as the tree the builder edits. A lone line goes
/// inside a group, so there is always a place to add another.
fn read_filter(text: &str) -> Node {
    if text.trim().is_empty() {
        return Node::Group { all: true, items: Vec::new() };
    }
    match Criteria::parse(text) {
        Ok(criteria) => match read_node(&criteria) {
            group @ Node::Group { .. } => group,
            line => Node::Group { all: true, items: vec![line] },
        },
        Err(_) => Node::Group { all: true, items: vec![Node::Raw(text.to_owned())] },
    }
}

fn write_value(value: &str, kind: FilterFieldKind) -> Option<String> {
    let value = value.trim();
    match kind {
        FilterFieldKind::Text => Some(format!("'{}'", value.replace('\'', "''"))),
        FilterFieldKind::Number => {
            value.replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| n.to_string())
        }
        FilterFieldKind::Boolean => match value.to_lowercase().as_str() {
            "true" => Some("True".to_owned()),
            "false" => Some("False".to_owned()),
            _ => None,
        },
        FilterFieldKind::Date => {
            let looks_right = value.len() >= 8 && value.chars().all(|c| c.is_ascii_digit() || c == '-');
            looks_right.then(|| format!("#{value}#"))
        }
    }
}

/// The expression of a node; `None` for one that is not finished yet.
fn write_node(node: &Node, fields: &[FilterField]) -> Option<String> {
    match node {
        Node::Raw(text) => Some(text.trim().to_owned()).filter(|t| !t.is_empty()),
        Node::Condition { field, op, value, values } => {
            let kind = fields.iter().find(|f| f.name == *field).map_or(FilterFieldKind::Text, |f| f.kind);
            let written = if op.takes_list() {
                let list = values.iter().map(|v| write_value(v, kind)).collect::<Option<Vec<String>>>()?;
                if list.is_empty() {
                    return None;
                }
                list.join(", ")
            } else if op.takes_value() {
                if value.trim().is_empty() {
                    return None;
                }
                write_value(value, kind)?
            } else {
                String::new()
            };
            Some(match op {
                Op::Equal => format!("[{field}] = {written}"),
                Op::NotEqual => format!("[{field}] <> {written}"),
                Op::Greater => format!("[{field}] > {written}"),
                Op::GreaterOrEqual => format!("[{field}] >= {written}"),
                Op::Less => format!("[{field}] < {written}"),
                Op::LessOrEqual => format!("[{field}] <= {written}"),
                Op::Contains => format!("Contains([{field}], {written})"),
                Op::StartsWith => format!("StartsWith([{field}], {written})"),
                Op::AnyOf => format!("[{field}] In ({written})"),
                Op::NoneOf => format!("Not [{field}] In ({written})"),
                Op::IsEmpty => format!("[{field}] Is Null"),
                Op::IsNotEmpty => format!("Not [{field}] Is Null"),
            })
        }
        Node::Group { all, items } => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    let text = write_node(item, fields)?;
                    let nested = matches!(item, Node::Group { items, .. } if items.len() > 1);
                    Some(if nested { format!("({text})") } else { text })
                })
                .collect();
            if parts.is_empty() { None } else { Some(parts.join(if *all { " And " } else { " Or " })) }
        }
    }
}

/// Walks to the node at `path`, for an edit coming from a button or a field.
fn node_at<'a>(root: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let mut node = root;
    for &i in path {
        let Node::Group { items, .. } = node else { return None };
        node = items.get_mut(i)?;
    }
    Some(node)
}

/// Builds a filter out of lines of field, comparison and value, in groups that
/// need all or any of their lines. It reads and writes the same expression language
/// as the rest of RavenRS, and a part it cannot show as a line stays editable as text.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FilterBuilder, FilterField, FilterFieldKind, Ui};
///
/// let filter = State::new(String::from("[Price] > 10"));
/// let builder = FilterBuilder::new(
///     filter.binding(),
///     [FilterField::new("Price", "Price", FilterFieldKind::Number)],
/// );
/// let mut ui = Ui::new(builder, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FilterBuilder {
    value: Binding<String>,
    fields: Rc<Vec<FilterField>>,
}

impl fmt::Debug for FilterBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilterBuilder").field("fields", &self.fields).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FilterBuilder {
    /// A builder over the expression in `value`, offering `fields`.
    pub fn new(value: Binding<String>, fields: impl IntoIterator<Item = FilterField>) -> Self {
        Self { value, fields: Rc::new(fields.into_iter().collect()) }
    }
}

/// What the rows of the builder need to edit the tree.
#[derive(Clone)]
struct Editor {
    tree: State<Node>,
    value: Binding<String>,
    fields: Rc<Vec<FilterField>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Editor {
    fn change(&self, path: &[usize], f: impl FnOnce(&mut Node)) {
        let mut tree = self.tree.get();
        if let Some(node) = node_at(&mut tree, path) {
            f(node);
        }
        let text = write_node(&tree, &self.fields).unwrap_or_default();
        self.tree.set(tree);
        if text != self.value.get() {
            self.value.set(text);
        }
    }

    fn remove(&self, path: &[usize]) {
        let Some((last, parent)) = path.split_last() else { return };
        self.change(parent, |node| {
            if let Node::Group { items, .. } = node
                && *last < items.len()
            {
                items.remove(*last);
            }
        });
    }

    fn new_line(&self) -> Node {
        let first = self.fields.first();
        Node::Condition {
            field: first.map_or_else(String::new, |f| f.name.clone()),
            op: if first.is_some_and(|f| f.kind == FilterFieldKind::Text) { Op::Contains } else { Op::Equal },
            value: String::new(),
            values: Vec::new(),
        }
    }
}

fn row_style() -> LayoutStyle {
    LayoutStyle {
        spacing: 8.0,
        align: Align::Center,
        align_self: Some(Align::Stretch),
        wrap: true,
        ..LayoutStyle::stack(Axis::Horizontal)
    }
}

fn remove_button(cx: &mut BuildCx<'_>, editor: &Editor, path: &[usize]) -> NodeId {
    let (editor, path) = (editor.clone(), path.to_vec());
    cx.child(
        &IconButton::new(IconName::X)
            .label(cx.labels().filter_remove.clone())
            .size(ButtonSize::Small)
            .on_click(move || editor.remove(&path)),
    )
}

fn build_condition(
    cx: &mut BuildCx<'_>,
    editor: &Editor,
    path: &[usize],
    field: &str,
    op: Op,
    value: &str,
    values: &[String],
) -> NodeId {
    let labels = cx.labels();
    let fields = editor.fields.clone();
    let at = fields.iter().position(|f| f.name == field);
    let kind = at.map_or(FilterFieldKind::Text, |i| fields[i].kind);

    let mut children = Vec::new();
    // A field no longer offered stays selectable, under its own name.
    let mut names: Vec<String> = fields.iter().map(|f| f.caption.clone()).collect();
    if at.is_none() {
        names.push(field.to_owned());
    }
    let chosen = at.unwrap_or(names.len() - 1);
    let (set, there, offered) = (editor.clone(), path.to_vec(), fields.clone());
    children.push(
        cx.child(
            &Select::new(
                Binding::new(
                    move || chosen,
                    move |i: usize| {
                        let Some(next) = offered.get(i).cloned() else { return };
                        set.change(&there, |node| {
                            if let Node::Condition { field, op, value, .. } = node {
                                *field = next.name.clone();
                                if !op.fits(next.kind) {
                                    *op = Op::Equal;
                                }
                                if next.kind == FilterFieldKind::Boolean {
                                    *value = "true".to_owned();
                                } else if value == "true" || value == "false" {
                                    value.clear();
                                }
                            }
                        });
                    },
                ),
                names,
            )
            .label(labels.filter_field.clone())
            .width(170.0),
        ),
    );

    let ops: Vec<Op> = ALL_OPS.iter().copied().filter(|o| o.fits(kind) || *o == op).collect();
    let current = ops.iter().position(|o| *o == op).unwrap_or(0);
    let names: Vec<String> = ops.iter().map(|o| o.label(&labels).to_owned()).collect();
    let (set, there) = (editor.clone(), path.to_vec());
    children.push(
        cx.child(
            &Select::new(
                Binding::new(
                    move || current,
                    move |i: usize| {
                        let Some(next) = ops.get(i).copied() else { return };
                        set.change(&there, |node| {
                            if let Node::Condition { op, value, values, .. } = node {
                                // What was written goes along to the other kind of comparison.
                                if next.takes_list() && !op.takes_list() {
                                    *values = Some(value.trim().to_owned())
                                        .filter(|v| !v.is_empty())
                                        .into_iter()
                                        .collect();
                                } else if !next.takes_list() && op.takes_list() {
                                    *value = values.first().cloned().unwrap_or_default();
                                }
                                *op = next;
                            }
                        });
                    },
                ),
                names,
            )
            .label(labels.filter_comparison.clone())
            .width(170.0),
        ),
    );

    if op.takes_list() {
        let (set, there, shown) = (editor.clone(), path.to_vec(), values.to_vec());
        let chosen = Binding::new(
            move || shown.clone(),
            move |picked: Vec<String>| {
                set.change(&there, |node| {
                    if let Node::Condition { values, .. } = node {
                        *values = picked;
                    }
                });
            },
        );
        let options = at.map(|i| fields[i].values.clone()).unwrap_or_default();
        children.push(
            cx.child(
                &TagBox::new(chosen, options)
                    .allow_new(true)
                    .placeholder(labels.filter_value.clone())
                    .label(labels.filter_value.clone())
                    .width(190.0),
            ),
        );
    } else if op.takes_value() {
        let (set, there) = (editor.clone(), path.to_vec());
        let shown = value.to_owned();
        if kind == FilterFieldKind::Boolean {
            children.push(
                cx.child(
                    &Select::new(
                        Binding::new(
                            move || usize::from(shown != "true"),
                            move |i: usize| {
                                set.change(&there, |node| {
                                    if let Node::Condition { value, .. } = node {
                                        *value = if i == 0 { "true" } else { "false" }.to_owned();
                                    }
                                });
                            },
                        ),
                        [labels.filter_yes.clone(), labels.filter_no.clone()],
                    )
                    .label(labels.filter_value.clone())
                    .width(120.0),
                ),
            );
        } else {
            let text = Binding::new(
                move || shown.clone(),
                move |text: String| {
                    set.change(&there, |node| {
                        if let Node::Condition { value, .. } = node {
                            *value = text;
                        }
                    });
                },
            );
            children
                .push(cx.child(&TextField::new(text).placeholder(labels.filter_value.clone()).width(180.0)));
        }
    }
    children.push(remove_button(cx, editor, path));
    cx.container(row_style(), &children, Paint::None)
}

fn build_raw(cx: &mut BuildCx<'_>, editor: &Editor, path: &[usize], text: &str) -> NodeId {
    let (set, there, shown) = (editor.clone(), path.to_vec(), text.to_owned());
    let binding = Binding::new(
        move || shown.clone(),
        move |text: String| {
            set.change(&there, |node| *node = Node::Raw(text));
        },
    );
    let field = cx.child(&TextField::new(binding).placeholder(cx.labels().filter_expression.clone()));
    let field = cx.container(
        LayoutStyle {
            grow: 1.0,
            min_width: Some(0.0),
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        },
        &[field],
        Paint::None,
    );
    let remove = remove_button(cx, editor, path);
    cx.container(row_style(), &[field, remove], Paint::None)
}

fn build_group(cx: &mut BuildCx<'_>, editor: &Editor, path: &[usize], all: bool, items: &[Node]) -> NodeId {
    let labels = cx.labels();
    let (set, there) = (editor.clone(), path.to_vec());
    let mode = cx.child(
        &Select::new(
            Binding::new(
                move || usize::from(!all),
                move |i: usize| {
                    set.change(&there, |node| {
                        if let Node::Group { all, .. } = node {
                            *all = i == 0;
                        }
                    });
                },
            ),
            [labels.filter_all.clone(), labels.filter_any.clone()],
        )
        .label(labels.filter_match.clone())
        .width(200.0),
    );
    let (set, there) = (editor.clone(), path.to_vec());
    let add = cx.child(
        &Button::new(labels.filter_add_condition.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .icon(IconName::Plus)
            .on_click(move || {
                let line = set.new_line();
                set.change(&there, |node| {
                    if let Node::Group { items, .. } = node {
                        items.push(line);
                    }
                });
            }),
    );
    let (set, there) = (editor.clone(), path.to_vec());
    let add_group = cx.child(
        &Button::new(labels.filter_add_group.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .icon(IconName::Plus)
            .on_click(move || {
                let line = set.new_line();
                set.change(&there, |node| {
                    if let Node::Group { items, .. } = node {
                        items.push(Node::Group { all: true, items: vec![line] });
                    }
                });
            }),
    );
    let mut heading = vec![mode, add, add_group];
    if !path.is_empty() {
        heading.push(remove_button(cx, editor, path));
    }
    let heading = cx.container(row_style(), &heading, Paint::None);
    let mut lines = vec![heading];
    for (i, item) in items.iter().enumerate() {
        let mut child = path.to_vec();
        child.push(i);
        lines.push(match item {
            Node::Condition { field, op, value, values } => {
                build_condition(cx, editor, &child, field, *op, value, values)
            }
            Node::Raw(text) => build_raw(cx, editor, &child, text),
            Node::Group { all, items } => build_group(cx, editor, &child, *all, items),
        });
    }
    cx.container(
        LayoutStyle {
            spacing: 8.0,
            padding: Insets { left: if path.is_empty() { 0.0 } else { 16.0 }, ..Insets::default() },
            align: Align::Stretch,
            align_self: Some(Align::Stretch),
            ..LayoutStyle::stack(Axis::Vertical)
        },
        &lines,
        Paint::None,
    )
}

impl View for FilterBuilder {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let text = self.value.get();
        let tree: State<Node> = {
            let first = text.clone();
            cx.use_state(move || read_filter(&first))
        };
        // Lines still being written say nothing yet, so only a real difference counts.
        let mine = write_node(&tree.peek(), &self.fields).unwrap_or_default();
        if mine != text {
            tree.set(read_filter(&text));
        }
        let editor = Editor { tree: tree.clone(), value: self.value.clone(), fields: self.fields.clone() };
        let Node::Group { all, items } = tree.get() else { return cx.empty() };
        build_group(cx, &editor, &[], all, &items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<FilterField> {
        vec![
            FilterField::new("Name", "Name", FilterFieldKind::Text),
            FilterField::new("Price", "Price", FilterFieldKind::Number),
            FilterField::new("Active", "Active", FilterFieldKind::Boolean),
            FilterField::new("Due", "Due", FilterFieldKind::Date),
        ]
    }

    /// What goes in comes out as the same expression, and what comes out is valid.
    fn round_trip(text: &str) -> String {
        let written = write_node(&read_filter(text), &fields()).unwrap_or_default();
        if !written.is_empty() {
            assert!(Criteria::parse(&written).is_ok(), "{written}");
        }
        written
    }

    #[test]
    fn lines_read_and_write_the_language() {
        for text in [
            "[Price] > 10",
            "[Price] <= 2.5",
            "[Name] = 'Acme'",
            "[Name] <> 'It''s'",
            "Contains([Name], 'ac')",
            "StartsWith([Name], 'ac')",
            "[Name] Is Null",
            "Not [Name] Is Null",
            "[Active] = True",
            "[Due] >= #2026-11-02#",
            "[Name] In ('Acme', 'It''s')",
            "Not [Name] In ('Acme')",
            "[Price] In (1, 2.5)",
            "[Due] In (#2026-11-02#)",
            "[Name] In ('Acme') And [Price] > 1",
            "[Price] > 1 And [Price] < 5",
            "[Price] > 1 Or [Name] = 'x'",
            "[Price] > 1 And ([Name] = 'x' Or [Name] = 'y')",
        ] {
            assert_eq!(round_trip(text), text);
        }
        assert_eq!(round_trip(""), "");
    }

    #[test]
    fn what_cannot_be_a_line_is_kept() {
        let kept = round_trip("Upper([Name]) = 'X' And [Price] > 1");
        assert!(kept.contains("Upper"), "{kept}");
        assert!(kept.contains("[Price] > 1"));
        let group = read_filter("Upper([Name]) = 'X'");
        assert!(matches!(&group, Node::Group { items, .. } if matches!(items[0], Node::Raw(_))));
        let broken = read_filter("[Name] = ");
        assert!(
            matches!(&broken, Node::Group { items, .. } if matches!(items[0], Node::Raw(_))),
            "{broken:?}"
        );
    }

    #[test]
    fn unfinished_lines_say_nothing() {
        let line = |field: &str, op, value: &str| Node::Condition {
            field: field.to_owned(),
            op,
            value: value.to_owned(),
            values: Vec::new(),
        };
        let tree = Node::Group {
            all: true,
            items: vec![
                line("Name", Op::Contains, ""),
                line("Price", Op::Greater, "abc"),
                line("Price", Op::Greater, "1,5"),
                Node::Group { all: false, items: vec![] },
            ],
        };
        assert_eq!(write_node(&tree, &fields()).as_deref(), Some("[Price] > 1.5"));
    }

    #[test]
    fn a_list_reads_as_one_line() {
        let read = read_filter("[Name] In ('a', 'b')");
        let Node::Group { items, .. } = &read else { panic!("{read:?}") };
        assert_eq!(
            items[0],
            Node::Condition {
                field: "Name".to_owned(),
                op: Op::AnyOf,
                value: String::new(),
                values: vec!["a".to_owned(), "b".to_owned()],
            }
        );
        // A list of anything but plain values stays an expression.
        let read = read_filter("[Name] In ([Other], 'b')");
        assert!(matches!(&read, Node::Group { items, .. } if matches!(items[0], Node::Raw(_))));
        let empty = Node::Condition {
            field: "Name".to_owned(),
            op: Op::AnyOf,
            value: String::new(),
            values: Vec::new(),
        };
        assert_eq!(write_node(&empty, &fields()), None, "nothing picked says nothing yet");
    }

    #[test]
    fn comparisons_fit_the_field() {
        assert!(Op::AnyOf.fits(FilterFieldKind::Date));
        assert!(!Op::AnyOf.fits(FilterFieldKind::Boolean));
        assert!(Op::Contains.fits(FilterFieldKind::Text));
        assert!(!Op::Contains.fits(FilterFieldKind::Number));
        assert!(!Op::Greater.fits(FilterFieldKind::Boolean));
        assert!(Op::IsEmpty.fits(FilterFieldKind::Date));
    }
}
