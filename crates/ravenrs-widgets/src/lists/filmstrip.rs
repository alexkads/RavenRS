//! A strip of thumbnails, as long as it needs to be.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Key, Rect, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::Semantic;

use super::selection;
use super::virtual_list::{ListProbe, ScrollCommand, VirtualList};
use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::grid::SelectionMode;
use crate::stack::EdgeArrow;
use crate::view::{AnyView, View};

/// How long an arrow at the end of the strip is.
const ARROW_LENGTH: f32 = 32.0;
/// How much of the view an arrow moves.
const ARROW_STEP: f32 = 0.8;

/// What a cell of a [`Filmstrip`] is told when it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FilmstripCell {
    /// Its position in the strip.
    pub index: usize,
    /// It is in the selection.
    pub selected: bool,
    /// The keyboard is on it.
    pub active: bool,
}

/// A horizontal strip of thumbnails that builds only what shows, so thousands cost the same as
/// a few. Each cell comes from whoever uses it, with its own image, stars or badges; the strip
/// draws the frame around it. A click selects alone, Ctrl or Cmd adds, and Shift takes a range;
/// the arrows, Home and End move and keep the selected cell in view, Space adds, Enter
/// activates. Arrows appear on the ends that have more to show.
///
/// ```
/// use std::collections::BTreeSet;
///
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Filmstrip, FilmstripCell, SelectionMode, Text, Ui};
///
/// let selected = State::new(BTreeSet::from([0_usize]));
/// let strip = Filmstrip::new(5_000, |cell: FilmstripCell| Text::new(format!("{}", cell.index + 1)))
///     .selection(selected.binding())
///     .mode(SelectionMode::Multiple);
/// let mut ui = Ui::new(strip, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Filmstrip {
    count: usize,
    cell: Rc<dyn Fn(FilmstripCell) -> AnyView>,
    key: Option<Rc<dyn Fn(usize) -> u64>>,
    selection: Option<Binding<BTreeSet<usize>>>,
    mode: SelectionMode,
    on_activate: Option<Rc<dyn Fn(usize)>>,
    cell_name: Option<Rc<dyn Fn(usize) -> String>>,
    cell_width: f32,
    height: f32,
    spacing: f32,
    label: Option<String>,
    fade: Semantic,
}

impl fmt::Debug for Filmstrip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filmstrip").field("count", &self.count).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Filmstrip {
    /// A strip of `count` cells, with `cell` making the view of each one when it shows.
    pub fn new<V: View + 'static>(count: usize, cell: impl Fn(FilmstripCell) -> V + 'static) -> Self {
        Self {
            count,
            cell: Rc::new(move |info| Box::new(cell(info))),
            key: None,
            selection: None,
            mode: SelectionMode::Single,
            on_activate: None,
            cell_name: None,
            cell_width: 96.0,
            height: 96.0,
            spacing: 6.0,
            label: None,
            fade: Semantic::Canvas,
        }
    }

    /// What identifies a cell, which is its position by default.
    pub fn key(mut self, key: impl Fn(usize) -> u64 + 'static) -> Self {
        self.key = Some(Rc::new(key));
        self
    }

    /// The selected cells, to read and to change from outside. A single one that changes from
    /// outside is brought into view.
    pub fn selection(mut self, selection: Binding<BTreeSet<usize>>) -> Self {
        self.selection = Some(selection);
        self
    }

    /// How cells can be selected ([`SelectionMode::Single`] by default).
    pub fn mode(mut self, mode: SelectionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Called with a cell on a double click or on Enter.
    pub fn on_activate(mut self, handler: impl Fn(usize) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// What screen readers call each cell.
    pub fn cell_name(mut self, name: impl Fn(usize) -> String + 'static) -> Self {
        self.cell_name = Some(Rc::new(name));
        self
    }

    /// The width of each cell (96 by default).
    pub fn cell_width(mut self, width: f32) -> Self {
        self.cell_width = width.max(1.0);
        self
    }

    /// The height of the strip (96 by default).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(1.0);
        self
    }

    /// Gap between cells (6 by default).
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    /// What screen readers call the strip, instead of the name in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The color the arrows fade from, which should be what is behind the strip.
    pub fn fade(mut self, fade: Semantic) -> Self {
        self.fade = fade;
        self
    }
}

/// The frame around a cell, and what makes it an option.
struct FilmCell {
    index: usize,
    selected: bool,
    active: bool,
    keyboard: bool,
    body: Rc<dyn Fn(FilmstripCell) -> AnyView>,
    name: Option<Rc<dyn Fn(usize) -> String>>,
    pick: Rc<dyn Fn(usize)>,
    open: Option<Rc<dyn Fn(usize)>>,
}

impl View for FilmCell {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered();
        let body =
            (self.body)(FilmstripCell { index: self.index, selected: self.selected, active: self.active });
        let child = cx.child(&*body);
        // By its thickness as well as its color: the state is not left to color alone.
        let border = if self.selected {
            Border { width: 2.0, color: theme.color(Semantic::Accent) }
        } else if self.active && self.keyboard {
            Border { width: 2.0, color: theme.color(Semantic::Ink) }
        } else if hovered {
            Border { width: 1.0, color: theme.color(Semantic::Ink).with_alpha(0.4) }
        } else {
            Border { width: 1.0, color: theme.color(Semantic::Border) }
        };
        let node = cx.container(
            LayoutStyle {
                grow: 1.0,
                overflow_hidden: true,
                align: Align::Stretch,
                padding: Insets::uniform(2.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[child],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                border: Some(border),
                ..QuadPaint::new(theme.color(Semantic::Surface))
            }),
        );
        let (pick, index) = (self.pick.clone(), self.index);
        cx.on_click(node, Rc::new(move || pick(index)));
        if let Some(open) = self.open.clone() {
            cx.on_double_click(node, Rc::new(move || open(index)));
        }
        cx.cursor(node, CursorIcon::Pointer);
        let mut semantics = Semantics::new(Role::ListBoxOption).selected(self.selected).active(self.active);
        if let Some(name) = &self.name {
            semantics = semantics.name(name(index));
        }
        cx.semantics(node, semantics);
        node
    }
}

impl View for Filmstrip {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let local: State<BTreeSet<usize>> = cx.use_state(BTreeSet::new);
        let selection = self.selection.clone().unwrap_or_else(|| local.binding());
        let commands: State<Option<ScrollCommand>> = cx.use_state(|| None);
        let cursor: State<usize> = cx.use_state(|| 0);
        let anchor: State<usize> = cx.use_state(|| 0);
        let probe = cx.use_state(|| Rc::new(Cell::new(ListProbe::default()))).peek().clone();
        // What the strip itself last made of the selection: anything else came from outside.
        let known: Rc<Cell<Option<usize>>> = cx.use_state(|| Rc::new(Cell::new(None))).peek().clone();
        let arrow_used: State<bool> = cx.use_state(|| false);

        let count = self.count;
        let selected = Rc::new(selection.get());
        let single = (selected.len() == 1).then(|| selected.iter().next().copied()).flatten();
        if single != known.get() {
            known.set(single);
            if let Some(index) = single {
                if *cursor.peek() != index {
                    cursor.set(index);
                    anchor.set(index);
                }
                commands.set(Some(ScrollCommand::Reveal(index)));
            }
        }
        let active = cursor.get().min(count.saturating_sub(1));
        let keyboard = cx.is_focus_visible();
        let mode = self.mode;
        let modifiers = cx.modifiers();

        let pick: Rc<dyn Fn(usize)> = {
            let (selection, cursor, anchor, known) =
                (selection.clone(), cursor.clone(), anchor.clone(), known.clone());
            Rc::new(move |index| {
                let mut next = anchor.get();
                selection.update(|set| {
                    next = selection::pick(set, mode, next, index, modifiers.get(), |i| i);
                    known.set((set.len() == 1).then(|| set.iter().next().copied()).flatten());
                });
                anchor.set(next);
                cursor.set(index);
            })
        };

        let (body, name, open, picker, shown) = (
            self.cell.clone(),
            self.cell_name.clone(),
            self.on_activate.clone(),
            pick.clone(),
            selected.clone(),
        );
        let mut list = VirtualList::new(count, move |index| FilmCell {
            index,
            selected: shown.contains(&index),
            active: index == active,
            keyboard,
            body: body.clone(),
            name: name.clone(),
            pick: picker.clone(),
            open: open.clone(),
        })
        .horizontal()
        .row_height(self.cell_width)
        .spacing(self.spacing)
        .padding(self.spacing)
        .height(self.height)
        .bare()
        .commands(commands.clone())
        .probe(probe.clone());
        list.key = self.key.clone();
        let strip = cx.child(&list);

        let seen = probe.get();
        let last = (seen.total - seen.view).max(0.0);
        let step = (seen.view * ARROW_STEP).max(ARROW_LENGTH);
        let arrow = |forward: bool| {
            let at = if forward { seen.view - ARROW_LENGTH } else { 0.0 };
            let (commands, arrow_used) = (commands.clone(), arrow_used.clone());
            let by = if forward { step } else { -step };
            EdgeArrow {
                forward,
                vertical: false,
                frame: Rect::new(at, 0.0, ARROW_LENGTH, self.height),
                fade: self.fade,
                label: if forward { labels.scroll_forward.clone() } else { labels.scroll_back.clone() },
                on_click: Rc::new(move || {
                    commands.set(Some(ScrollCommand::By(by)));
                    arrow_used.set(true);
                }),
            }
        };
        let mut nodes = vec![strip];
        let back = (seen.offset > 0.5).then(|| cx.keyed_child(1, &arrow(false)));
        let forward = (seen.offset < last - 0.5).then(|| cx.keyed_child(2, &arrow(true)));
        nodes.extend(back);
        nodes.extend(forward);
        // Reaching the end takes away the arrow that had the keyboard.
        if arrow_used.get() {
            arrow_used.set(false);
            match (back, forward) {
                (Some(other), None) | (None, Some(other)) => cx.request_focus_node(other),
                _ => {}
            }
        }
        let root = cx.container(
            LayoutStyle {
                height: Some(self.height),
                min_width: Some(0.0),
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &nodes,
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.filmstrip_name.clone());
        cx.semantics(root, Semantics::new(Role::ListBox).name(name).description(count.to_string()));
        cx.focusable(root, cx.theme().radii.sm);
        let open = self.on_activate.clone();
        let select_all = selection.clone();
        cx.on_key(
            root,
            Rc::new(move |key, modifiers| {
                if count == 0 {
                    return false;
                }
                let to = match key {
                    Key::ArrowLeft => active.saturating_sub(1),
                    Key::ArrowRight => (active + 1).min(count - 1),
                    Key::Home => 0,
                    Key::End => count - 1,
                    Key::Space => {
                        if mode == SelectionMode::Multiple {
                            let adding = ravenrs_core::Modifiers { ctrl: true, ..Default::default() };
                            let mut next = anchor.get();
                            selection.update(|set| {
                                next = selection::pick(set, mode, next, active, adding, |i| i);
                            });
                            anchor.set(next);
                        } else {
                            selection.set(BTreeSet::from([active]));
                        }
                        return true;
                    }
                    Key::Enter => {
                        if let Some(open) = &open {
                            open(active);
                        }
                        return true;
                    }
                    Key::Character(c) if selection::adds(modifiers) && c.eq_ignore_ascii_case("a") => {
                        if mode != SelectionMode::Multiple {
                            return false;
                        }
                        select_all.set((0..count).collect());
                        return true;
                    }
                    _ => return false,
                };
                cursor.set(to);
                // Ctrl or Cmd moves without changing the selection.
                if !selection::adds(modifiers) && mode != SelectionMode::None {
                    let mut next = anchor.get();
                    selection.update(|set| {
                        next = selection::pick(set, mode, next, to, modifiers, |i| i);
                        known.set((set.len() == 1).then(|| set.iter().next().copied()).flatten());
                    });
                    anchor.set(next);
                }
                commands.set(Some(ScrollCommand::Reveal(to)));
                true
            }),
        );
        root
    }
}
