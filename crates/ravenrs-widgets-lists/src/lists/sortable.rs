//! A list whose rows the user puts in order.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Insets, Key, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::Semantic;

use crate::IconName;
use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::reorder::{self, Extents, Reorder};
use crate::util::{self, FocusTarget, Roving};
use crate::view::{AnyView, View};

/// A list that reorders by dragging a row, with a line showing where it will land, or with Alt
/// and the arrows, Home and End. The arrows alone move between rows, and each move is announced to
/// screen readers. The order belongs to the app, which applies what
/// [`on_reorder`](Self::on_reorder) tells it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{SortableList, Text, Ui};
///
/// let items = State::new(vec!["Warm", "Cool", "Matte"]);
/// let shown = items.get();
/// let moved = items.clone();
/// let list = SortableList::new(shown.len(), move |i| Text::new(shown[i]))
///     .on_reorder(move |from, to| moved.update(|v| {
///         let item = v.remove(from);
///         v.insert(to, item);
///     }));
/// let mut ui = Ui::new(list, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SortableList {
    count: usize,
    item: Rc<dyn Fn(usize) -> AnyView>,
    key: Option<Rc<dyn Fn(usize) -> u64>>,
    row_name: Option<Rc<dyn Fn(usize) -> String>>,
    on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
    spacing: f32,
    label: Option<String>,
    grip: bool,
}

impl fmt::Debug for SortableList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SortableList").field("count", &self.count).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SortableList {
    /// A list of `count` rows, in the order the app keeps.
    pub fn new<V: View + 'static>(count: usize, item: impl Fn(usize) -> V + 'static) -> Self {
        Self {
            count,
            item: Rc::new(move |index| Box::new(item(index))),
            key: None,
            row_name: None,
            on_reorder: None,
            spacing: 2.0,
            label: None,
            grip: true,
        }
    }

    /// What identifies a row, which is its position by default. With it, what a row keeps of
    /// its own goes with it when it moves.
    pub fn key(mut self, key: impl Fn(usize) -> u64 + 'static) -> Self {
        self.key = Some(Rc::new(key));
        self
    }

    /// Called with where a row was and where it should go.
    pub fn on_reorder(mut self, handler: impl Fn(usize, usize) + 'static) -> Self {
        self.on_reorder = Some(Rc::new(handler));
        self
    }

    /// What screen readers call each row, for rows whose content does not say it.
    pub fn row_name(mut self, name: impl Fn(usize) -> String + 'static) -> Self {
        self.row_name = Some(Rc::new(name));
        self
    }

    /// Gap between rows (2 by default).
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    /// What screen readers call the list, instead of the name in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Whether a row shows the mark that says it can be dragged (on by default).
    pub fn grip(mut self, grip: bool) -> Self {
        self.grip = grip;
        self
    }
}

struct SortableRow<'a> {
    list: &'a SortableList,
    index: usize,
    extents: Extents,
    drag: State<Option<Reorder>>,
    roving: Roving,
    apply: Rc<dyn Fn(usize, usize)>,
}

impl View for SortableRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let list = self.list;
        let index = self.index;
        let frame = cx.scroll_viewport();
        if let (Some(size), Some(slot)) = (frame, self.extents.borrow_mut().get_mut(index)) {
            *slot = size.height;
        }
        let state = self.drag.get();
        let dragged = state.is_some_and(|d| d.moving && d.from == index);
        let mut parts = Vec::with_capacity(2);
        if list.grip {
            parts.push(util::icon(cx, IconName::EllipsisVertical, 14.0, theme.color(Semantic::InkMuted)));
        }
        let item = (list.item)(index);
        let content = cx.child(&*item);
        let content = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content],
            Paint::None,
        );
        parts.push(content);
        if let (Some(gap), Some(size)) = (state.and_then(|d| d.marked_gap()), frame) {
            let half = list.spacing / 2.0;
            if gap == index {
                parts.push(reorder::marker(cx, true, size.width, -half));
            } else if gap == index + 1 && gap == list.count {
                parts.push(reorder::marker(cx, true, size.width, size.height + half));
            }
        }
        let ink = theme.color(Semantic::Ink);
        let fill = if dragged {
            theme.color(Semantic::Accent).with_alpha(0.14)
        } else if cx.is_hovered() {
            ink.with_alpha(0.05)
        } else {
            Color::TRANSPARENT
        };
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s2,
                padding: Insets::symmetric(theme.spacing.s1, theme.spacing.s2),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, ..QuadPaint::new(fill) }),
        );
        cx.report_frame(node);
        cx.cursor(node, CursorIcon::Pointer);
        let mut semantics =
            Semantics::new(Role::ListItem).description(labels.sortable_hint.clone()).active(dragged);
        if let Some(name) = &list.row_name {
            semantics = semantics.name(name(index));
        }
        cx.semantics(node, semantics);
        cx.focusable(node, theme.radii.sm);
        cx.on_drag(
            node,
            reorder::drag_handler(
                self.drag.clone(),
                self.extents.clone(),
                index,
                list.spacing,
                true,
                self.apply.clone(),
            ),
        );
        let (apply, count, called_off) = (self.apply.clone(), list.count, self.drag.clone());
        let before: crate::KeyHandler = Rc::new(move |key, modifiers| {
            if *key == Key::Escape && called_off.peek().is_some() {
                called_off.set(None);
                return true;
            }
            if let Some(to) = reorder::key_target(key, modifiers, index, count, true) {
                apply(index, to);
            }
            reorder::is_move_key(key, modifiers)
        });
        self.roving.wire_with(cx, node, None, Some(before));
        node
    }
}

impl View for SortableList {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let drag: State<Option<Reorder>> = cx.use_state(|| None);
        let extents: Extents = cx.use_state(|| Rc::new(RefCell::new(Vec::new()))).peek().clone();
        extents.borrow_mut().resize(self.count, 0.0);
        let spoken: State<String> = cx.use_state(String::new);
        let focus_to: FocusTarget = util::roving_cell(cx, None);
        let count = self.count;
        let apply: Rc<dyn Fn(usize, usize)> = {
            let (handler, spoken, focus_to) = (self.on_reorder.clone(), spoken.clone(), focus_to.clone());
            let words = labels.clone();
            Rc::new(move |from, to| {
                if let Some(handler) = &handler {
                    handler(from, to);
                    spoken.set(words.moved_to_position(to + 1, count));
                    focus_to.request(to);
                }
            })
        };
        let mut nodes = Vec::with_capacity(count + 1);
        for index in 0..count {
            let row = SortableRow {
                list: self,
                index,
                extents: extents.clone(),
                drag: drag.clone(),
                roving: Roving { index, count, focus_to: focus_to.clone() },
                apply: apply.clone(),
            };
            let key = self.key.as_ref().map_or(index as u64, |key| key(index));
            nodes.push(cx.keyed_child(key, &row));
        }
        let announcement = spoken.get();
        nodes.push(reorder::announcer(cx, announcement));
        let node = cx.container(
            LayoutStyle {
                spacing: self.spacing,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &nodes,
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.sortable_name.clone());
        cx.semantics(node, Semantics::new(Role::List).name(name).description(count.to_string()));
        node
    }
}
