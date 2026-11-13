//! Where commands live: a toolbar that folds into a menu when it runs out of
//! room, a button with alternatives, and a question before something final.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Insets, Point, Role, Size};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::floating::{PANEL_MARGIN, move_by_handle, panel_inside};
use crate::hotkey::Hotkey;
use crate::overlay::{Entry, MenuList};
use crate::{
    AnyView, Button, ButtonSize, ButtonStyle, Dialog, IconButton, IconName, MenuItem, Text, TextField,
    VStack, View, util,
};

/// Room a button takes besides its label: its padding, and the icon if it has one.
const BUTTON_PADDING: f32 = 26.0;
const ICON_ROOM: f32 = 22.0;
const TOOLBAR_GAP: f32 = 4.0;
const SEPARATOR_ROOM: f32 = 9.0;
const MORE_ROOM: f32 = 36.0;

/// One command of a [`Toolbar`].
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ToolbarAction {
    label: String,
    icon: Option<IconName>,
    style: ButtonStyle,
    disabled: bool,
    icon_only: bool,
    hotkey: Option<Hotkey>,
    on_click: Option<Rc<dyn Fn()>>,
    accessible_name: Option<String>,
    accessible_description: Option<String>,
}

impl fmt::Debug for ToolbarAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolbarAction").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ToolbarAction {
    /// A ghost button with this text.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            style: ButtonStyle::Ghost,
            disabled: false,
            icon_only: false,
            hotkey: None,
            on_click: None,
            accessible_name: None,
            accessible_description: None,
        }
    }

    /// What screen readers call it, instead of its label.
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// What screen readers say after its name. Its keys follow, when it has them.
    pub fn accessible_description(mut self, text: impl Into<String>) -> Self {
        self.accessible_description = Some(text.into());
        self
    }

    /// What screen readers say after its name: its description, then its keys.
    fn description(&self) -> Option<String> {
        let keys = self.hotkey.as_ref().map(Hotkey::label);
        let parts: Vec<&str> =
            [self.accessible_description.as_deref(), keys.as_deref()].into_iter().flatten().collect();
        (!parts.is_empty()).then(|| parts.join(". "))
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Shows only the icon; the label stays as its name, and in the overflow menu.
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// The button's style. `Ghost` by default.
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Greys the command out. It keeps its place, so its neighbors do not move
    /// under the pointer, but it is left out of the overflow menu.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Runs the command with these keys, also from the overflow menu's place, and
    /// shows them there. Off while the command is disabled.
    pub fn hotkey(mut self, hotkey: Hotkey) -> Self {
        self.hotkey = Some(hotkey);
        self
    }

    /// Runs `handler` when the command is chosen.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn room(&self, cx: &mut BuildCx<'_>) -> f32 {
        if self.icon_only && self.icon.is_some() {
            return MORE_ROOM;
        }
        let theme = *cx.theme();
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        let text = cx.measure_text(&self.label, &attrs).width;
        text + BUTTON_PADDING + if self.icon.is_some() { ICON_ROOM } else { 0.0 }
    }

    fn view(&self) -> AnyView {
        let handler = self.on_click.clone();
        let click = move || {
            if let Some(handler) = &handler {
                handler();
            }
        };
        let name = self.accessible_name.clone().unwrap_or_else(|| self.label.clone());
        let description = self.description();
        match (self.icon_only, self.icon) {
            (true, Some(icon)) => {
                let mut button = IconButton::new(icon)
                    .label(name)
                    .style(self.style)
                    .size(ButtonSize::Small)
                    .disabled(self.disabled)
                    .on_click(click);
                if let Some(description) = description {
                    button = button.description(description);
                }
                Box::new(button)
            }
            (_, icon) => {
                let mut button = Button::new(self.label.clone())
                    .style(self.style)
                    .size(ButtonSize::Small)
                    .disabled(self.disabled)
                    .on_click(click);
                if self.accessible_name.is_some() {
                    button = button.accessible_name(name);
                }
                if let Some(description) = description {
                    button = button.accessible_description(description);
                }
                if let Some(icon) = icon {
                    button = button.icon(icon);
                }
                Box::new(button)
            }
        }
    }

    fn menu_item(&self) -> MenuItem {
        let mut item = MenuItem::new(self.label.clone());
        if let Some(icon) = self.icon {
            item = item.icon(icon);
        }
        if self.style == ButtonStyle::Danger {
            item = item.destructive();
        }
        if let Some(hotkey) = &self.hotkey {
            item = item.shortcut(hotkey.label());
        }
        if let Some(handler) = self.on_click.clone() {
            item = item.on_select(move || handler());
        }
        item
    }
}

enum ToolbarEntry {
    Action(ToolbarAction),
    Separator,
    View(AnyView, f32),
}

/// Makes the keys of these commands work for as long as they are built.
fn register_hotkeys(cx: &mut BuildCx<'_>, entries: &[ToolbarEntry]) {
    // Shift first: without it, the same key would answer before.
    for wants_shift in [true, false] {
        for entry in entries {
            if let ToolbarEntry::Action(ToolbarAction {
                hotkey: Some(hotkey),
                on_click: Some(handler),
                disabled: false,
                ..
            }) = entry
                && hotkey.shift == wants_shift
            {
                hotkey.register(cx, handler.clone());
            }
        }
    }
}

/// The element of one entry of a bar of commands.
fn entry_node(cx: &mut BuildCx<'_>, index: usize, entry: &ToolbarEntry) -> NodeId {
    match entry {
        ToolbarEntry::Action(action) => cx.keyed_child(index as u64, action.view().as_ref()),
        ToolbarEntry::View(view, _) => cx.keyed_child(index as u64, view.as_ref()),
        ToolbarEntry::Separator => {
            let line = cx.theme().color(Semantic::Border);
            cx.container(
                LayoutStyle { width: Some(1.0), height: Some(18.0), ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint::new(line)),
            )
        }
    }
}

/// A row of commands. What does not fit moves to a menu at its end, from the
/// last command back, so the order never changes.
///
/// ```
/// use ravenrs_widgets::{IconName, Toolbar, ToolbarAction, Ui};
///
/// let bar = Toolbar::new()
///     .action(ToolbarAction::new("New").icon(IconName::Plus).on_click(|| {}))
///     .separator()
///     .action(ToolbarAction::new("Delete").icon(IconName::Trash2).disabled(true));
/// let mut ui = Ui::new(bar, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Toolbar {
    entries: Vec<ToolbarEntry>,
    label: Option<String>,
    aligned_end: bool,
}

impl fmt::Debug for Toolbar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Toolbar").field("entries", &self.entries.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Toolbar {
    /// An empty toolbar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a command.
    pub fn action(mut self, action: ToolbarAction) -> Self {
        self.entries.push(ToolbarEntry::Action(action));
        self
    }

    /// Appends a line between groups of commands.
    pub fn separator(mut self) -> Self {
        self.entries.push(ToolbarEntry::Separator);
        self
    }

    /// Appends any view, such as a search field, that needs `width` logical
    /// pixels. It stays on the bar while there is room, and is dropped after that.
    pub fn view(mut self, width: f32, view: impl View + 'static) -> Self {
        self.entries.push(ToolbarEntry::View(Box::new(view), width));
        self
    }

    /// The name screen readers announce.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Takes the room left in its row, with the commands at the far end.
    pub(crate) fn aligned_end(mut self) -> Self {
        self.aligned_end = true;
        self
    }
}

/// How many entries of these widths fit in `available`, leaving room for the
/// overflow menu if some have to go.
pub(crate) fn entries_that_fit(widths: &[f32], available: f32, more: f32, gap: f32) -> usize {
    let total: f32 = widths.iter().map(|w| w + gap).sum();
    if total - gap <= available {
        return widths.len();
    }
    let mut used = 0.0;
    for (i, w) in widths.iter().enumerate() {
        used += w + gap;
        if used + more > available {
            return i;
        }
    }
    widths.len()
}

impl View for Toolbar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        register_hotkeys(cx, &self.entries);
        let widths: Vec<f32> = self
            .entries
            .iter()
            .map(|entry| match entry {
                ToolbarEntry::Action(action) => action.room(cx),
                ToolbarEntry::Separator => SEPARATOR_ROOM,
                ToolbarEntry::View(_, width) => *width,
            })
            .collect();
        let fits = cx
            .scroll_viewport()
            .map_or(widths.len(), |v| entries_that_fit(&widths, v.width, MORE_ROOM, TOOLBAR_GAP));
        // A line with nothing after it says nothing.
        let shown = if fits < widths.len()
            && matches!(self.entries.get(fits.wrapping_sub(1)), Some(ToolbarEntry::Separator))
        {
            fits - 1
        } else {
            fits
        };

        let mut children = Vec::with_capacity(shown + 1);
        for (i, entry) in self.entries[..shown].iter().enumerate() {
            children.push(entry_node(cx, i, entry));
        }
        let hidden = self.entries[shown..]
            .iter()
            .filter_map(|entry| match entry {
                ToolbarEntry::Action(action) if !action.disabled => Some(Entry::Item(action.menu_item())),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !hidden.is_empty() {
            children.push(cx.keyed_child(u64::MAX, &MoreMenu(hidden)));
        }
        let node = cx.container(
            LayoutStyle {
                spacing: TOOLBAR_GAP,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                height: Some(36.0),
                min_width: Some(0.0),
                overflow_hidden: true,
                grow: if self.aligned_end { 1.0 } else { 0.0 },
                justify: if self.aligned_end { Justify::End } else { Justify::Start },
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::None,
        );
        // Being a scroll container is what tells the next build how wide it is.
        cx.scrollable_virtual(node, false);
        let mut semantics = Semantics::new(ravenrs_core::Role::Group);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
        node
    }
}

/// How opaque a [`FloatingToolbar`] is, so that it says it is over what is under it.
const FLOATING_OPACITY: f32 = 0.92;

/// A translucent row of commands floating over its content, such as a zoom bar over a photo.
/// Clicks on it never reach the content. Tab goes through its commands.
///
/// With [`FloatingToolbar::movable`] it has a handle: dragging it, or its arrow keys, take the
/// bar anywhere in the content, and a double click brings it home. Smaller content than the bar
/// leaves it at the top left corner.
///
/// ```
/// use ravenrs_layout::Align;
/// use ravenrs_widgets::{FloatingToolbar, IconName, Text, ToolbarAction, Ui};
///
/// let bar = FloatingToolbar::new(Text::new("A photo"))
///     .label("Zoom")
///     .action(ToolbarAction::new("Fit").on_click(|| {}))
///     .separator()
///     .action(ToolbarAction::new("Zoom in").icon(IconName::Plus).icon_only().on_click(|| {}))
///     .place(Align::Start, Align::End);
/// let mut ui = Ui::new(bar, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct FloatingToolbar<C> {
    content: C,
    bar: FloatingBar,
    fill: bool,
}

/// What a [`FloatingToolbar`] has apart from its content.
struct FloatingBar {
    entries: Vec<ToolbarEntry>,
    label: Option<String>,
    across: Align,
    down: Align,
    margin: f32,
    place: Option<Binding<Option<Point>>>,
}

impl<C> fmt::Debug for FloatingToolbar<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FloatingToolbar").field("entries", &self.bar.entries.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> FloatingToolbar<C> {
    /// An empty bar at the bottom center of `content`.
    pub fn new(content: C) -> Self {
        Self {
            content,
            bar: FloatingBar {
                entries: Vec::new(),
                label: None,
                across: Align::Center,
                down: Align::End,
                margin: 12.0,
                place: None,
            },
            fill: false,
        }
    }

    /// Appends a command.
    pub fn action(mut self, action: ToolbarAction) -> Self {
        self.bar.entries.push(ToolbarEntry::Action(action));
        self
    }

    /// Appends a line between groups of commands.
    pub fn separator(mut self) -> Self {
        self.bar.entries.push(ToolbarEntry::Separator);
        self
    }

    /// Appends any view, such as a [`ValueSlider`](crate::ValueSlider).
    pub fn view(mut self, view: impl View + 'static) -> Self {
        self.bar.entries.push(ToolbarEntry::View(Box::new(view), 0.0));
        self
    }

    /// The name screen readers announce.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.bar.label = Some(label.into());
        self
    }

    /// Which side of the content it stays on, across and down: `Start`, `Center` or `End`.
    /// At the bottom center by default.
    pub fn place(mut self, across: Align, down: Align) -> Self {
        self.bar.across = across;
        self.bar.down = down;
        self
    }

    /// How far from the edges of the content it stays, in logical pixels (12 by default).
    pub fn margin(mut self, margin: f32) -> Self {
        self.bar.margin = margin;
        self
    }

    /// Adds a handle that moves the bar. `place` is where it was left, from the top left
    /// corner of the content, written when the drag ends so that the app can keep it; `None`
    /// is the place given by [`Self::place`].
    pub fn movable(mut self, place: Binding<Option<Point>>) -> Self {
        self.bar.place = Some(place);
        self
    }

    /// Takes the free space of the parent, for content that fills a stage or a whole screen.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }
}

/// The handle of a movable [`FloatingToolbar`].
struct BarHandle<'a> {
    at: Point,
    largest: Point,
    place: &'a Binding<Option<Point>>,
    dragging: State<Option<Point>>,
    name: String,
}

impl View for BarHandle<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let ink = theme.color(if cx.is_hovered() { Semantic::Ink } else { Semantic::InkMuted });
        let icon = util::icon(cx, IconName::EllipsisVertical, 14.0, ink);
        let node = cx.container(
            LayoutStyle {
                width: Some(16.0),
                height: Some(ButtonSize::Small.height()),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[icon],
            Paint::None,
        );
        move_by_handle(cx, node, self.at, self.largest, self.place, &self.dragging);
        let home = self.place.clone();
        cx.on_double_click(node, Rc::new(move || home.set(None)));
        cx.semantics(node, Semantics::new(Role::Button).name(self.name.clone()));
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// Where the bar goes along one side of the content.
fn along(side: Align, room: f32, size: f32, margin: f32) -> f32 {
    let at = match side {
        Align::Center => (room - size) / 2.0,
        Align::End => room - size - margin,
        _ => margin,
    };
    at.max(PANEL_MARGIN)
}

/// The bar itself. Its own view, so that the size it reports is the bar's.
struct BarSurface<'a> {
    bar: &'a FloatingBar,
    stage: Size,
}

impl View for BarSurface<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let bar = self.bar;
        // Both sizes come from the last layout, and a change in either builds again.
        let size = cx.scroll_viewport().unwrap_or(Size::ZERO);
        let largest = Point::new(
            (self.stage.width - size.width - PANEL_MARGIN).max(PANEL_MARGIN),
            (self.stage.height - size.height - PANEL_MARGIN).max(PANEL_MARGIN),
        );
        let home = Point::new(
            along(bar.across, self.stage.width, size.width, bar.margin),
            along(bar.down, self.stage.height, size.height, bar.margin),
        );
        let dragging: State<Option<Point>> = cx.use_state(|| None);
        let at = dragging
            .get()
            .or_else(|| bar.place.as_ref().and_then(Binding::get))
            .map_or(home, |wanted| panel_inside(wanted, largest));

        let mut children = Vec::with_capacity(bar.entries.len() + 1);
        if let Some(place) = &bar.place {
            let name = match &bar.label {
                Some(label) => format!("{}: {label}", cx.labels().panel_move),
                None => cx.labels().panel_move.clone(),
            };
            let handle = BarHandle { at, largest, place, dragging, name };
            children.push(cx.keyed_child(u64::MAX, &handle));
        }
        for (i, entry) in bar.entries.iter().enumerate() {
            children.push(entry_node(cx, i, entry));
        }
        let fill = theme.color(Semantic::SurfaceElevated);
        let node = cx.container(
            LayoutStyle {
                absolute: Some((at.x, at.y)),
                padding: Insets::uniform(theme.spacing.s1),
                spacing: TOOLBAR_GAP,
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(fill.with_alpha(fill.a * FLOATING_OPACITY))
            }),
        );
        cx.report_frame(node);
        // What lands on it stays on it, instead of reaching the content.
        cx.cursor(node, CursorIcon::Default);
        let mut semantics = Semantics::new(Role::Toolbar);
        if let Some(label) = &bar.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
        node
    }
}

impl<C: View> View for FloatingToolbar<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        register_hotkeys(cx, &self.bar.entries);
        let content = cx.child(&self.content);
        let stage = cx.scroll_viewport().unwrap_or(Size::ZERO);
        let surface = cx.child(&BarSurface { bar: &self.bar, stage });
        let root = cx.container(
            LayoutStyle {
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                grow: if self.fill { 1.0 } else { 0.0 },
                min_height: self.fill.then_some(0.0),
                min_width: self.fill.then_some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[content, surface],
            Paint::None,
        );
        cx.report_frame(root);
        root
    }
}

struct MoreMenu(Vec<Entry>);

impl View for MoreMenu {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        menu_trigger(
            cx,
            &self.0,
            IconName::Ellipsis,
            ButtonStyle::Ghost,
            ButtonSize::Small,
            cx.labels().more_commands.clone(),
        )
    }
}

/// A square button that opens `entries` under it, and takes focus back after.
pub(crate) fn menu_trigger(
    cx: &mut BuildCx<'_>,
    entries: &[Entry],
    icon: IconName,
    style: ButtonStyle,
    size: ButtonSize,
    name: String,
) -> NodeId {
    let open: State<bool> = cx.use_state(|| false);
    let toggle = open.clone();
    let anchor = cx.child(
        &IconButton::new(icon)
            .label(name)
            .style(style)
            .size(size)
            .on_click(move || toggle.update(|o| *o = !*o)),
    );
    let is_open = open.get();
    cx.update_semantics(anchor, |s| s.expanded = Some(is_open));
    let was_open = cx.use_state(|| Rc::new(Cell::new(false))).get();
    if is_open {
        was_open.set(true);
        let close: Rc<dyn Fn()> = Rc::new(move || open.set(false));
        cx.overlay(
            anchor,
            Placement::Below,
            &MenuList { entries, close: close.clone(), on_side: None },
            Some(close),
        );
    } else if was_open.replace(false) {
        cx.request_focus_node(anchor);
    }
    anchor
}

/// A button with a main command and, beside it, a menu of alternatives, such as
/// Save, with Save and Close and Save and New.
///
/// ```
/// use ravenrs_widgets::{MenuItem, SplitButton, Ui};
///
/// let save = SplitButton::new("Save")
///     .on_click(|| {})
///     .item(MenuItem::new("Save and close").on_select(|| {}));
/// let mut ui = Ui::new(save, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SplitButton {
    label: String,
    icon: Option<IconName>,
    style: ButtonStyle,
    size: ButtonSize,
    disabled: bool,
    on_click: Option<Rc<dyn Fn()>>,
    entries: Vec<Entry>,
}

impl fmt::Debug for SplitButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitButton").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SplitButton {
    /// A primary button with this text and an empty menu.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            style: ButtonStyle::Primary,
            size: ButtonSize::Medium,
            disabled: false,
            on_click: None,
            entries: Vec::new(),
        }
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The buttons' style. `Primary` by default.
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// The buttons' size. `Medium` by default.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Turns off the main command and the menu.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Runs `handler` when the main button is clicked.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Appends an alternative to the menu.
    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }
}

impl View for SplitButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let handler = self.on_click.clone();
        let mut main = Button::new(self.label.clone())
            .style(self.style)
            .size(self.size)
            .disabled(self.disabled)
            .on_click(move || {
                if let Some(handler) = &handler {
                    handler();
                }
            });
        if let Some(icon) = self.icon {
            main = main.icon(icon);
        }
        let main = cx.child(&main);
        let name = format!("{}: {}", self.label, cx.labels().more_commands);
        let trigger = if self.disabled {
            cx.child(
                &IconButton::new(IconName::ChevronDown)
                    .label(name)
                    .style(self.style)
                    .size(self.size)
                    .disabled(true),
            )
        } else {
            menu_trigger(cx, &self.entries, IconName::ChevronDown, self.style, self.size, name)
        };
        cx.container(
            LayoutStyle {
                spacing: 1.0,
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[main, trigger],
            Paint::None,
        )
    }
}

/// A question before something that cannot be undone. Holds what is being asked
/// until it is answered; show [`ConfirmDialog`] once, and call
/// [`ask`](Self::ask) from anywhere.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ConfirmDialog(State<Option<Rc<Question>>>);

struct Question {
    title: String,
    message: String,
    action: String,
    /// The word that has to be typed, exactly, before the action can run.
    word: Option<String>,
    typed: State<String>,
    run: Rc<dyn Fn()>,
}

impl fmt::Debug for ConfirmDialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfirmDialog").field("asking", &self.0.peek().is_some()).finish()
    }
}

impl Default for ConfirmDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ConfirmDialog {
    /// A dialog with nothing to ask yet.
    pub fn new() -> Self {
        Self(State::new(None))
    }

    /// Asks `title`, explaining with `message`. The button called `action` runs `run`.
    pub fn ask(
        &self,
        title: impl Into<String>,
        message: impl Into<String>,
        action: impl Into<String>,
        run: impl Fn() + 'static,
    ) {
        self.put(title.into(), message.into(), action.into(), None, Rc::new(run));
    }

    /// Like [`ask`](Self::ask), for what is too costly to answer with one click: the
    /// button called `action` only works after `word` is typed exactly as it is, capitals
    /// included, so a hurried click never runs `run`.
    pub fn ask_typing(
        &self,
        title: impl Into<String>,
        message: impl Into<String>,
        action: impl Into<String>,
        word: impl Into<String>,
        run: impl Fn() + 'static,
    ) {
        self.put(title.into(), message.into(), action.into(), Some(word.into()), Rc::new(run));
    }

    fn put(&self, title: String, message: String, action: String, word: Option<String>, run: Rc<dyn Fn()>) {
        let typed = State::new(String::new());
        self.0.set(Some(Rc::new(Question { title, message, action, word, typed, run })));
    }
}

impl View for ConfirmDialog {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let Some(question) = self.0.get() else { return cx.empty() };
        let (read, write) = (self.0.clone(), self.0.clone());
        let open = Binding::new(
            move || read.get().is_some(),
            move |open: bool| {
                if !open {
                    write.set(None);
                }
            },
        );
        let (cancel, done, run) = (self.0.clone(), self.0.clone(), question.run.clone());
        let labels = cx.labels();
        let typed_right = question.word.as_ref().is_none_or(|word| question.typed.get() == *word);
        let mut dialog = Dialog::new(open, question.title.clone()).description(question.message.clone());
        if let Some(word) = &question.word {
            let (answered, run, word_typed) = (self.0.clone(), question.run.clone(), word.clone());
            let field = TextField::new(question.typed.binding())
                .placeholder(word.clone())
                .autofocus()
                .on_submit(move |text| {
                    if text == word_typed {
                        answered.set(None);
                        run();
                    }
                });
            dialog = dialog.content(
                VStack::new((
                    Text::new(format!("{} {word}", labels.confirm_by_typing)).style(TextStyle::Callout),
                    field,
                ))
                .spacing(8.0)
                .align(Align::Stretch),
            );
        }
        cx.child(
            &dialog
                .action(
                    Button::new(labels.cancel.clone())
                        .style(ButtonStyle::Secondary)
                        .on_click(move || cancel.set(None)),
                )
                .action(
                    Button::new(question.action.clone())
                        .style(ButtonStyle::Danger)
                        .disabled(!typed_right)
                        .on_click(move || {
                            done.set(None);
                            run();
                        }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::entries_that_fit;

    #[test]
    fn everything_fits_or_the_menu_takes_room() {
        let w = [50.0, 50.0, 50.0];
        assert_eq!(entries_that_fit(&w, 160.0, 30.0, 0.0), 3, "no menu is needed");
        assert_eq!(entries_that_fit(&w, 150.0, 30.0, 0.0), 3, "exactly enough");
        assert_eq!(entries_that_fit(&w, 149.0, 30.0, 0.0), 2, "the menu pushes one out");
        assert_eq!(entries_that_fit(&w, 100.0, 30.0, 0.0), 1);
        assert_eq!(entries_that_fit(&w, 20.0, 30.0, 0.0), 0);
        assert_eq!(entries_that_fit(&[], 0.0, 30.0, 0.0), 0);
        assert_eq!(entries_that_fit(&w, 158.0, 30.0, 4.0), 3, "gaps only between entries");
    }
}
