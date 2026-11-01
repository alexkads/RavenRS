//! Floating layers: tooltips, popovers, menus, selects and dialogs.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::{AnyView, Button, ButtonStyle, IconButton, IconName, View, util};

/// The floating panel surface shared by menus, selects and popovers.
fn panel_paint(theme: &Theme) -> Paint {
    Paint::Quad(QuadPaint {
        corner_radius: theme.radii.lg,
        border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
        shadow: Some(theme.elevation.raised),
        ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
    })
}

/// Wraps children in the floating panel surface. Clicks on its background stay
/// in the panel instead of reaching what is underneath.
pub(crate) fn panel(cx: &mut BuildCx<'_>, children: &[NodeId], padding: f32, min_width: f32) -> NodeId {
    let theme = *cx.theme();
    let style = LayoutStyle {
        padding: Insets::uniform(padding),
        min_width: Some(min_width),
        align: Align::Stretch,
        ..LayoutStyle::stack(Axis::Vertical)
    };
    let node = cx.container(style, children, panel_paint(&theme));
    cx.cursor(node, CursorIcon::Default);
    node
}

/// Shows a short label above its content while the pointer is over it.
pub struct Tooltip<V> {
    content: V,
    text: String,
}

impl<V> fmt::Debug for Tooltip<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tooltip").field("text", &self.text).finish_non_exhaustive()
    }
}

impl<V: View> Tooltip<V> {
    /// Shows `text` over `content` on hover.
    pub fn new(content: V, text: impl Into<String>) -> Self {
        Self { content, text: text.into() }
    }
}

struct Bubble<'a>(&'a str);

impl View for Bubble<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        // Inverted colors read as a tooltip in every theme.
        let text =
            util::label(cx, self.0, TextStyle::Callout, Some(Weight::MEDIUM), theme.color(Semantic::Canvas));
        let node = cx.container(
            LayoutStyle { padding: Insets::symmetric(5.0, 10.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[text],
            Paint::Quad(QuadPaint { corner_radius: 7.0, ..QuadPaint::new(theme.color(Semantic::Ink)) }),
        );
        // The text is already the name or description of the anchor.
        cx.semantics(node, Semantics::hidden());
        node
    }
}

impl<V: View> View for Tooltip<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        cx.label_control(child, &self.text, None);
        let node = cx.container(
            LayoutStyle { align_self: Some(Align::Start), ..LayoutStyle::stack(Axis::Horizontal) },
            &[child],
            Paint::None,
        );
        // Hoverable even when the content itself is not interactive.
        cx.cursor(node, CursorIcon::Default);
        if cx.is_hover_within() {
            cx.overlay(node, Placement::AboveCenter, &Bubble(&self.text), None);
        }
        node
    }
}

/// Floating content anchored below a view, open while `open` is true. Clicking
/// outside or pressing Escape closes it.
pub struct Popover<A, C> {
    open: Binding<bool>,
    anchor: A,
    content: C,
}

impl<A, C> fmt::Debug for Popover<A, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Popover").finish_non_exhaustive()
    }
}

impl<A: View, C: View> Popover<A, C> {
    /// Shows `content` below `anchor` while `open` is true.
    pub fn new(open: Binding<bool>, anchor: A, content: C) -> Self {
        Self { open, anchor, content }
    }
}

pub(crate) struct PanelOf<'a>(pub &'a dyn View);

impl View for PanelOf<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(self.0);
        panel(cx, &[child], 16.0, 220.0)
    }
}

impl<A: View, C: View> View for Popover<A, C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.child(&self.anchor);
        if self.open.get() {
            let close = self.open.clone();
            cx.overlay(
                anchor,
                Placement::Below,
                &PanelOf(&self.content),
                Some(Rc::new(move || close.set(false))),
            );
        }
        anchor
    }
}

/// One entry of a [`DropdownMenu`].
pub struct MenuItem {
    label: String,
    icon: Option<IconName>,
    shortcut: Option<String>,
    destructive: bool,
    on_select: Option<Rc<dyn Fn()>>,
}

impl fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MenuItem").field("label", &self.label).finish_non_exhaustive()
    }
}

impl MenuItem {
    /// A menu item labeled `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), icon: None, shortcut: None, destructive: false, on_select: None }
    }

    /// Shows an icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Shows a shortcut hint at the trailing edge, e.g. `"⌘C"` (display only).
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Paints the item in the danger color (Delete, Remove…).
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Runs `handler` when the item is chosen; the menu closes first.
    pub fn on_select(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

pub(crate) enum Entry {
    Item(MenuItem),
    Separator,
}

/// A button that opens a list of actions. The first item takes focus as the
/// menu opens; arrow keys move, Enter runs, Escape closes, and focus returns
/// to the button.
pub struct DropdownMenu {
    label: String,
    icon: Option<IconName>,
    style: ButtonStyle,
    size: crate::ButtonSize,
    entries: Vec<Entry>,
}

impl fmt::Debug for DropdownMenu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DropdownMenu").field("label", &self.label).finish_non_exhaustive()
    }
}

impl DropdownMenu {
    /// A secondary button labeled `label` with an empty menu.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            style: ButtonStyle::Secondary,
            size: crate::ButtonSize::Medium,
            entries: Vec::new(),
        }
    }

    /// The trigger's size (`Medium` by default).
    pub fn size(mut self, size: crate::ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Shows an icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Sets the trigger button style.
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Appends a menu item.
    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }

    /// Appends a separator line.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
}

/// A row in a menu or select list. `on_click` runs after `close`. Rows take
/// focus as the list opens, and arrow keys move between them.
struct ListRow<'a> {
    label: &'a str,
    icon: Option<IconName>,
    trailing: Option<&'a str>,
    checked: bool,
    destructive: bool,
    close: Rc<dyn Fn()>,
    on_click: Option<Rc<dyn Fn()>>,
    roving: util::Roving,
    /// `MenuItem` in menus, `ListBoxOption` in select lists.
    role: Role,
    /// Left (false) or Right (true) in a menubar menu: open the neighbor menu.
    on_side: Option<Rc<dyn Fn(bool)>>,
}

impl View for ListRow<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let hovered = cx.is_hovered() || cx.is_focus_visible();
        let ink = if self.destructive { theme.color(Semantic::Danger) } else { theme.color(Semantic::Ink) };
        let muted = if self.destructive { ink } else { theme.color(Semantic::InkMuted) };
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, 16.0, muted));
        }
        let weight = self.checked.then_some(Weight::SEMIBOLD);
        let label = util::label(cx, self.label, TextStyle::Body, weight, ink);
        children.push(cx.container(
            LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[label],
            Paint::None,
        ));
        if let Some(t) = self.trailing {
            children.push(util::label(cx, t, TextStyle::Callout, None, theme.color(Semantic::InkMuted)));
        }
        if self.checked {
            children.push(util::icon(
                cx,
                IconName::Check,
                16.0,
                theme.color(Semantic::Accent).lerp(ink, 0.3),
            ));
        }
        let fill = if hovered { theme.color(Semantic::Ink).with_alpha(0.07) } else { Color::TRANSPARENT };
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, 10.0),
            height: Some(34.0),
            spacing: 10.0,
            align: Align::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let node = cx.container(
            style,
            &children,
            Paint::Quad(QuadPaint { corner_radius: 7.0, ..QuadPaint::new(fill) }),
        );
        let mut semantics = Semantics::new(self.role);
        if self.role == Role::ListBoxOption {
            semantics = semantics.selected(self.checked);
        }
        if let Some(t) = self.trailing {
            semantics = semantics.name(self.label).description(t);
        }
        cx.semantics(node, semantics);
        let (action, close) = (self.on_click.clone(), self.close.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                close();
                if let Some(a) = &action {
                    a();
                }
            }),
        );
        cx.focusable(node, 7.0);
        let before: Option<crate::KeyHandler> = self.on_side.clone().map(|side| {
            Rc::new(move |key: &ravenrs_core::Key, _: ravenrs_core::Modifiers| match key {
                ravenrs_core::Key::ArrowLeft => {
                    side(false);
                    true
                }
                ravenrs_core::Key::ArrowRight => {
                    side(true);
                    true
                }
                _ => false,
            }) as crate::KeyHandler
        });
        self.roving.wire_with(cx, node, None, before);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// The shared roving focus of a list that focuses row `start` when it opens.
fn list_focus(cx: &mut BuildCx<'_>, start: usize) -> util::FocusTarget {
    util::roving_cell(cx, Some(start))
}

pub(crate) struct MenuList<'a> {
    pub entries: &'a [Entry],
    pub close: Rc<dyn Fn()>,
    /// Set by a menubar: Left and Right open the neighbor menu.
    pub on_side: Option<Rc<dyn Fn(bool)>>,
}

impl View for MenuList<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let mut rows = Vec::new();
        let count = self.entries.iter().filter(|e| matches!(e, Entry::Item(_))).count();
        let focus_to = list_focus(cx, 0);
        let mut item_index = 0;
        for (i, entry) in self.entries.iter().enumerate() {
            match entry {
                Entry::Item(item) => {
                    let roving = util::Roving { index: item_index, count, focus_to: focus_to.clone() };
                    item_index += 1;
                    let row = ListRow {
                        roving,
                        label: &item.label,
                        icon: item.icon,
                        trailing: item.shortcut.as_deref(),
                        checked: false,
                        destructive: item.destructive,
                        close: self.close.clone(),
                        on_click: item.on_select.clone(),
                        role: Role::MenuItem,
                        on_side: self.on_side.clone(),
                    };
                    rows.push(cx.keyed_child(i as u64, &row));
                }
                Entry::Separator => {
                    let line = cx.child(&crate::Divider::horizontal());
                    rows.push(cx.container(
                        LayoutStyle {
                            padding: Insets::symmetric(4.0, 4.0),
                            align: Align::Stretch,
                            ..LayoutStyle::stack(Axis::Vertical)
                        },
                        &[line],
                        Paint::None,
                    ));
                }
            }
        }
        let node = panel(cx, &rows, 6.0, 220.0);
        cx.semantics(node, Semantics::new(Role::Menu));
        node
    }
}

impl View for DropdownMenu {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let open: State<bool> = cx.use_state(|| false);
        let toggle = open.clone();
        let mut trigger = Button::new(self.label.clone())
            .style(self.style)
            .size(self.size)
            .trailing_icon(IconName::ChevronDown)
            .on_click(move || toggle.update(|o| *o = !*o));
        if let Some(icon) = self.icon {
            trigger = trigger.icon(icon);
        }
        let anchor = cx.child(&trigger);
        let is_open = open.get();
        cx.update_semantics(anchor, |s| s.expanded = Some(is_open));
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if is_open {
            was_open.set(true);
            let close: Rc<dyn Fn()> = {
                let open = open.clone();
                Rc::new(move || open.set(false))
            };
            cx.overlay(
                anchor,
                Placement::Below,
                &MenuList { entries: &self.entries, close: close.clone(), on_side: None },
                Some(close),
            );
        } else if was_open.replace(false) {
            // Focus returns to the trigger when the menu closes.
            cx.request_focus_node(anchor);
        }
        anchor
    }
}

/// Picks one option from a list, shown in a field-like trigger. Enter, Space or
/// an arrow key opens the list with the current option focused; arrows move,
/// Enter picks, Escape closes, and focus returns to the field.
#[derive(Debug)]
pub struct Select {
    selected: Binding<usize>,
    options: Vec<String>,
    width: f32,
    label: Option<String>,
}

impl Select {
    /// A select over `options`; `selected` holds the zero-based index of the choice.
    pub fn new(selected: Binding<usize>, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { selected, options: options.into_iter().map(Into::into).collect(), width: 220.0, label: None }
    }

    /// Sets the width in logical pixels (220 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The name screen readers announce, such as "Country". Inside a
    /// [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

struct Options<'a> {
    options: &'a [String],
    selected: Binding<usize>,
    close: Rc<dyn Fn()>,
    width: f32,
}

impl View for Options<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let current = self.selected.get();
        let (count, focus_to) = (self.options.len(), list_focus(cx, current));
        let rows: Vec<NodeId> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let selected = self.selected.clone();
                let row = ListRow {
                    roving: util::Roving { index: i, count, focus_to: focus_to.clone() },
                    label: option,
                    icon: None,
                    trailing: None,
                    checked: i == current,
                    destructive: false,
                    close: self.close.clone(),
                    on_click: Some(Rc::new(move || selected.set(i))),
                    role: Role::ListBoxOption,
                    on_side: None,
                };
                cx.keyed_child(i as u64, &row)
            })
            .collect();
        let node = panel(cx, &rows, 6.0, self.width);
        cx.semantics(node, Semantics::new(Role::ListBox));
        node
    }
}

impl View for Select {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<bool> = cx.use_state(|| false);
        let is_open = open.get();
        let hovered = cx.is_hovered();
        let current = self.options.get(self.selected.get()).map_or("", String::as_str);
        let label = util::label(cx, current, TextStyle::Body, None, theme.color(Semantic::Ink));
        let label = cx.container(
            LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[label],
            Paint::None,
        );
        let chevron = util::icon(
            cx,
            if is_open { IconName::ChevronUp } else { IconName::ChevronDown },
            16.0,
            theme.color(Semantic::InkMuted),
        );
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base },
            }
        };
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, 12.0),
            width: Some(self.width),
            height: Some(40.0),
            spacing: 8.0,
            align: Align::Center,
            align_self: Some(Align::Start),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(border),
            ..QuadPaint::new(theme.color(Semantic::Control))
        };
        let node = cx.container(style, &[label, chevron], Paint::Quad(paint));
        cx.semantics(label, Semantics::hidden());
        let mut semantics = Semantics::new(Role::ComboBox).value(current).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
        let toggle = open.clone();
        cx.on_click(node, Rc::new(move || toggle.update(|o| *o = !*o)));
        // Arrow keys open the list too, like a native select.
        let opener = open.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let opens = matches!(key, ravenrs_core::Key::ArrowDown | ravenrs_core::Key::ArrowUp);
                if opens {
                    opener.set(true);
                }
                opens
            }),
        );
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Pointer);
        let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
        if !is_open && was_open.replace(false) {
            // Focus returns to the field when the list closes.
            cx.request_focus();
        }
        if is_open {
            was_open.set(true);
            let close: Rc<dyn Fn()> = {
                let open = open.clone();
                Rc::new(move || open.set(false))
            };
            let list = Options {
                options: &self.options,
                selected: self.selected.clone(),
                close: close.clone(),
                width: self.width,
            };
            cx.overlay(node, Placement::Below, &list, Some(close));
        }
        node
    }
}

/// A modal dialog over a dimmed backdrop, shown while `open` is true.
/// Clicking the backdrop, the close button or pressing Escape closes it.
pub struct Dialog {
    open: Binding<bool>,
    title: String,
    description: Option<String>,
    content: Option<AnyView>,
    actions: Vec<AnyView>,
    width: f32,
    close_label: String,
}

impl fmt::Debug for Dialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dialog").field("title", &self.title).finish_non_exhaustive()
    }
}

impl Dialog {
    /// A dialog shown while `open` is true.
    pub fn new(open: Binding<bool>, title: impl Into<String>) -> Self {
        Self {
            open,
            title: title.into(),
            description: None,
            content: None,
            actions: Vec::new(),
            width: 460.0,
            close_label: "Close".into(),
        }
    }

    /// Adds a muted description under the title.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the dialog body.
    pub fn content(mut self, content: impl View + 'static) -> Self {
        self.content = Some(Box::new(content));
        self
    }

    /// The widest the dialog gets, in logical pixels (460 by default). It always
    /// fits the window.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The close button's name for screen readers ("Close" by default).
    pub fn close_label(mut self, label: impl Into<String>) -> Self {
        self.close_label = label.into();
        self
    }

    /// Buttons at the bottom-right, in order.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.actions.push(Box::new(action));
        self
    }
}

struct DialogLayer<'a> {
    dialog: &'a Dialog,
}

impl View for DialogLayer<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let viewport = cx.viewport();
        let d = self.dialog;
        let close = d.open.clone();
        let title = util::label(cx, &d.title, TextStyle::Title, None, theme.color(Semantic::Ink));
        let title = cx.container(
            LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[title],
            Paint::None,
        );
        let x = cx.child(
            &IconButton::new(IconName::X)
                .label(d.close_label.clone())
                .size(crate::ButtonSize::Small)
                .on_click(move || close.set(false)),
        );
        cx.semantics(title, Semantics::new(Role::Heading).level(2));
        let header = cx.container(
            LayoutStyle { align: Align::Center, spacing: 12.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[title, x],
            Paint::None,
        );
        let mut body = vec![header];
        if let Some(text) = &d.description {
            body.push(util::label(cx, text, TextStyle::Body, None, theme.color(Semantic::InkMuted)));
        }
        if let Some(content) = &d.content {
            body.push(cx.child(content.as_ref()));
        }
        if !d.actions.is_empty() {
            let actions: Vec<NodeId> = d.actions.iter().map(|a| cx.child(a.as_ref())).collect();
            body.push(cx.container(
                LayoutStyle {
                    spacing: 10.0,
                    justify: Justify::End,
                    padding: Insets { top: 8.0, ..Insets::ZERO },
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &actions,
                Paint::None,
            ));
        }
        let card = cx.container(
            LayoutStyle {
                width: Some((viewport.width - 32.0).clamp(240.0, d.width.max(240.0))),
                padding: Insets::uniform(24.0),
                spacing: 14.0,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &body,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.xl,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        let mut semantics = Semantics::new(Role::Dialog).name(d.title.clone()).modal();
        if let Some(text) = &d.description {
            semantics = semantics.description(text.clone());
        }
        cx.semantics(card, semantics);
        // The card swallows clicks so only the backdrop closes the dialog.
        cx.cursor(card, CursorIcon::Default);
        let backdrop = cx.container(
            LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[card],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Scrim))),
        );
        let close = d.open.clone();
        cx.on_click(backdrop, Rc::new(move || close.set(false)));
        backdrop
    }
}

impl View for Dialog {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        if self.open.get() {
            let close = self.open.clone();
            cx.overlay(
                anchor,
                Placement::Fill,
                &DialogLayer { dialog: self },
                Some(Rc::new(move || close.set(false))),
            );
        }
        anchor
    }
}
