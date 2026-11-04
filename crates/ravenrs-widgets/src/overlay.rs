//! Floating layers: tooltips, popovers, menus, selects and dialogs.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Effect, Insets, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::{AnyView, Button, ButtonStyle, IconButton, IconName, Motion, View, motion, util};

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
    motion::popup(cx, node);
    node
}

/// Shows a short label above its content while the pointer is over it, with the key
/// that does the same when there is one.
///
/// ```
/// use ravenrs_widgets::{Button, Tooltip, Ui, ViewExt};
///
/// let save = Tooltip::new(Button::new("Save"), "Save the file").shortcut("⌘S");
/// let open = Button::new("Open").tooltip("Open a file");
/// let mut ui = Ui::new(ravenrs_widgets::VStack::new((save, open)), ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct Tooltip<V> {
    content: V,
    text: String,
    shortcut: Option<String>,
}

impl<V> fmt::Debug for Tooltip<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tooltip").field("text", &self.text).finish_non_exhaustive()
    }
}

impl<V: View> Tooltip<V> {
    /// Shows `text` over `content` on hover.
    pub fn new(content: V, text: impl Into<String>) -> Self {
        Self { content, text: text.into(), shortcut: None }
    }

    /// Shows this key as a key cap beside the text, such as `"⌘S"`.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }
}

pub(crate) struct Bubble<'a>(pub &'a str);

/// The bubble of a tooltip that also shows a key.
struct KeyBubble<'a> {
    text: &'a str,
    keys: Option<&'a str>,
}

impl View for Bubble<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        KeyBubble { text: self.0, keys: None }.build(cx)
    }
}

impl View for KeyBubble<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        // Inverted colors read as a tooltip in every theme.
        let ink = theme.color(Semantic::Canvas);
        let mut children = vec![util::label(cx, self.text, TextStyle::Callout, Some(Weight::MEDIUM), ink)];
        if let Some(keys) = self.keys {
            let cap = crate::Kbd::new(keys.to_owned()).inherit_color();
            children.push(cx.with_foreground(ink, |cx| cx.child(&cap)));
        }
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(5.0, 10.0),
                spacing: theme.spacing.s2,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint { corner_radius: 7.0, ..QuadPaint::new(theme.color(Semantic::Ink)) }),
        );
        // The text is already the name or description of the anchor.
        cx.semantics(node, Semantics::hidden());
        motion::popup(cx, node);
        node
    }
}

impl<V: View> View for Tooltip<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        cx.label_control(child, &self.text, self.shortcut.as_deref());
        let node = cx.container(
            LayoutStyle { align_self: Some(Align::Fit), ..LayoutStyle::stack(Axis::Horizontal) },
            &[child],
            Paint::None,
        );
        // Hoverable even when the content itself is not interactive.
        cx.cursor(node, CursorIcon::Default);
        if cx.is_hover_within() {
            let bubble = KeyBubble { text: &self.text, keys: self.shortcut.as_deref() };
            cx.overlay(node, Placement::AboveCenter, &bubble, None);
        }
        node
    }
}

/// What a [`HelpTip`] opens: what something is and, if it helps, how to use it.
struct HelpPanel<'a> {
    title: &'a str,
    text: &'a str,
    how: Option<&'a str>,
}

impl View for HelpPanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let wrapped = |cx: &mut BuildCx<'_>, text: &str, weight, color| {
            cx.child(
                &crate::Text::new(text.to_owned())
                    .style(TextStyle::Callout)
                    .font_weight(weight)
                    .foreground(color),
            )
        };
        let mut lines = vec![
            wrapped(cx, self.title, Weight::SEMIBOLD, ink),
            wrapped(cx, self.text, Weight::REGULAR, muted),
        ];
        if let Some(how) = self.how {
            lines.push(cx.child(&crate::Divider::horizontal()));
            lines.push(wrapped(cx, how, Weight::REGULAR, ink));
        }
        let column = cx.container(
            LayoutStyle {
                width: Some(HELP_WIDTH),
                spacing: theme.spacing.s2,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &lines,
            Paint::None,
        );
        panel(cx, &[column], theme.spacing.s3, 0.0)
    }
}

const HELP_WIDTH: f32 = 280.0;

/// A small "?" beside something that needs explaining. Its panel opens under the
/// pointer and on keyboard focus, and a click keeps it open until a click elsewhere.
/// Screen readers get the same words as the button's description.
#[derive(Clone, Debug)]
pub struct HelpTip {
    title: String,
    text: String,
    how: Option<String>,
}

impl HelpTip {
    /// Explains what `title` is.
    pub fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        Self { title: title.into(), text: text.into(), how: None }
    }

    /// Adds how to use it, set apart from what it is.
    #[must_use]
    pub fn how(mut self, how: impl Into<String>) -> Self {
        self.how = Some(how.into());
        self
    }
}

impl View for HelpTip {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let pinned: State<bool> = cx.use_state(|| false);
        let toggle = pinned.clone();
        let button = IconButton::new(IconName::CircleHelp)
            .label(format!("{}: {}", cx.labels().help, self.title))
            .size(crate::ButtonSize::Small)
            .style(ButtonStyle::Ghost)
            .on_click(move || toggle.update(|open| *open = !*open));
        // Built as this view, so that hover and focus on the button are this view's.
        let node = button.build(cx);
        let description = match &self.how {
            Some(how) => format!("{} {how}", self.text),
            None => self.text.clone(),
        };
        cx.update_semantics(node, |s| s.description = Some(description.clone()));
        if pinned.get() || cx.is_hovered() || cx.is_focus_visible() {
            let panel = HelpPanel { title: &self.title, text: &self.text, how: self.how.as_deref() };
            let close = pinned.clone();
            let dismiss: Rc<dyn Fn()> = Rc::new(move || close.set(false));
            cx.overlay(node, Placement::Below, &panel, pinned.get().then_some(dismiss));
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

/// One entry of a [`DropdownMenu`] or a [`ContextMenu`](crate::ContextMenu). It can run an
/// action, show a check mark, show any view, or open a submenu at its side.
///
/// ```
/// use ravenrs_widgets::{DropdownMenu, MenuItem, Ui};
///
/// let menu = DropdownMenu::new("View")
///     .item(MenuItem::new("Show grid").checked(true))
///     .item(MenuItem::new("Export").item(MenuItem::new("As PDF")).item(MenuItem::new("As CSV")));
/// let mut ui = Ui::new(menu, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct MenuItem {
    label: String,
    icon: Option<IconName>,
    shortcut: Option<String>,
    destructive: bool,
    on_select: Option<Rc<dyn Fn()>>,
    checked: Option<bool>,
    element: Option<AnyView>,
    submenu: Vec<Entry>,
}

impl fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MenuItem").field("label", &self.label).finish_non_exhaustive()
    }
}

impl MenuItem {
    /// A menu item labeled `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            destructive: false,
            on_select: None,
            checked: None,
            element: None,
            submenu: Vec::new(),
        }
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

    /// Makes it an item that is on or off: a check mark shows while it is on, and screen
    /// readers hear which. What choosing it does is still up to [`Self::on_select`].
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Shows this view in the row instead of the label, such as a color with its name.
    /// The label still names the item for screen readers.
    pub fn element(mut self, view: impl View + 'static) -> Self {
        self.element = Some(Box::new(view));
        self
    }

    /// Adds an item to this item's submenu, which opens at its side on hover, on click or
    /// with the Right arrow; Left and Escape come back. An item with a submenu does not
    /// run [`Self::on_select`].
    pub fn item(mut self, item: MenuItem) -> Self {
        self.submenu.push(Entry::Item(item));
        self
    }

    /// Adds a separator line to this item's submenu.
    pub fn separator(mut self) -> Self {
        self.submenu.push(Entry::Separator);
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
    /// Shown instead of the label, which still names the row.
    element: Option<&'a dyn View>,
    /// An item that is on or off, as opposed to a plain command.
    toggled: Option<bool>,
    /// What this row opens at its side.
    submenu: &'a [Entry],
    /// The row of this list whose submenu is open, set when any row of the list has one.
    /// The flag says the submenu takes the keyboard, as it does unless the pointer opened it.
    open_submenu: Option<State<Option<(usize, bool)>>>,
}

impl ListRow<'_> {
    /// Opens this row's submenu from the keyboard or a click.
    fn opener(&self) -> Option<Rc<dyn Fn()>> {
        let open = self.open_submenu.clone().filter(|_| !self.submenu.is_empty())?;
        let index = self.roving.index;
        Some(Rc::new(move || open.set(Some((index, true)))))
    }

    /// While the pointer moves over this row, its submenu is the open one, if it has one.
    fn follow_pointer(&self, cx: &mut BuildCx<'_>, node: NodeId) {
        let Some(open) = self.open_submenu.clone() else { return };
        let (index, has_submenu, focus_to) =
            (self.roving.index, !self.submenu.is_empty(), self.roving.focus_to.clone());
        cx.on_pointer_move(
            node,
            Rc::new(move |_, _| {
                let current = *open.peek();
                if current.map(|(i, _)| i) == has_submenu.then_some(index) {
                    return;
                }
                open.set(has_submenu.then_some((index, false)));
                // The submenu that goes away may hold the keyboard.
                if current.is_some_and(|(_, focused)| focused) {
                    focus_to.request(index);
                }
            }),
        );
    }

    /// Builds the submenu at the side of this row while it is the open one.
    fn submenu_overlay(&self, cx: &mut BuildCx<'_>, node: NodeId) {
        let Some(open) = &self.open_submenu else { return };
        let Some((_, focused)) = open.get().filter(|(i, _)| *i == self.roving.index) else { return };
        if self.submenu.is_empty() {
            return;
        }
        let close_submenu: Rc<dyn Fn()> = {
            let (open, focus_to, index) = (open.clone(), self.roving.focus_to.clone(), self.roving.index);
            Rc::new(move || {
                open.set(None);
                focus_to.request(index);
            })
        };
        // Left comes back here; Right goes on to the next menu of a menubar, as anywhere else.
        let on_side: Rc<dyn Fn(bool)> = {
            let (close, outer) = (close_submenu.clone(), self.on_side.clone());
            Rc::new(move |forward| {
                if !forward {
                    close();
                } else if let Some(outer) = &outer {
                    outer(true);
                }
            })
        };
        let list = MenuLevel {
            entries: self.submenu,
            close: self.close.clone(),
            on_side: Some(on_side),
            takes_focus: focused,
        };
        // Told apart by the flag: asked again from the keyboard, it starts over and takes focus.
        let key = 0x7375_626d_656e_7500 + u64::from(focused);
        cx.keyed_overlay(key, node, Placement::Right, &list, Some(close_submenu));
    }
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
        let label = match self.element {
            Some(element) => cx.keyed_child(1, element),
            None => util::label(cx, self.label, TextStyle::Body, weight, ink),
        };
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
        let opener = self.opener();
        let is_open =
            self.open_submenu.as_ref().and_then(|o| o.get()).is_some_and(|(i, _)| i == self.roving.index);
        if opener.is_some() {
            children.push(util::icon(cx, IconName::ChevronRight, 16.0, muted));
        }
        let lit = hovered || (is_open && opener.is_some());
        let fill = if lit { theme.color(Semantic::Ink).with_alpha(0.07) } else { Color::TRANSPARENT };
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
        motion::control(cx, node);
        let mut semantics = Semantics::new(self.role);
        if self.role == Role::ListBoxOption {
            semantics = semantics.selected(self.checked);
        }
        if let Some(t) = self.trailing {
            semantics = semantics.name(self.label).description(t);
        }
        if self.element.is_some() {
            semantics = semantics.name(self.label);
        }
        if let Some(on) = self.toggled {
            semantics = semantics.toggled(on);
        }
        if opener.is_some() {
            semantics = semantics.expanded(is_open);
        }
        cx.semantics(node, semantics);
        if let Some(open) = &opener {
            let open = open.clone();
            cx.on_click(node, Rc::new(move || open()));
        } else {
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
        }
        cx.focusable(node, 7.0);
        let before: Option<crate::KeyHandler> = (self.on_side.is_some() || opener.is_some()).then(|| {
            let (side, opener) = (self.on_side.clone(), opener.clone());
            Rc::new(move |key: &ravenrs_core::Key, _: ravenrs_core::Modifiers| match key {
                ravenrs_core::Key::ArrowRight if opener.is_some() => {
                    if let Some(open) = &opener {
                        open();
                    }
                    true
                }
                ravenrs_core::Key::ArrowLeft | ravenrs_core::Key::ArrowRight if side.is_some() => {
                    if let Some(side) = &side {
                        side(*key == ravenrs_core::Key::ArrowRight);
                    }
                    true
                }
                _ => false,
            }) as crate::KeyHandler
        });
        self.roving.wire_with(cx, node, None, before);
        self.follow_pointer(cx, node);
        self.submenu_overlay(cx, node);
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
        MenuLevel {
            entries: self.entries,
            close: self.close.clone(),
            on_side: self.on_side.clone(),
            takes_focus: true,
        }
        .build(cx)
    }
}

/// One panel of a menu, the first one or a submenu. `close` closes the whole menu.
struct MenuLevel<'a> {
    entries: &'a [Entry],
    close: Rc<dyn Fn()>,
    on_side: Option<Rc<dyn Fn(bool)>>,
    /// A submenu the pointer opened leaves the keyboard where it was.
    takes_focus: bool,
}

impl View for MenuLevel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let mut rows = Vec::new();
        let count = self.entries.iter().filter(|e| matches!(e, Entry::Item(_))).count();
        let focus_to = util::roving_cell(cx, self.takes_focus.then_some(0));
        let open_submenu: State<Option<(usize, bool)>> = cx.use_state(|| None);
        // Plain menus never pay for following the pointer.
        let has_submenus =
            self.entries.iter().any(|e| matches!(e, Entry::Item(item) if !item.submenu.is_empty()));
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
                        checked: item.checked == Some(true),
                        destructive: item.destructive,
                        close: self.close.clone(),
                        on_click: item.on_select.clone(),
                        role: Role::MenuItem,
                        on_side: self.on_side.clone(),
                        element: item.element.as_deref().map(|view| view as &dyn View),
                        toggled: item.checked,
                        submenu: &item.submenu,
                        open_submenu: has_submenus.then(|| open_submenu.clone()),
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
    state: util::EditorState,
    fill: bool,
    size: crate::ButtonSize,
    searchable: bool,
}

impl Select {
    /// A select over `options`; `selected` holds the zero-based index of the choice.
    pub fn new(selected: Binding<usize>, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            selected,
            options: options.into_iter().map(Into::into).collect(),
            width: 220.0,
            fill: false,
            label: None,
            state: util::EditorState::default(),
            size: crate::ButtonSize::Medium,
            searchable: false,
        }
    }

    /// A select over values of any type, each with the text that shows it: `value` holds
    /// the value itself, not its position. A value that is not among the options shows
    /// nothing until one is picked.
    ///
    /// ```
    /// use ravenrs_reactive::State;
    /// use ravenrs_widgets::{Select, Ui};
    ///
    /// #[derive(Clone, PartialEq)]
    /// enum Quality {
    ///     Draft,
    ///     Final,
    /// }
    ///
    /// let quality = State::new(Quality::Final);
    /// let select = Select::of(quality.binding(), [(Quality::Draft, "Draft"), (Quality::Final, "Final")]);
    /// let mut ui = Ui::new(select, ravenrs_style::Theme::raven_dark());
    /// ui.update();
    /// ```
    pub fn of<T: Clone + PartialEq + 'static>(
        value: Binding<T>,
        options: impl IntoIterator<Item = (T, impl Into<String>)>,
    ) -> Self {
        let (values, labels): (Vec<T>, Vec<String>) =
            options.into_iter().map(|(value, label)| (value, label.into())).unzip();
        let values = Rc::new(values);
        let (read, write, known) = (value.clone(), value, values.clone());
        // Past the end when it is not among the options, which shows nothing.
        let selected = Binding::new(
            move || {
                let current = read.get();
                known.iter().position(|v| *v == current).unwrap_or(usize::MAX)
            },
            move |i: usize| {
                if let Some(value) = values.get(i) {
                    write.set(value.clone());
                }
            },
        );
        Self::new(selected, labels)
    }

    /// Opens with a search box that filters the options as you type.
    pub fn searchable(mut self) -> Self {
        self.searchable = true;
        self
    }

    /// Sets the height and text size to line up with a [`Button`] of the same size
    /// (`Medium` by default).
    pub fn size(mut self, size: crate::ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Takes the width of its parent, as other fields of a form do.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
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

    /// Turns the control off: it fades, leaves the tab order and ignores input.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Shows the value without letting it change; it can still be focused and read.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }
}

pub(crate) struct Options<'a> {
    pub options: &'a [String],
    pub selected: Binding<usize>,
    pub close: Rc<dyn Fn()>,
    pub width: f32,
}

impl View for Options<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let current = self.selected.get();
        let count = self.options.len();
        // Nothing to start from when the value is not among the options.
        let focus_to = list_focus(cx, if current < count { current } else { 0 });
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
                    element: None,
                    toggled: None,
                    submenu: &[],
                    open_submenu: None,
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
        let query: State<String> = cx.use_state(String::new);
        let highlight: State<usize> = cx.use_state(|| 0);
        let state = self.state;
        let is_open = open.get() && state.accepts_input();
        let hovered = cx.is_hovered() && state.accepts_input();
        let current = self.options.get(self.selected.get()).map_or("", String::as_str);
        let label =
            util::label(cx, current, self.size.text_style(), None, state.ink(theme.color(Semantic::Ink)));
        let label = cx.container(
            LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[label],
            Paint::None,
        );
        let chevron = util::icon(
            cx,
            if is_open { IconName::ChevronUp } else { IconName::ChevronDown },
            if self.size == crate::ButtonSize::Mini { 12.0 } else { 16.0 },
            state.ink(theme.color(Semantic::InkMuted)),
        );
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: state.ink(if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base }),
            }
        };
        let style = LayoutStyle {
            padding: Insets::symmetric(
                0.0,
                if matches!(self.size, crate::ButtonSize::Small | crate::ButtonSize::Mini) {
                    8.0
                } else {
                    12.0
                },
            ),
            width: (!self.fill).then_some(self.width),
            height: Some(self.size.height()),
            spacing: 8.0,
            align: Align::Center,
            align_self: Some(if self.fill { Align::Stretch } else { Align::Fit }),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            corner_radius: theme.radii.md,
            border: Some(border),
            ..QuadPaint::new(state.fill(theme.color(Semantic::Control)))
        };
        let node = cx.container(style, &[label, chevron], Paint::Quad(paint));
        cx.semantics(label, Semantics::hidden());
        let mut semantics = Semantics::new(Role::ComboBox).value(current).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, state.semantics(semantics));
        if state.disabled {
            return node;
        }
        if !state.accepts_input() {
            cx.focusable(node, theme.radii.md);
            return node;
        }
        // A new search each time, starting from the current option.
        let reset: Rc<dyn Fn()> = {
            let (query, highlight, selected) = (query.clone(), highlight.clone(), self.selected.clone());
            let count = self.options.len();
            Rc::new(move || {
                query.set(String::new());
                highlight.set(Some(selected.get()).filter(|i| *i < count).unwrap_or(0));
            })
        };
        let (toggle, new_search) = (open.clone(), reset.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                new_search();
                toggle.update(|o| *o = !*o);
            }),
        );
        // Arrow keys open the list too, like a native select.
        let opener = open.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let opens = matches!(key, ravenrs_core::Key::ArrowDown | ravenrs_core::Key::ArrowUp);
                if opens {
                    reset();
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
            if self.searchable {
                let pick: Rc<dyn Fn(usize)> = {
                    let (selected, close) = (self.selected.clone(), close.clone());
                    Rc::new(move |i| {
                        selected.set(i);
                        close();
                    })
                };
                let list = crate::floating::SearchPanel {
                    options: &self.options,
                    current: Some(self.selected.get()),
                    pick,
                    query,
                    highlight,
                    open: open.clone(),
                    width: self.width,
                };
                cx.overlay(node, Placement::Below, &list, Some(close));
            } else {
                let list = Options {
                    options: &self.options,
                    selected: self.selected.clone(),
                    close: close.clone(),
                    width: self.width,
                };
                cx.overlay(node, Placement::Below, &list, Some(close));
            }
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
    close_label: Option<String>,
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
            close_label: None,
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

    /// The close button's name for screen readers (from [`Labels`](crate::Labels) by default).
    pub fn close_label(mut self, label: impl Into<String>) -> Self {
        self.close_label = Some(label.into());
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
                .label(d.close_label.clone().unwrap_or_else(|| cx.labels().close.clone()))
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
        cx.motion(
            card,
            Motion::new()
                .surface_exit()
                .enter(Effect::moved(0.0, 8.0).with_scale(0.96), theme.motion.fast)
                .exit(Effect { opacity: 0.0, ..Effect::IDENTITY.with_scale(0.98) }, theme.motion.fast),
        );
        let scrim = motion::scrim(cx, viewport);
        let backdrop = cx.container(
            LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[scrim, card],
            Paint::None,
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
