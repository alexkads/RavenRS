//! Tabs for open documents.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Insets, Key, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::quick_edit::is_f2;
use crate::reorder::{self, Extents, Reorder};
use crate::stack::ScrollEdgeArrow;
use crate::util::{self, FocusTarget, Roving};
use crate::view::{Keyed, View};
use crate::{ButtonSize, ContextMenu, Dot, HStack, IconButton, IconName, InlineRename, MenuItem, Tone};

/// The widest a title gets; a longer one ends in `…`.
const TITLE_MAX_WIDTH: f32 = 160.0;
const TAB_HEIGHT: f32 = 34.0;
/// How thick the band of a tab's color is.
const COLOR_BAND: f32 = 3.0;

/// An open document in a [`DocumentTabs`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DocumentTab {
    /// What identifies it, whatever its place.
    pub id: u64,
    /// What it is called.
    pub title: String,
    /// A color for telling it apart.
    pub color: Option<Color>,
    /// It has changes that were not saved.
    pub unsaved: bool,
    /// An icon before its title.
    pub icon: Option<IconName>,
    /// It has a button that closes it (on by default).
    pub closable: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DocumentTab {
    /// A tab called `title`.
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self { id, title: title.into(), color: None, unsaved: false, icon: None, closable: true }
    }

    /// A color for telling it apart.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Whether it has changes that were not saved, shown with a dot.
    pub fn unsaved(mut self, unsaved: bool) -> Self {
        self.unsaved = unsaved;
        self
    }

    /// An icon before its title.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Whether it has a button that closes it.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// Tabs for open documents, as a browser or an editor has: they close, change places by dragging
/// or with Alt and the arrows, are renamed with a double click, F2 or the menu, carry a color and
/// a dot for what was not saved, and scroll with arrows when they do not fit. The arrows alone
/// move between them, and Delete closes. The tabs belong to the app, which applies what the
/// handlers tell it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{DocumentTab, DocumentTabs, Ui};
///
/// let active = State::new(1_u64);
/// let tabs = DocumentTabs::new(
///     vec![DocumentTab::new(1, "Wedding"), DocumentTab::new(2, "Portraits").unsaved(true)],
///     active.binding(),
/// )
/// .on_close(|id| println!("close {id}"))
/// .on_reorder(|from, to| println!("{from} to {to}"));
/// let mut ui = Ui::new(tabs, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DocumentTabs {
    tabs: Vec<DocumentTab>,
    active: Binding<u64>,
    on_close: Option<Rc<dyn Fn(u64)>>,
    on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
    on_rename: Option<Rc<dyn Fn(u64, String)>>,
    on_new: Option<Rc<dyn Fn()>>,
    menu: Option<Rc<dyn Fn(u64) -> Vec<MenuItem>>>,
    label: Option<String>,
    fade: Semantic,
}

impl fmt::Debug for DocumentTabs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentTabs").field("tabs", &self.tabs).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DocumentTabs {
    /// These tabs, in this order, with the id of the open one in `active`.
    pub fn new(tabs: Vec<DocumentTab>, active: Binding<u64>) -> Self {
        Self {
            tabs,
            active,
            on_close: None,
            on_reorder: None,
            on_rename: None,
            on_new: None,
            menu: None,
            label: None,
            fade: Semantic::Canvas,
        }
    }

    /// Called with a tab to close, and turns on what closes them.
    pub fn on_close(mut self, handler: impl Fn(u64) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// Called with where a tab was and where it should go, and lets them be moved.
    pub fn on_reorder(mut self, handler: impl Fn(usize, usize) + 'static) -> Self {
        self.on_reorder = Some(Rc::new(handler));
        self
    }

    /// Called with a tab and its new name, and lets them be renamed.
    pub fn on_rename(mut self, handler: impl Fn(u64, String) + 'static) -> Self {
        self.on_rename = Some(Rc::new(handler));
        self
    }

    /// Shows a button at the end that calls this.
    pub fn on_new(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_new = Some(Rc::new(handler));
        self
    }

    /// More items for a tab's menu, above the ones it already has.
    pub fn menu(mut self, items: impl Fn(u64) -> Vec<MenuItem> + 'static) -> Self {
        self.menu = Some(Rc::new(items));
        self
    }

    /// What screen readers call the strip, instead of the name in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The color behind the strip, which its arrows fade from.
    pub fn fade(mut self, fade: Semantic) -> Self {
        self.fade = fade;
        self
    }
}

struct TabView<'a> {
    strip: &'a DocumentTabs,
    tab: &'a DocumentTab,
    index: usize,
    extents: Extents,
    drag: State<Option<Reorder>>,
    editing: State<bool>,
    roving: Roving,
    spoken: State<String>,
}

impl View for TabView<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (tab, strip, index) = (self.tab, self.strip, self.index);
        let frame = cx.scroll_viewport();
        if let (Some(size), Some(slot)) = (frame, self.extents.borrow_mut().get_mut(index)) {
            *slot = size.width;
        }
        let on = strip.active.get() == tab.id;
        let hovered = cx.is_hover_within();
        let editing = self.editing.get() && strip.on_rename.is_some();
        let ink = theme.color(if on { Semantic::Ink } else { Semantic::InkMuted });
        let mut parts = Vec::with_capacity(6);
        // Tabs keep their width and the strip scrolls: squeezed, none of them could be read.
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        let title_width = if editing {
            TITLE_MAX_WIDTH
        } else {
            (cx.measure_text(&tab.title, &attrs).width.ceil() + 1.0).min(TITLE_MAX_WIDTH)
        };
        let gap = theme.spacing.s2;
        let closes = tab.closable && strip.on_close.is_some();
        let right_pad = if closes { theme.spacing.s2 } else { theme.spacing.s3 };
        let mut width = theme.spacing.s3 + right_pad + title_width;
        if tab.icon.is_some() {
            width += 14.0 + gap;
        }
        if tab.unsaved {
            width += 7.0 + gap;
        }
        if closes {
            width += ButtonSize::Mini.height() + gap;
        }
        if let Some(icon) = tab.icon {
            parts.push(util::icon(cx, icon, 14.0, ink));
        }
        if editing {
            let name: State<String> = cx.use_state(|| tab.title.clone());
            name.set_if_changed(tab.title.clone());
            let (rename, id) = (strip.on_rename.clone(), tab.id);
            let field = InlineRename::new(name.binding())
                .editing(self.editing.binding())
                .label(labels.rename.clone())
                .width(TITLE_MAX_WIDTH)
                .on_rename(move |new| {
                    if let Some(rename) = &rename {
                        rename(id, new);
                    }
                });
            parts.push(cx.child(&field));
        } else {
            let _: State<String> = cx.use_state(|| tab.title.clone());
            let title = cx.text_truncated(
                &tab.title,
                &attrs,
                ink,
                LayoutStyle { width: Some(title_width), min_width: Some(0.0), ..LayoutStyle::leaf() },
            );
            cx.semantics(title, Semantics::hidden());
            parts.push(title);
        }
        if tab.unsaved {
            parts.push(cx.child(&Dot::new().tone(Tone::Accent).size(7.0).label(labels.unsaved.clone())));
        }
        if closes {
            let (close, id) = (strip.on_close.clone(), tab.id);
            let button = IconButton::new(IconName::X)
                .label(labels.close_tab.clone())
                .size(ButtonSize::Mini)
                .on_click(move || {
                    if let Some(close) = &close {
                        close(id);
                    }
                });
            parts.push(cx.child(&button));
        }
        if let (Some(color), Some(size)) = (tab.color, frame) {
            parts.push(cx.container(
                LayoutStyle {
                    absolute: Some((0.0, 0.0)),
                    width: Some(size.width),
                    height: Some(COLOR_BAND),
                    ..LayoutStyle::leaf()
                },
                &[],
                Paint::Quad(QuadPaint::new(color)),
            ));
        }
        if let (Some(gap), Some(size)) = (self.drag.get().and_then(|d| d.marked_gap()), frame) {
            if gap == index {
                parts.push(reorder::marker(cx, false, size.height, 0.0));
            } else if gap == index + 1 && gap == strip.tabs.len() {
                parts.push(reorder::marker(cx, false, size.height, size.width));
            }
        }
        // By its surface as well as its text: the open tab joins the page under it.
        let fill = if on {
            theme.color(Semantic::Surface)
        } else if hovered {
            theme.color(Semantic::Ink).with_alpha(0.05)
        } else {
            Color::TRANSPARENT
        };
        let radius = theme.radii.sm;
        let node = cx.container(
            LayoutStyle {
                height: Some(TAB_HEIGHT),
                width: Some(width),
                min_width: Some(width),
                spacing: gap,
                padding: Insets { top: 0.0, bottom: 0.0, left: theme.spacing.s3, right: right_pad },
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint { corner_radius: radius, ..QuadPaint::new(fill) }),
        );
        cx.report_frame(node);
        let (active, id) = (strip.active.clone(), tab.id);
        cx.on_click(node, Rc::new(move || active.set(id)));
        cx.cursor(node, CursorIcon::Pointer);
        cx.focusable(node, radius);
        cx.semantics(node, {
            let semantics = Semantics::new(Role::Tab).name(tab.title.clone()).selected(on);
            if tab.unsaved { semantics.description(labels.unsaved.clone()) } else { semantics }
        });
        let count = strip.tabs.len();
        let apply: Rc<dyn Fn(usize, usize)> = {
            let (handler, spoken, focus_to, words) =
                (strip.on_reorder.clone(), self.spoken.clone(), self.roving.focus_to.clone(), labels.clone());
            Rc::new(move |from, to| {
                if let Some(handler) = &handler {
                    handler(from, to);
                    spoken.set(words.moved_to_position(to + 1, count));
                    focus_to.request(to);
                }
            })
        };
        if strip.on_reorder.is_some() && !editing {
            cx.on_drag(
                node,
                reorder::drag_handler(
                    self.drag.clone(),
                    self.extents.clone(),
                    index,
                    0.0,
                    false,
                    apply.clone(),
                ),
            );
        }
        if strip.on_rename.is_some() {
            let starts = self.editing.clone();
            cx.on_double_click(node, Rc::new(move || starts.set(true)));
        }
        let (rename_key, close_key, called_off) =
            (self.editing.clone(), strip.on_close.clone().filter(|_| closes), self.drag.clone());
        let renames = strip.on_rename.is_some();
        let before: crate::KeyHandler = Rc::new(move |key, modifiers| {
            if *key == Key::Escape && called_off.peek().is_some() {
                called_off.set(None);
                return true;
            }
            if renames && is_f2(key) {
                rename_key.set(true);
                return true;
            }
            if *key == Key::Delete
                && let Some(close) = &close_key
            {
                close(id);
                return true;
            }
            if let Some(to) = reorder::key_target(key, modifiers, index, count, false) {
                apply(index, to);
            }
            reorder::is_move_key(key, modifiers)
        });
        let active = strip.active.clone();
        let ids: Rc<Vec<u64>> = Rc::new(strip.tabs.iter().map(|t| t.id).collect());
        let opens: Rc<dyn Fn(usize)> = Rc::new(move |to| {
            if let Some(id) = ids.get(to) {
                active.set(*id);
            }
        });
        self.roving.wire_with(cx, node, Some(opens), Some(before));
        node
    }
}

/// The tab, the state it keeps for itself and its menu.
struct TabWithMenu<'a> {
    strip: &'a DocumentTabs,
    index: usize,
    extents: Extents,
    drag: State<Option<Reorder>>,
    focus_to: FocusTarget,
    spoken: State<String>,
}

impl View for TabWithMenu<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        let strip = self.strip;
        let Some(tab) = strip.tabs.get(self.index) else { return cx.empty() };
        let editing: State<bool> = cx.use_state(|| false);
        let view = TabView {
            strip,
            tab,
            index: self.index,
            extents: self.extents.clone(),
            drag: self.drag.clone(),
            editing: editing.clone(),
            roving: Roving { index: self.index, count: strip.tabs.len(), focus_to: self.focus_to.clone() },
            spoken: self.spoken.clone(),
        };
        let mut menu = ContextMenu::new(view);
        let mut any_above = false;
        if let Some(items) = &strip.menu {
            for item in items(tab.id) {
                menu = menu.item(item);
                any_above = true;
            }
        }
        if any_above {
            menu = menu.separator();
        }
        if strip.on_rename.is_some() {
            menu = menu.item(MenuItem::new(labels.rename.clone()).on_select(move || editing.set(true)));
        }
        if let Some(close) = &strip.on_close {
            let id = tab.id;
            if tab.closable {
                let close_one = close.clone();
                menu = menu.item(MenuItem::new(labels.close_tab.clone()).on_select(move || close_one(id)));
            }
            let others: Vec<u64> =
                strip.tabs.iter().filter(|t| t.id != id && t.closable).map(|t| t.id).collect();
            if !others.is_empty() {
                let close_all = close.clone();
                menu = menu.item(MenuItem::new(labels.close_other_tabs.clone()).on_select(move || {
                    others.iter().for_each(|other| close_all(*other));
                }));
            }
            let right: Vec<u64> =
                strip.tabs.iter().skip(self.index + 1).filter(|t| t.closable).map(|t| t.id).collect();
            if !right.is_empty() {
                let close_all = close.clone();
                menu = menu.item(MenuItem::new(labels.close_tabs_to_right.clone()).on_select(move || {
                    right.iter().for_each(|other| close_all(*other));
                }));
            }
        }
        menu.build(cx)
    }
}

impl View for DocumentTabs {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let drag: State<Option<Reorder>> = cx.use_state(|| None);
        let extents: Extents = cx.use_state(|| Rc::new(RefCell::new(Vec::new()))).peek().clone();
        extents.borrow_mut().resize(self.tabs.len(), 0.0);
        let spoken: State<String> = cx.use_state(String::new);
        let focus_to = util::roving_cell(cx, None);
        let tabs: Vec<Keyed<TabWithMenu<'_>>> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                Keyed::new(
                    tab.id,
                    TabWithMenu {
                        strip: self,
                        index,
                        extents: extents.clone(),
                        drag: drag.clone(),
                        focus_to: focus_to.clone(),
                        spoken: spoken.clone(),
                    },
                )
            })
            .collect();
        let strip = cx.child(&ScrollEdgeArrow::new(tabs).spacing(2.0).fade(self.fade).height(TAB_HEIGHT));
        let strip = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[strip],
            Paint::None,
        );
        let mut nodes = vec![strip];
        if let Some(new) = self.on_new.clone() {
            nodes.push(
                cx.child(
                    &HStack::new((IconButton::new(IconName::Plus)
                        .label(labels.new_tab.clone())
                        .size(ButtonSize::Small)
                        .on_click(move || new()),))
                    .padding(Insets::symmetric(0.0, theme.spacing.s1)),
                ),
            );
        }
        let announcement = spoken.get();
        nodes.push(reorder::announcer(cx, announcement));
        let node = cx.container(
            LayoutStyle {
                min_width: Some(0.0),
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &nodes,
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.document_tabs_name.clone());
        cx.semantics(node, Semantics::new(Role::TabList).name(name));
        node
    }
}
