//! Writing an expression as text, with every field, operator and function at hand.

use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_criteria::functions::catalog;
use ravenrs_criteria::{Category, Criteria, FieldType, Schema, validate};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint};
use crate::{
    Button, ButtonSize, ButtonStyle, FilterField, FilterFieldKind, IconName, Labels, Text, TextArea,
    TextCaret, TextField, TreeNode, TreeView, View, VirtualList, util,
};

/// Height of the lists under the text.
const LIST_HEIGHT: f32 = 200.0;
const ROW_HEIGHT: f32 = 28.0;

/// One thing the editor can insert.
#[derive(Clone, Debug, PartialEq)]
struct Item {
    /// What the list shows.
    title: String,
    /// What goes into the text.
    insert: String,
    /// How many characters back from the end of `insert` the caret lands, to sit
    /// between brackets or quotes.
    back: usize,
    /// What the panel beside the list says about it.
    about: String,
}

impl Item {
    fn new(
        title: impl Into<String>,
        insert: impl Into<String>,
        back: usize,
        about: impl Into<String>,
    ) -> Self {
        Self { title: title.into(), insert: insert.into(), back, about: about.into() }
    }
}

const FIELDS: &str = "fields";
const CONSTANTS: &str = "constants";
const OPERATORS: &str = "operators";
const FUNCTIONS: &str = "functions";

fn category_key(category: Category) -> &'static str {
    match category {
        Category::Logical => "fn-logical",
        Category::String => "fn-text",
        Category::Math => "fn-math",
        Category::DateTime => "fn-date",
        Category::Aggregate => "fn-aggregate",
        _ => "fn-reporting",
    }
}

fn category_label(category: Category, labels: &Labels) -> String {
    match category {
        Category::Logical => labels.expression_logical.clone(),
        Category::String => labels.expression_text.clone(),
        Category::Math => labels.expression_math.clone(),
        Category::DateTime => labels.expression_date.clone(),
        Category::Aggregate => labels.expression_aggregate.clone(),
        _ => labels.expression_reporting.clone(),
    }
}

fn kind_label(kind: FilterFieldKind, labels: &Labels) -> &str {
    match kind {
        FilterFieldKind::Number => &labels.expression_kind_number,
        FilterFieldKind::Boolean => &labels.expression_kind_boolean,
        FilterFieldKind::Date => &labels.expression_kind_date,
        _ => &labels.expression_kind_text,
    }
}

/// What a category of the tree offers.
fn items(key: &str, fields: &[FilterField], labels: &Labels) -> Vec<Item> {
    match key {
        FIELDS => fields
            .iter()
            .map(|f| Item::new(f.caption.clone(), format!("[{}]", f.name), 0, kind_label(f.kind, labels)))
            .collect(),
        CONSTANTS => vec![
            Item::new("True", "True", 0, labels.filter_yes.clone()),
            Item::new("False", "False", 0, labels.filter_no.clone()),
            Item::new("Null", "Null", 0, labels.expression_null.clone()),
            Item::new("'…'", "''", 1, labels.expression_text_value.clone()),
            Item::new("#…#", "##", 1, labels.expression_date_value.clone()),
        ],
        OPERATORS => {
            let op =
                |symbol: &str, insert: &str, back: usize, about: &str| Item::new(symbol, insert, back, about);
            vec![
                op("=", " = ", 0, &labels.filter_equal),
                op("<>", " <> ", 0, &labels.filter_not_equal),
                op(">", " > ", 0, &labels.filter_greater),
                op(">=", " >= ", 0, &labels.filter_greater_or_equal),
                op("<", " < ", 0, &labels.filter_less),
                op("<=", " <= ", 0, &labels.filter_less_or_equal),
                op("And", " And ", 0, &labels.expression_and),
                op("Or", " Or ", 0, &labels.expression_or),
                op("Not", "Not ", 0, &labels.expression_not),
                op("In", " In ()", 1, &labels.filter_any_of),
                op("Between", " Between(, )", 3, &labels.expression_between),
                op("Like", " Like ''", 1, &labels.expression_like),
                op("Is Null", " Is Null", 0, &labels.filter_is_empty),
                op("+", " + ", 0, &labels.expression_add),
                op("-", " - ", 0, &labels.expression_subtract),
                op("*", " * ", 0, &labels.expression_multiply),
                op("/", " / ", 0, &labels.expression_divide),
                op("%", " % ", 0, &labels.expression_modulo),
            ]
        }
        _ => catalog()
            .into_iter()
            .filter(|f| category_key(f.category) == key)
            .map(|f| {
                // A function that takes nothing has nothing to write between its brackets.
                let empty = f.signature.ends_with("()");
                Item::new(
                    f.name,
                    format!("{}()", f.name),
                    usize::from(!empty),
                    format!("{}\n\n{}", f.signature, f.description),
                )
            })
            .collect(),
    }
}

/// What is wrong with `text` over these fields, or `None` when it reads well.
fn problem(text: &str, fields: &[FilterField]) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    let criteria = match Criteria::parse(text) {
        Ok(criteria) => criteria,
        Err(e) => return Some(e.to_string()),
    };
    let schema = fields.iter().fold(Schema::new(), |schema, f| {
        let kind = match f.kind {
            FilterFieldKind::Number => FieldType::Number,
            FilterFieldKind::Boolean => FieldType::Boolean,
            FilterFieldKind::Date => FieldType::Date,
            _ => FieldType::Text,
        };
        schema.field(&f.name, kind)
    });
    validate(&criteria, &schema).first().map(ToString::to_string)
}

/// An editor for an expression written as text, like the expression editor of DevExpress:
/// the text on top, and under it the fields, constants, operators and functions by group,
/// a search over them and what the chosen one does. A double click, Enter or the Insert
/// button writes it at the caret. What does not read, or names a field that is not there,
/// is said under the text right away.
///
/// Only text that reads well reaches `value`, so whatever is bound to it never sees an
/// expression half written.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ExpressionEditor, FilterField, FilterFieldKind, Ui};
///
/// let expression = State::new(String::from("[Price] > 10"));
/// let editor = ExpressionEditor::new(
///     expression.binding(),
///     [FilterField::new("Price", "Price", FilterFieldKind::Number)],
/// );
/// let mut ui = Ui::new(editor, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct ExpressionEditor {
    value: Binding<String>,
    fields: Rc<Vec<FilterField>>,
    categories: Vec<Category>,
    rows: usize,
}

impl fmt::Debug for ExpressionEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionEditor").field("fields", &self.fields).finish_non_exhaustive()
    }
}

impl ExpressionEditor {
    /// An editor over the expression in `value`, offering `fields`.
    pub fn new(value: Binding<String>, fields: impl IntoIterator<Item = FilterField>) -> Self {
        Self {
            value,
            fields: Rc::new(fields.into_iter().collect()),
            categories: vec![Category::Logical, Category::String, Category::Math, Category::DateTime],
            rows: 4,
        }
    }

    /// Also offers the aggregates (`Count`, `Sum`, `Avg`…), for an expression over rows.
    pub fn aggregates(mut self, show: bool) -> Self {
        self.toggle(Category::Aggregate, show);
        self
    }

    /// Also offers the functions of printed reports (formatting, page numbers).
    pub fn reporting(mut self, show: bool) -> Self {
        self.toggle(Category::Reporting, show);
        self
    }

    /// Lines of text shown before it scrolls. 4 by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    fn toggle(&mut self, category: Category, show: bool) {
        self.categories.retain(|c| *c != category);
        if show {
            self.categories.push(category);
        }
    }
}

impl View for ExpressionEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let theme = *cx.theme();
        let outside = self.value.get();
        let seen: State<String> = {
            let first = outside.clone();
            cx.use_state(move || first)
        };
        let draft: State<String> = {
            let first = outside.clone();
            cx.use_state(move || first)
        };
        let caret: TextCaret = cx.use_state(TextCaret::new).get();
        let category: State<Option<String>> = cx.use_state(|| Some(FIELDS.to_owned()));
        let open: State<BTreeSet<String>> = cx.use_state(|| BTreeSet::from([FUNCTIONS.to_owned()]));
        let chosen: State<Option<usize>> = cx.use_state(|| None);
        let search: State<String> = cx.use_state(String::new);
        let last_category: State<Option<String>> = cx.use_state(|| None);

        // Someone else changed the expression: what is being written gives way.
        if outside != *seen.peek() {
            seen.set(outside.clone());
            draft.set(outside);
        }

        let text = {
            let (read, write, seen, value, fields) =
                (draft.clone(), draft.clone(), seen.clone(), self.value.clone(), self.fields.clone());
            Binding::new(
                move || read.get(),
                move |text: String| {
                    if problem(&text, &fields).is_none() && text != *seen.peek() {
                        seen.set(text.clone());
                        value.set(text.clone());
                    }
                    write.set(text);
                },
            )
        };
        let mut parts = vec![
            cx.child(
                &TextArea::new(text.clone())
                    .caret(caret.clone())
                    .rows(self.rows)
                    .placeholder(labels.filter_expression.clone()),
            ),
        ];
        if let Some(problem) = problem(&draft.get(), &self.fields) {
            parts.push(util::label(cx, &problem, TextStyle::Callout, None, theme.color(Semantic::Danger)));
        }

        // The tree of what can be inserted.
        let functions = TreeNode::new(FUNCTIONS, labels.expression_functions.clone()).children(
            self.categories.iter().map(|c| TreeNode::new(category_key(*c), category_label(*c, &labels))),
        );
        let nodes = vec![
            TreeNode::new(FIELDS, labels.expression_fields.clone()),
            TreeNode::new(CONSTANTS, labels.expression_constants.clone()),
            TreeNode::new(OPERATORS, labels.expression_operators.clone()),
            functions,
        ];
        let tree = cx.child(
            &TreeView::new(nodes)
                .selected(category.binding())
                .expanded(open.binding())
                .height(LIST_HEIGHT + 38.0)
                .row_height(ROW_HEIGHT)
                .label(labels.expression_categories.clone()),
        );
        let tree = cx.container(
            LayoutStyle { width: Some(170.0), align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[tree],
            Paint::None,
        );

        // A new category starts from the top, with nothing chosen.
        let key = category.get().unwrap_or_default();
        if last_category.peek().as_deref() != Some(key.as_str()) {
            last_category.set(Some(key.clone()));
            chosen.set(None);
            search.set(String::new());
        }
        let needle = search.get().to_lowercase();
        let shown: Rc<Vec<Item>> = Rc::new(
            items(&key, &self.fields, &labels)
                .into_iter()
                .filter(|i| needle.is_empty() || i.title.to_lowercase().contains(&needle))
                .collect(),
        );
        let insert = {
            let (shown, caret, text) = (shown.clone(), caret.clone(), text.clone());
            Rc::new(move |index: usize| {
                let Some(item) = shown.get(index) else { return };
                caret.insert(&text, &item.insert);
                let len = text.get().chars().count();
                caret.place(caret.position(len).saturating_sub(item.back));
            })
        };
        let search_field = cx.child(
            &TextField::new(search.binding())
                .placeholder(labels.expression_search.clone())
                .icon(IconName::Search)
                .compact(),
        );
        let list = {
            let (names, rows, activate) = (shown.clone(), shown.clone(), insert.clone());
            cx.child(
                &VirtualList::new(shown.len(), move |i| Text::new(rows[i].title.clone()).truncate())
                    .row_height(ROW_HEIGHT)
                    .selection(chosen.binding())
                    .on_activate(move |i| activate(i))
                    .row_name(move |i| names[i].title.clone())
                    .height(LIST_HEIGHT)
                    .label(labels.expression_items.clone()),
            )
        };
        let middle = cx.container(
            LayoutStyle {
                width: Some(200.0),
                spacing: 6.0,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[search_field, list],
            Paint::None,
        );

        // What the chosen one is, and a way to insert it without a double click.
        let picked = chosen.get().filter(|&i| i < shown.len());
        let mut about = Vec::new();
        if let Some(i) = picked {
            let item = &shown[i];
            about.push(util::label(
                cx,
                &item.title,
                TextStyle::Body,
                Some(Weight::SEMIBOLD),
                theme.color(Semantic::Ink),
            ));
            about.push(cx.child(&Text::new(item.about.clone()).style(TextStyle::Callout)));
            let insert = insert.clone();
            about.push(
                cx.child(
                    &Button::new(labels.expression_insert.clone())
                        .style(ButtonStyle::Secondary)
                        .size(ButtonSize::Small)
                        .icon(IconName::Plus)
                        .on_click(move || insert(i)),
                ),
            );
        }
        let about = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                spacing: 8.0,
                align: Align::Start,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &about,
            Paint::None,
        );
        parts.push(cx.container(
            LayoutStyle {
                spacing: 12.0,
                align: Align::Start,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[tree, middle, about],
            Paint::None,
        ));
        cx.container(
            LayoutStyle { spacing: 10.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &parts,
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<FilterField> {
        vec![
            FilterField::new("Name", "Name", FilterFieldKind::Text),
            FilterField::new("Price", "Price", FilterFieldKind::Number),
        ]
    }

    #[test]
    fn what_does_not_read_is_said() {
        assert_eq!(problem("", &fields()), None);
        assert_eq!(problem("[Price] > 10 And Contains([Name], 'a')", &fields()), None);
        assert!(problem("[Price] >", &fields()).is_some());
        assert!(problem("[Nope] = 1", &fields()).is_some(), "a field that is not there");
    }

    #[test]
    fn every_group_offers_something_that_reads() {
        let labels = Labels::default();
        for key in [FIELDS, CONSTANTS, OPERATORS, "fn-logical", "fn-text", "fn-math", "fn-date"] {
            let all = items(key, &fields(), &labels);
            assert!(!all.is_empty(), "{key}");
            for item in all {
                assert!(item.back <= item.insert.chars().count(), "{item:?}");
                assert!(!item.about.is_empty(), "{item:?}");
            }
        }
        let functions = items("fn-text", &fields(), &labels);
        let contains = functions.iter().find(|i| i.title == "Contains").expect("Contains");
        assert_eq!((contains.insert.as_str(), contains.back), ("Contains()", 1));
    }
}
