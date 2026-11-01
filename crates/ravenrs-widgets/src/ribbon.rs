//! An Office-style ribbon, after the DevExpress Ribbon control: application
//! menu, Quick Access Toolbar, pages (tabs) with groups of large and small
//! commands, contextual page categories, search, and a collapsed mode.

use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Quad, Rect};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint};
use crate::overlay::{Entry, MenuList, panel};
use crate::{IconName, MenuItem, TextField, Tone, Tooltip, View, util};

/// How much room a ribbon command takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RibbonSize {
    /// A tall button: large icon above a (wrapping) label.
    #[default]
    Large,
    /// A row with a small icon and a label; three stack in a column.
    Small,
    /// A small icon only (with a tooltip); three stack in a column.
    IconOnly,
}

/// A command in a ribbon group: a button, a toggle, or a button with a menu.
pub struct RibbonItem {
    label: String,
    icon: Option<IconName>,
    size: RibbonSize,
    action: Option<Rc<dyn Fn()>>,
    toggle: Option<Binding<bool>>,
    menu: Vec<Entry>,
    enabled: bool,
    tooltip: Option<String>,
}

impl fmt::Debug for RibbonItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RibbonItem")
            .field("label", &self.label)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl RibbonItem {
    /// A command button (large by default).
    pub fn button(label: impl Into<String>, icon: IconName) -> Self {
        Self {
            label: label.into(),
            icon: Some(icon),
            size: RibbonSize::Large,
            action: None,
            toggle: None,
            menu: Vec::new(),
            enabled: true,
            tooltip: None,
        }
    }

    /// A button that stays pressed while `value` is true (Bold, Ruler…).
    pub fn toggle(label: impl Into<String>, icon: IconName, value: Binding<bool>) -> Self {
        Self { toggle: Some(value), size: RibbonSize::Small, ..Self::button(label, icon) }
    }

    pub fn large(mut self) -> Self {
        self.size = RibbonSize::Large;
        self
    }

    pub fn small(mut self) -> Self {
        self.size = RibbonSize::Small;
        self
    }

    pub fn icon_only(mut self) -> Self {
        self.size = RibbonSize::IconOnly;
        self
    }

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.action = Some(Rc::new(handler));
        self
    }

    /// Adds a drop-down item; a button with items opens a menu.
    pub fn menu_item(mut self, item: MenuItem) -> Self {
        self.menu.push(Entry::Item(item));
        self
    }

    pub fn menu_separator(mut self) -> Self {
        self.menu.push(Entry::Separator);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Hover text (defaults to the label for icon-only items).
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

/// A labeled set of commands inside a page.
pub struct RibbonGroup {
    title: String,
    items: Vec<RibbonItem>,
    launcher: Option<Rc<dyn Fn()>>,
}

impl fmt::Debug for RibbonGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RibbonGroup")
            .field("title", &self.title)
            .field("items", &self.items)
            .finish_non_exhaustive()
    }
}

impl RibbonGroup {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), items: Vec::new(), launcher: None }
    }

    pub fn item(mut self, item: RibbonItem) -> Self {
        self.items.push(item);
        self
    }

    /// The small arrow in the group caption that opens the full dialog.
    pub fn dialog_launcher(mut self, handler: impl Fn() + 'static) -> Self {
        self.launcher = Some(Rc::new(handler));
        self
    }
}

/// A tab of the ribbon.
#[derive(Debug)]
pub struct RibbonPage {
    title: String,
    groups: Vec<RibbonGroup>,
    /// Contextual pages belong to a colored category and appear on demand.
    category: Option<(String, Tone)>,
    visible: bool,
}

impl RibbonPage {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), groups: Vec::new(), category: None, visible: true }
    }

    pub fn group(mut self, group: RibbonGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Makes this a contextual page (e.g. "Table Tools") shown only while `visible`.
    pub fn contextual(mut self, category: impl Into<String>, tone: Tone, visible: bool) -> Self {
        self.category = Some((category.into(), tone));
        self.visible = visible;
        self
    }
}

/// The ribbon.
pub struct Ribbon {
    pages: Vec<RibbonPage>,
    app_menu: Option<(String, Vec<Entry>)>,
    quick_access: Vec<RibbonItem>,
    search: Option<Binding<String>>,
    selected: Option<Binding<usize>>,
}

impl fmt::Debug for Ribbon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ribbon").field("pages", &self.pages).finish_non_exhaustive()
    }
}

impl Default for Ribbon {
    fn default() -> Self {
        Self::new()
    }
}

impl Ribbon {
    pub fn new() -> Self {
        Self { pages: Vec::new(), app_menu: None, quick_access: Vec::new(), search: None, selected: None }
    }

    pub fn page(mut self, page: RibbonPage) -> Self {
        self.pages.push(page);
        self
    }

    /// The accent "File" button and its menu.
    pub fn app_menu(mut self, label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        self.app_menu = Some((label.into(), items.into_iter().map(Entry::Item).collect()));
        self
    }

    /// Icon commands always visible at the top-left (Save, Undo, Redo…).
    pub fn quick_access(mut self, item: RibbonItem) -> Self {
        self.quick_access.push(item.icon_only());
        self
    }

    /// A "Tell me what you want to do" search box.
    pub fn search(mut self, query: Binding<String>) -> Self {
        self.search = Some(query);
        self
    }

    /// Where the selected page index lives (otherwise the ribbon keeps it).
    pub fn selected_page(mut self, page: Binding<usize>) -> Self {
        self.selected = Some(page);
        self
    }
}

/// Large button height area (icon + two lines of label).
const BODY_HEIGHT: f32 = 92.0;
const CAPTION_HEIGHT: f32 = 22.0;

fn tone_color(theme: &Theme, tone: Tone) -> Color {
    match tone {
        Tone::Neutral => theme.color(Semantic::InkMuted),
        Tone::Accent => theme.color(Semantic::Accent),
        Tone::Success => theme.color(Semantic::Success),
        Tone::Warning => theme.color(Semantic::Warning),
        Tone::Danger => theme.color(Semantic::Danger),
    }
}

struct ItemView<'a>(&'a RibbonItem);

impl View for ItemView<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let item = self.0;
        let open: State<bool> = cx.use_state(|| false);
        let on = item.toggle.as_ref().is_some_and(Binding::get);
        let hovered = cx.is_hovered() && item.enabled;
        let pressed = cx.is_pressed() && item.enabled;
        let ink = theme.color(Semantic::Ink);
        let color = if item.enabled { ink } else { theme.color(Semantic::InkMuted).with_alpha(0.5) };
        let icon_color = if !item.enabled {
            color
        } else if on {
            theme.color(Semantic::Accent).lerp(ink, 0.2)
        } else {
            theme.color(Semantic::Accent).lerp(ink, 0.45)
        };
        let has_menu = !item.menu.is_empty();
        let mut children = Vec::new();
        let node_style = match item.size {
            RibbonSize::Large => {
                if let Some(i) = item.icon {
                    children.push(util::icon(cx, i, 28.0, icon_color));
                }
                let mut a = util::attrs(&theme, TextStyle::Caption, None);
                a.size = 12.0;
                a.line_height = 15.0;
                a.letter_spacing = 0.0;
                let label = cx.text(
                    &item.label,
                    &a,
                    color,
                    LayoutStyle { max_width: Some(72.0), ..LayoutStyle::leaf() },
                );
                children.push(label);
                if has_menu {
                    children.push(util::icon(
                        cx,
                        IconName::ChevronDown,
                        12.0,
                        theme.color(Semantic::InkMuted),
                    ));
                }
                LayoutStyle {
                    height: Some(BODY_HEIGHT - 6.0),
                    min_width: Some(52.0),
                    padding: Insets { top: 8.0, bottom: 4.0, left: 6.0, right: 6.0 },
                    spacing: 4.0,
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Vertical)
                }
            }
            RibbonSize::Small | RibbonSize::IconOnly => {
                if let Some(i) = item.icon {
                    children.push(util::icon(cx, i, 16.0, icon_color));
                }
                if item.size == RibbonSize::Small {
                    let mut a = util::attrs(&theme, TextStyle::Callout, None);
                    a.size = 13.0;
                    children.push(cx.text(&item.label, &a, color, LayoutStyle::leaf()));
                }
                if has_menu {
                    children.push(util::icon(
                        cx,
                        IconName::ChevronDown,
                        12.0,
                        theme.color(Semantic::InkMuted),
                    ));
                }
                let icon_only = item.size == RibbonSize::IconOnly;
                LayoutStyle {
                    height: Some(26.0),
                    width: (icon_only && !has_menu).then_some(28.0),
                    padding: Insets::symmetric(0.0, 6.0),
                    spacing: 6.0,
                    align: Align::Center,
                    justify: if icon_only { Justify::Center } else { Justify::Start },
                    ..LayoutStyle::stack(Axis::Horizontal)
                }
            }
        };
        let fill = if on || open.get() {
            theme.color(Semantic::Accent).with_alpha(if pressed { 0.28 } else { 0.18 })
        } else if pressed {
            ink.with_alpha(0.12)
        } else if hovered {
            ink.with_alpha(0.07)
        } else {
            Color::TRANSPARENT
        };
        let border = (on || open.get())
            .then(|| Border { width: 1.0, color: theme.color(Semantic::Accent).with_alpha(0.45) });
        let node = cx.container(
            node_style,
            &children,
            Paint::Quad(QuadPaint { corner_radius: 6.0, border, ..QuadPaint::new(fill) }),
        );
        if item.enabled {
            let (action, toggle, menu_open) = (item.action.clone(), item.toggle.clone(), open.clone());
            cx.on_click(
                node,
                Rc::new(move || {
                    if has_menu {
                        menu_open.update(|o| *o = !*o);
                        return;
                    }
                    if let Some(t) = &toggle {
                        t.update(|v| *v = !*v);
                    }
                    if let Some(a) = &action {
                        a();
                    }
                }),
            );
            cx.focusable(node, 6.0);
            cx.cursor(node, CursorIcon::Pointer);
        }
        if open.get() {
            let close: Rc<dyn Fn()> = {
                let open = open.clone();
                Rc::new(move || open.set(false))
            };
            cx.overlay(
                node,
                Placement::Below,
                &MenuList { entries: &item.menu, close: close.clone() },
                Some(close),
            );
        }
        node
    }
}

/// An item, wrapped in a tooltip when it has one (icon-only items always do).
struct Command<'a>(&'a RibbonItem);

impl View for Command<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let tip = self
            .0
            .tooltip
            .clone()
            .or_else(|| (self.0.size == RibbonSize::IconOnly).then(|| self.0.label.clone()));
        match tip {
            Some(t) if !t.is_empty() => cx.child(&Tooltip::new(ItemView(self.0), t)),
            _ => cx.child(&ItemView(self.0)),
        }
    }
}

struct GroupView<'a>(&'a RibbonGroup);

impl View for GroupView<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let g = self.0;
        // Large items stand alone; runs of small items stack three to a column.
        let mut columns = Vec::new();
        let mut stack: Vec<NodeId> = Vec::new();
        let flush = |cx: &mut BuildCx<'_>, stack: &mut Vec<NodeId>, columns: &mut Vec<NodeId>| {
            if !stack.is_empty() {
                columns.push(cx.container(
                    LayoutStyle {
                        spacing: 2.0,
                        justify: Justify::Start,
                        ..LayoutStyle::stack(Axis::Vertical)
                    },
                    stack,
                    Paint::None,
                ));
                stack.clear();
            }
        };
        for (i, item) in g.items.iter().enumerate() {
            let node = cx.keyed_child(i as u64, &Command(item));
            if item.size == RibbonSize::Large {
                flush(cx, &mut stack, &mut columns);
                columns.push(node);
            } else {
                stack.push(node);
                if stack.len() == 3 {
                    flush(cx, &mut stack, &mut columns);
                }
            }
        }
        flush(cx, &mut stack, &mut columns);
        let body = cx.container(
            LayoutStyle {
                height: Some(BODY_HEIGHT),
                padding: Insets::symmetric(3.0, 4.0),
                spacing: 2.0,
                align: Align::Start,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &columns,
            Paint::None,
        );
        let muted = theme.color(Semantic::InkMuted);
        let mut caption = vec![util::label(cx, &g.title, TextStyle::Caption, None, muted)];
        if let Some(l) = &g.launcher {
            let arrow = util::icon(cx, IconName::ArrowDown, 11.0, muted);
            let b = cx.container(
                LayoutStyle {
                    width: Some(18.0),
                    height: Some(18.0),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[arrow],
                Paint::None,
            );
            cx.on_click(b, l.clone());
            cx.cursor(b, CursorIcon::Pointer);
            caption.push(b);
        }
        let caption = cx.container(
            LayoutStyle {
                height: Some(CAPTION_HEIGHT),
                spacing: 4.0,
                align: Align::Center,
                justify: Justify::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &caption,
            Paint::None,
        );
        let line = theme.color(Semantic::Border);
        cx.container(
            LayoutStyle {
                padding: Insets { right: 6.0, left: 2.0, ..Insets::ZERO },
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[body, caption],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(
                    Rect::new(f.max_x() - 1.0, f.y() + 8.0, 1.0, f.height() - 16.0),
                    line,
                ));
            })),
        )
    }
}

struct PageBody<'a>(&'a RibbonPage);

impl View for PageBody<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let groups: Vec<NodeId> =
            self.0.groups.iter().enumerate().map(|(i, g)| cx.keyed_child(i as u64, &GroupView(g))).collect();
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(4.0, 8.0),
                spacing: 6.0,
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &groups,
            Paint::None,
        );
        cx.clip(node);
        node
    }
}

/// The page body shown as a popup while the ribbon is collapsed.
struct BodyPopup<'a>(&'a RibbonPage, f32);

impl View for BodyPopup<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let body = cx.child(&PageBody(self.0));
        panel(cx, &[body], 0.0, self.1)
    }
}

struct Tab<'a> {
    page: &'a RibbonPage,
    index: usize,
    selected: Binding<usize>,
    collapsed: State<bool>,
    popup: State<bool>,
}

impl View for Tab<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.selected.get() == self.index;
        let hovered = cx.is_hovered();
        let ink = theme.color(Semantic::Ink);
        let tone = self.page.category.as_ref().map(|(_, t)| tone_color(&theme, *t));
        let color = match (on, tone) {
            (true, Some(t)) => t.lerp(ink, 0.25),
            (true, None) => ink,
            _ if hovered => ink,
            _ => theme.color(Semantic::InkMuted),
        };
        let label = util::label(cx, &self.page.title, TextStyle::Callout, Some(Weight::SEMIBOLD), color);
        let accent = tone.unwrap_or(theme.color(Semantic::Accent));
        let category_fill = tone.map(|t| t.with_alpha(0.10));
        let node = cx.container(
            LayoutStyle {
                height: Some(34.0),
                padding: Insets::symmetric(0.0, 12.0),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[label],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                if let Some(fill) = category_fill {
                    scene.push_quad(Quad::new(f, fill));
                }
                if on {
                    scene.push_quad(
                        Quad::new(Rect::new(f.x() + 8.0, f.max_y() - 2.0, f.width() - 16.0, 2.0), accent)
                            .radius(1.0),
                    );
                }
            })),
        );
        let (sel, i, collapsed, popup) =
            (self.selected.clone(), self.index, self.collapsed.clone(), self.popup.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                let same = sel.get() == i;
                sel.set(i);
                if collapsed.get() {
                    popup.set(!(same && popup.get()));
                }
            }),
        );
        let (collapsed, popup) = (self.collapsed.clone(), self.popup.clone());
        cx.on_double_click(
            node,
            Rc::new(move || {
                popup.set(false);
                collapsed.update(|c| *c = !*c);
            }),
        );
        cx.focusable(node, 6.0);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl View for Ribbon {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let own: State<usize> = cx.use_state(|| 0);
        let selected = self.selected.clone().unwrap_or_else(|| own.binding());
        let collapsed: State<bool> = cx.use_state(|| false);
        let popup: State<bool> = cx.use_state(|| false);
        let app_open: State<bool> = cx.use_state(|| false);
        let visible: Vec<usize> = (0..self.pages.len()).filter(|&i| self.pages[i].visible).collect();
        let current = {
            let s = selected.get();
            if visible.contains(&s) { s } else { visible.first().copied().unwrap_or(0) }
        };

        // Title row: application menu, quick access, tabs, search, collapse.
        let mut row = Vec::new();
        if let Some((label, entries)) = &self.app_menu {
            let on_accent = theme.color(Semantic::OnAccent);
            let text = util::label(cx, label, TextStyle::Callout, Some(Weight::SEMIBOLD), on_accent);
            let hovered = cx.is_hovered();
            let fill = theme.color(if hovered { Semantic::AccentHover } else { Semantic::Accent });
            let app = cx.container(
                LayoutStyle {
                    height: Some(30.0),
                    padding: Insets::symmetric(0.0, 14.0),
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[text],
                Paint::Quad(QuadPaint { corner_radius: 6.0, ..QuadPaint::new(fill) }),
            );
            let o = app_open.clone();
            cx.on_click(app, Rc::new(move || o.update(|v| *v = !*v)));
            cx.focusable(app, 6.0);
            cx.cursor(app, CursorIcon::Pointer);
            if app_open.get() {
                let close: Rc<dyn Fn()> = {
                    let o = app_open.clone();
                    Rc::new(move || o.set(false))
                };
                cx.overlay(app, Placement::Below, &MenuList { entries, close: close.clone() }, Some(close));
            }
            row.push(app);
        }
        if !self.quick_access.is_empty() {
            let items: Vec<NodeId> = self
                .quick_access
                .iter()
                .enumerate()
                .map(|(i, q)| cx.keyed_child(500 + i as u64, &Command(q)))
                .collect();
            row.push(cx.container(
                LayoutStyle {
                    spacing: 1.0,
                    padding: Insets::symmetric(0.0, 6.0),
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &items,
                Paint::None,
            ));
        }
        // Contextual categories get a caption above their tabs.
        let mut tabs = Vec::new();
        for &i in &visible {
            let page = &self.pages[i];
            let tab = cx.keyed_child(
                1000 + i as u64,
                &Tab {
                    page,
                    index: i,
                    selected: selected.clone(),
                    collapsed: collapsed.clone(),
                    popup: popup.clone(),
                },
            );
            if let Some((name, tone)) = &page.category {
                let color = tone_color(&theme, *tone);
                let cap = util::label(cx, &name.to_uppercase(), TextStyle::Caption, None, color);
                tabs.push(cx.container(
                    LayoutStyle { spacing: 0.0, align: Align::Center, ..LayoutStyle::stack(Axis::Vertical) },
                    &[cap, tab],
                    Paint::None,
                ));
            } else {
                tabs.push(tab);
            }
        }
        row.push(cx.container(
            LayoutStyle {
                spacing: 2.0,
                align: Align::End,
                grow: 1.0,
                min_width: Some(0.0),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &tabs,
            Paint::None,
        ));
        if let Some(q) = &self.search {
            row.push(
                cx.child(
                    &TextField::new(q.clone())
                        .placeholder("Tell me what you want to do")
                        .icon(IconName::Search)
                        .compact()
                        .width(240.0),
                ),
            );
        }
        let is_collapsed = collapsed.get();
        let (c, p) = (collapsed.clone(), popup.clone());
        row.push(
            cx.child(&Tooltip::new(
                crate::IconButton::new(if is_collapsed {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronUp
                })
                .size(crate::ButtonSize::Small)
                .on_click(move || {
                    p.set(false);
                    c.update(|v| *v = !*v);
                }),
                if is_collapsed { "Pin the ribbon" } else { "Collapse the ribbon" },
            )),
        );
        let title_row = cx.container(
            LayoutStyle {
                padding: Insets { top: 6.0, bottom: 0.0, left: 8.0, right: 8.0 },
                spacing: 8.0,
                align: Align::End,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &row,
            Paint::None,
        );
        let line = theme.color(Semantic::Border);
        let mut sections = vec![title_row];
        if let Some(page) = self.pages.get(current) {
            if is_collapsed {
                if popup.get() {
                    let close: Rc<dyn Fn()> = {
                        let p = popup.clone();
                        Rc::new(move || p.set(false))
                    };
                    let width = cx.viewport().width - 32.0;
                    cx.overlay(title_row, Placement::Below, &BodyPopup(page, width.min(1100.0)), Some(close));
                }
            } else {
                sections.push(cx.keyed_child(2000 + current as u64, &PageBody(page)));
            }
        }
        let surface = theme.color(Semantic::Surface);
        cx.container(
            LayoutStyle {
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &sections,
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                scene.push_quad(Quad::new(f, surface));
                scene.push_quad(Quad::new(Rect::new(f.x(), f.max_y() - 1.0, f.width(), 1.0), line));
                // Hairline under the tab strip.
                scene.push_quad(Quad::new(
                    Rect::new(f.x(), f.y() + 40.0, f.width(), 1.0),
                    line.with_alpha(0.6),
                ));
            })),
        )
    }
}
