//! Arranging the fields of a form: groups, columns, tabs and what is wrong.

use std::fmt;
use std::rc::Rc;

use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint};
use crate::{Alert, AnyView, Tabs, Tone, View, util};

const FIELD_GAP: f32 = 12.0;
const COLUMN_GAP: f32 = 16.0;
const GROUP_GAP: f32 = 20.0;
/// A field narrower than this moves to a line of its own, wherever the form is.
const MIN_COLUMN_WIDTH: f32 = 180.0;

enum Cell {
    Field(AnyView),
    Wide(AnyView),
}

/// A titled set of fields in a [`FormLayout`], filling its columns row by row.
#[derive(Default)]
pub struct FormGroup {
    title: Option<String>,
    columns: Option<usize>,
    cells: Vec<Cell>,
}

impl fmt::Debug for FormGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormGroup").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FormGroup {
    /// A group with no title.
    pub fn new() -> Self {
        Self::default()
    }

    /// A group under this title.
    pub fn titled(title: impl Into<String>) -> Self {
        Self { title: Some(title.into()), ..Self::default() }
    }

    /// How many columns this group uses, instead of the form's.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = Some(columns.max(1));
        self
    }

    /// Appends a field, in the next free column.
    pub fn field(mut self, field: impl View + 'static) -> Self {
        self.cells.push(Cell::Field(Box::new(field)));
        self
    }

    /// Appends a field on a row of its own, as wide as the group: a long text, a list.
    pub fn wide(mut self, field: impl View + 'static) -> Self {
        self.cells.push(Cell::Wide(Box::new(field)));
        self
    }
}

/// Lays out `cells` in rows of `columns`: the index of the first cell of each
/// row, with how many it holds. A wide cell has a row of its own.
fn form_rows(wide: &[bool], columns: usize) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    let mut start = 0;
    while start < wide.len() {
        let mut count = 1;
        if !wide[start] {
            while count < columns && start + count < wide.len() && !wide[start + count] {
                count += 1;
            }
        }
        rows.push((start, count));
        start += count;
    }
    rows
}

/// A view shown in more than one build, by a parent that takes ownership.
struct Shared(Rc<AnyView>);

impl View for Shared {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(self.0.as_ref().as_ref())
    }
}

/// The body of a form: groups of fields in columns, tabs below them, and a list of
/// what keeps it from being saved at the top. Columns collapse when the form is narrow.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{FormField, FormGroup, FormLayout, TextField, Ui};
///
/// let (name, city) = (State::new(String::new()), State::new(String::new()));
/// let form = FormLayout::new().columns(2).group(
///     FormGroup::titled("Contact")
///         .field(FormField::new("Name", TextField::new(name.binding())))
///         .field(FormField::new("City", TextField::new(city.binding()))),
/// );
/// let mut ui = Ui::new(form, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Default)]
pub struct FormLayout {
    columns: Option<usize>,
    errors: Vec<String>,
    groups: Vec<FormGroup>,
    tabs: Vec<(String, Rc<AnyView>)>,
}

impl fmt::Debug for FormLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormLayout").field("groups", &self.groups).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FormLayout {
    /// An empty form with one column.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many columns the groups use, when the window has room for them.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = Some(columns.max(1));
        self
    }

    /// What keeps the form from being saved, listed at the top. Empty shows nothing.
    pub fn errors(mut self, errors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.errors = errors.into_iter().map(Into::into).collect();
        self
    }

    /// Appends a group of fields.
    pub fn group(mut self, group: FormGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Appends a tab under the groups, for long parts such as lists of related records.
    pub fn tab(mut self, title: impl Into<String>, content: impl View + 'static) -> Self {
        self.tabs.push((title.into(), Rc::new(Box::new(content))));
        self
    }
}

impl View for FormLayout {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut parts = Vec::new();
        if !self.errors.is_empty() {
            let title = cx.labels().form_errors.clone();
            parts.push(cx.child(&Alert::new(title, self.errors.join("\n")).tone(Tone::Danger)));
        }
        for (g, group) in self.groups.iter().enumerate() {
            let mut lines = Vec::new();
            if let Some(title) = &group.title {
                lines.push(util::label(cx, title, TextStyle::Headline, None, theme.color(Semantic::Ink)));
            }
            let wide: Vec<bool> = group.cells.iter().map(|c| matches!(c, Cell::Wide(_))).collect();
            let columns = group.columns.or(self.columns).unwrap_or(1);
            for (start, count) in form_rows(&wide, columns) {
                let mut cells = Vec::with_capacity(columns);
                for (i, cell) in group.cells[start..start + count].iter().enumerate() {
                    let (Cell::Field(view) | Cell::Wide(view)) = cell;
                    let key = (g as u64) << 32 | (start + i) as u64;
                    let node = cx.keyed_child(key, view.as_ref());
                    cells.push(cx.container(
                        LayoutStyle {
                            grow: 1.0,
                            width: Some(MIN_COLUMN_WIDTH),
                            align: Align::Stretch,
                            ..LayoutStyle::stack(Axis::Vertical)
                        },
                        &[node],
                        Paint::None,
                    ));
                }
                // Keeps a short last row's fields as wide as the rows above.
                if !wide[start] {
                    while cells.len() < columns {
                        cells.push(cx.container(
                            LayoutStyle { grow: 1.0, width: Some(MIN_COLUMN_WIDTH), ..LayoutStyle::leaf() },
                            &[],
                            Paint::None,
                        ));
                    }
                }
                lines.push(cx.container(
                    LayoutStyle {
                        spacing: COLUMN_GAP,
                        align: Align::Start,
                        align_self: Some(Align::Stretch),
                        wrap: true,
                        ..LayoutStyle::stack(Axis::Horizontal)
                    },
                    &cells,
                    Paint::None,
                ));
            }
            parts.push(cx.container(
                LayoutStyle {
                    spacing: FIELD_GAP,
                    align: Align::Stretch,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &lines,
                Paint::None,
            ));
        }
        if !self.tabs.is_empty() {
            let selected: State<usize> = cx.use_state(|| 0);
            let mut tabs = Tabs::new(selected.binding());
            for (title, content) in &self.tabs {
                tabs = tabs.tab(title.clone(), Shared(content.clone()));
            }
            parts.push(cx.child(&tabs));
        }
        cx.container(
            LayoutStyle {
                spacing: GROUP_GAP,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &parts,
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::form_rows;

    #[test]
    fn fields_fill_rows_and_wide_ones_take_their_own() {
        assert_eq!(form_rows(&[], 2), []);
        assert_eq!(form_rows(&[false, false, false], 2), [(0, 2), (2, 1)]);
        assert_eq!(form_rows(&[false, true, false, false], 2), [(0, 1), (1, 1), (2, 2)]);
        assert_eq!(form_rows(&[false, false], 1), [(0, 1), (1, 1)]);
    }
}
