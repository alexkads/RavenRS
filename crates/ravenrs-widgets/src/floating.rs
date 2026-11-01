//! More floating UI: context menus, hover cards, sheets, toasts, the command
//! palette and the combobox.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Point, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, Placement, QuadPaint, Semantics};
use crate::overlay::{Entry, MenuList, PanelOf, panel};
use crate::{ButtonSize, IconButton, IconName, MenuItem, TextField, Tone, View, util};

/// Opens a menu at the pointer on right click (or long press, planned) over its content.
pub struct ContextMenu<V> {
    content: V,
    entries: Vec<Entry>,
}

impl<V> fmt::Debug for ContextMenu<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextMenu").field("entries", &self.entries.len()).finish_non_exhaustive()
    }
}

impl<V: View> ContextMenu<V> {
    /// Wraps `content` with an initially empty menu.
    pub fn new(content: V) -> Self {
        Self { content, entries: Vec::new() }
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

impl<V: View> View for ContextMenu<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let at: State<Option<Point>> = cx.use_state(|| None);
        let child = cx.child(&self.content);
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        let open = at.clone();
        cx.on_context_menu(node, Rc::new(move |p| open.set(Some(p))));
        if let Some(p) = at.get() {
            let close: Rc<dyn Fn()> = {
                let at = at.clone();
                Rc::new(move || at.set(None))
            };
            cx.overlay(
                node,
                Placement::At(p),
                &MenuList { entries: &self.entries, close: close.clone(), on_side: None },
                Some(close),
            );
        }
        node
    }
}

/// Rich content shown below a trigger while the pointer is over the trigger or
/// the card itself.
pub struct HoverCard<A, C> {
    trigger: A,
    content: C,
}

impl<A, C> fmt::Debug for HoverCard<A, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HoverCard")
    }
}

impl<A: View, C: View> HoverCard<A, C> {
    /// Shows `content` below `trigger` on hover.
    pub fn new(trigger: A, content: C) -> Self {
        Self { trigger, content }
    }
}

impl<A: View, C: View> View for HoverCard<A, C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.trigger);
        let node = cx.container(
            LayoutStyle { align_self: Some(Align::Start), ..LayoutStyle::stack(Axis::Horizontal) },
            &[child],
            Paint::None,
        );
        cx.cursor(node, CursorIcon::Default);
        if cx.is_hover_within() {
            cx.overlay(node, Placement::Below, &PanelOf(&self.content), None);
        }
        node
    }
}

/// The edge a [`Sheet`] slides in from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Side {
    /// The left edge.
    Left,
    /// The right edge.
    #[default]
    Right,
}

/// A panel that slides over the UI from one edge, over a dimmed backdrop.
pub struct Sheet {
    open: Binding<bool>,
    title: String,
    description: Option<String>,
    content: Option<crate::AnyView>,
    side: Side,
    width: f32,
}

impl fmt::Debug for Sheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sheet").field("title", &self.title).finish_non_exhaustive()
    }
}

impl Sheet {
    /// A sheet shown while `open` is true; slides from the right, 400 logical pixels wide.
    pub fn new(open: Binding<bool>, title: impl Into<String>) -> Self {
        Self { open, title: title.into(), description: None, content: None, side: Side::Right, width: 400.0 }
    }

    /// Adds a muted description under the title.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the sheet body.
    pub fn content(mut self, content: impl View + 'static) -> Self {
        self.content = Some(Box::new(content));
        self
    }

    /// Sets the edge it slides in from.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Sets the panel width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

struct SheetLayer<'a>(&'a Sheet);

impl View for SheetLayer<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let s = self.0;
        let viewport = cx.viewport();
        let close = s.open.clone();
        let title = util::label(cx, &s.title, TextStyle::Title, None, theme.color(Semantic::Ink));
        let title = cx.container(
            LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[title],
            Paint::None,
        );
        let x = cx.child(
            &IconButton::new(IconName::X)
                .label("Close")
                .size(ButtonSize::Small)
                .on_click(move || close.set(false)),
        );
        let header = cx.container(
            LayoutStyle { align: Align::Center, spacing: 12.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[title, x],
            Paint::None,
        );
        let mut body = vec![header];
        if let Some(d) = &s.description {
            body.push(util::label(cx, d, TextStyle::Body, None, theme.color(Semantic::InkMuted)));
        }
        if let Some(c) = &s.content {
            body.push(cx.child(c.as_ref()));
        }
        let panel = cx.container(
            LayoutStyle {
                width: Some(s.width.min(viewport.width)),
                height: Some(viewport.height),
                padding: Insets::uniform(24.0),
                spacing: 16.0,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &body,
            Paint::Quad(QuadPaint {
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                shadow: Some(theme.elevation.raised),
                ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
            }),
        );
        let mut semantics = Semantics::new(Role::Dialog).name(s.title.clone()).modal();
        if let Some(d) = &s.description {
            semantics = semantics.description(d.clone());
        }
        cx.semantics(panel, semantics);
        cx.cursor(panel, CursorIcon::Default);
        let backdrop = cx.container(
            LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                justify: if s.side == Side::Right { Justify::End } else { Justify::Start },
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[panel],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Scrim))),
        );
        let close = s.open.clone();
        cx.on_click(backdrop, Rc::new(move || close.set(false)));
        backdrop
    }
}

impl View for Sheet {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        if self.open.get() {
            let close = self.open.clone();
            cx.overlay(anchor, Placement::Fill, &SheetLayer(self), Some(Rc::new(move || close.set(false))));
        }
        anchor
    }
}

/// One notification in a [`Toaster`].
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    id: u64,
    title: String,
    message: String,
    tone: Tone,
    duration: f64,
    /// When it was first shown, on the UI clock.
    shown_at: Rc<Cell<Option<f64>>>,
}

/// A handle to show toasts from anywhere (event handlers, async tasks).
/// Place one [`Toaster`] in the UI to display them.
///
/// ```
/// use ravenrs_widgets::{Toaster, Toasts, Tone, Ui};
///
/// let toasts = Toasts::new();
/// let mut ui = Ui::new(Toaster::new(toasts.clone()), ravenrs_style::Theme::raven_dark());
///
/// toasts.show("Saved", "Your changes are stored.", Tone::Success);
/// assert_eq!(toasts.len(), 1);
/// ui.update();
/// ```
#[derive(Clone, Debug, Default)]
pub struct Toasts {
    list: State<Vec<Toast>>,
    next: Rc<Cell<u64>>,
}

impl Toasts {
    /// An empty toast list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows a toast for 4 seconds.
    pub fn show(&self, title: impl Into<String>, message: impl Into<String>, tone: Tone) {
        self.show_for(title, message, tone, 4.0);
    }

    /// Shows a toast for `seconds` seconds. At most four are kept; the oldest go first.
    pub fn show_for(&self, title: impl Into<String>, message: impl Into<String>, tone: Tone, seconds: f64) {
        let id = self.next.get();
        self.next.set(id + 1);
        let toast = Toast {
            id,
            title: title.into(),
            message: message.into(),
            tone,
            duration: seconds,
            shown_at: Rc::new(Cell::new(None)),
        };
        self.list.update(|l| {
            l.push(toast);
            // Keep the stack short; the oldest go first.
            let excess = l.len().saturating_sub(4);
            l.drain(..excess);
        });
    }

    /// Removes the toast with this id, if still shown.
    pub fn dismiss(&self, id: u64) {
        self.list.update(|l| l.retain(|t| t.id != id));
    }

    /// Number of toasts currently shown.
    pub fn len(&self) -> usize {
        self.list.peek().len()
    }

    /// No toast is shown.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Displays [`Toasts`] stacked in the bottom-right corner; each disappears
/// after its duration or when closed.
#[derive(Debug)]
pub struct Toaster(Toasts);

impl Toaster {
    /// Displays the toasts of `toasts`.
    pub fn new(toasts: Toasts) -> Self {
        Self(toasts)
    }
}

fn tone_color(theme: &Theme, tone: Tone) -> (Color, IconName) {
    match tone {
        Tone::Neutral => (theme.color(Semantic::InkMuted), IconName::Bell),
        Tone::Accent => {
            (theme.color(Semantic::Accent).lerp(theme.color(Semantic::Ink), 0.25), IconName::Info)
        }
        Tone::Success => (theme.color(Semantic::Success), IconName::CircleCheck),
        Tone::Warning => (theme.color(Semantic::Warning), IconName::TriangleAlert),
        Tone::Danger => (theme.color(Semantic::Danger), IconName::CircleAlert),
    }
}

struct ToastStack<'a>(&'a [Toast], Toasts);

impl View for ToastStack<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut cards = Vec::new();
        for t in self.0 {
            let (color, icon) = tone_color(&theme, t.tone);
            let icon = util::icon(cx, icon, 18.0, color);
            let title = util::label(
                cx,
                &t.title,
                TextStyle::Body,
                Some(Weight::SEMIBOLD),
                theme.color(Semantic::Ink),
            );
            let msg = util::label(cx, &t.message, TextStyle::Callout, None, theme.color(Semantic::InkMuted));
            let text = cx.container(
                LayoutStyle { grow: 1.0, spacing: 2.0, ..LayoutStyle::stack(Axis::Vertical) },
                &[title, msg],
                Paint::None,
            );
            let (toasts, id) = (self.1.clone(), t.id);
            let x = cx.keyed_child(
                t.id,
                &IconButton::new(IconName::X)
                    .label("Dismiss")
                    .size(ButtonSize::Small)
                    .on_click(move || toasts.dismiss(id)),
            );
            let card = cx.container(
                LayoutStyle {
                    width: Some(360.0),
                    padding: Insets { top: 14.0, bottom: 14.0, left: 16.0, right: 8.0 },
                    spacing: 12.0,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[icon, text, x],
                Paint::Quad(QuadPaint {
                    corner_radius: theme.radii.lg,
                    border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                    shadow: Some(theme.elevation.raised),
                    ..QuadPaint::new(theme.color(Semantic::SurfaceElevated))
                }),
            );
            // Announced as it appears; danger toasts interrupt.
            let role = if t.tone == Tone::Danger { Role::Alert } else { Role::Status };
            cx.semantics(card, Semantics::new(role));
            cx.cursor(card, CursorIcon::Default);
            cards.push(card);
        }
        cx.container(LayoutStyle { spacing: 10.0, ..LayoutStyle::stack(Axis::Vertical) }, &cards, Paint::None)
    }
}

impl View for Toaster {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let anchor = cx.empty();
        let now = cx.now();
        let all = self.0.list.get();
        let mut visible = Vec::new();
        let mut expired = false;
        for t in &all {
            let shown = t.shown_at.get().unwrap_or_else(|| {
                t.shown_at.set(Some(now));
                now
            });
            let until = shown + t.duration;
            if now >= until {
                expired = true;
            } else {
                cx.wake_at(until);
                visible.push(t.clone());
            }
        }
        if expired {
            let ids: Vec<u64> = visible.iter().map(|t| t.id).collect();
            self.0.list.update(|l| l.retain(|t| ids.contains(&t.id)));
        }
        if !visible.is_empty() {
            cx.overlay(anchor, Placement::BottomRight, &ToastStack(&visible, self.0.clone()), None);
        }
        anchor
    }
}

/// An action in the [`CommandPalette`].
pub struct Command {
    label: String,
    group: String,
    icon: Option<IconName>,
    shortcut: Option<String>,
    action: Rc<dyn Fn()>,
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command").field("label", &self.label).finish_non_exhaustive()
    }
}

impl Command {
    /// A command labeled `label`; `action` runs when it is chosen.
    pub fn new(label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            group: String::new(),
            icon: None,
            shortcut: None,
            action: Rc::new(action),
        }
    }

    /// Lists the command under this group heading.
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    /// Shows an icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Shows a shortcut hint, e.g. `"⌘S"` (display only).
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// Is every character of `query` found in order in `text`? (case-insensitive)
pub(crate) fn fuzzy_match(text: &str, query: &str) -> bool {
    let text = text.to_lowercase();
    let mut chars = text.chars();
    query.to_lowercase().chars().filter(|c| !c.is_whitespace()).all(|q| chars.any(|c| c == q))
}

/// A searchable list of commands in a modal, opened with Cmd/Ctrl+K.
/// Type to filter (fuzzy), use ↑ ↓ to choose, Enter to run, Escape to close.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{Command, CommandPalette, IconName, Ui};
///
/// let open = State::new(true);
/// let palette = CommandPalette::new(open.binding())
///     .command(Command::new("New file", || {}).group("File").icon(IconName::Plus).shortcut("⌘N"))
///     .command(Command::new("Toggle theme", || {}).group("View"));
///
/// let mut ui = Ui::new(palette, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
pub struct CommandPalette {
    open: Binding<bool>,
    commands: Vec<Command>,
    placeholder: String,
}

impl fmt::Debug for CommandPalette {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandPalette").field("commands", &self.commands.len()).finish_non_exhaustive()
    }
}

impl CommandPalette {
    /// A palette shown while `open` is true; Cmd/Ctrl+K toggles it.
    pub fn new(open: Binding<bool>) -> Self {
        Self { open, commands: Vec::new(), placeholder: "Type a command or search…".into() }
    }

    /// Appends a command.
    pub fn command(mut self, command: Command) -> Self {
        self.commands.push(command);
        self
    }

    /// Sets the search box placeholder.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }
}

struct PaletteLayer<'a> {
    palette: &'a CommandPalette,
    query: State<String>,
    highlight: State<usize>,
}

impl View for PaletteLayer<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let viewport = cx.viewport();
        let q = self.query.get();
        let matches: Vec<usize> = (0..self.palette.commands.len())
            .filter(|&i| fuzzy_match(&self.palette.commands[i].label, &q))
            .collect();
        let highlight = self.highlight.get().min(matches.len().saturating_sub(1));
        let actions: Rc<Vec<Rc<dyn Fn()>>> =
            Rc::new(matches.iter().map(|&i| self.palette.commands[i].action.clone()).collect());

        let (hl, open, acts, count) =
            (self.highlight.clone(), self.palette.open.clone(), actions.clone(), matches.len());
        let field = cx.child(
            &TextField::new(self.query.binding())
                .placeholder(self.palette.placeholder.clone())
                .icon(IconName::Search)
                .autofocus()
                .intercept_keys(Rc::new(move |key, _| {
                    match key {
                        Key::ArrowDown if count > 0 => hl.update(|h| *h = (*h + 1).min(count - 1)),
                        Key::ArrowUp => hl.update(|h| *h = h.saturating_sub(1)),
                        Key::Enter => {
                            if let Some(a) = acts.get(hl.get().min(count.saturating_sub(1))) {
                                open.set(false);
                                a();
                            }
                        }
                        Key::Escape => open.set(false),
                        _ => return false,
                    }
                    true
                })),
        );
        let mut rows = vec![field];
        let mut last_group: Option<&str> = None;
        for (n, &i) in matches.iter().enumerate().take(12) {
            let c = &self.palette.commands[i];
            if last_group != Some(c.group.as_str()) && !c.group.is_empty() {
                rows.push(util::label(
                    cx,
                    &c.group.to_uppercase(),
                    TextStyle::Caption,
                    None,
                    theme.color(Semantic::InkMuted),
                ));
            }
            last_group = Some(c.group.as_str());
            let on = n == highlight;
            let ink = theme.color(Semantic::Ink);
            let mut items = Vec::new();
            if let Some(icon) = c.icon {
                items.push(util::icon(cx, icon, 16.0, theme.color(Semantic::InkMuted)));
            }
            let label = util::label(cx, &c.label, TextStyle::Body, None, ink);
            items.push(cx.container(
                LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
                &[label],
                Paint::None,
            ));
            if let Some(s) = &c.shortcut {
                items.push(cx.child(&crate::Kbd::new(s.clone())));
            }
            let fill = if on { theme.color(Semantic::Accent).with_alpha(0.16) } else { Color::TRANSPARENT };
            let row = cx.container(
                LayoutStyle {
                    height: Some(38.0),
                    padding: Insets::symmetric(0.0, 10.0),
                    spacing: 10.0,
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &items,
                Paint::Quad(QuadPaint { corner_radius: 8.0, ..QuadPaint::new(fill) }),
            );
            let mut semantics = Semantics::new(Role::ListBoxOption).selected(on).active(on);
            if let Some(shortcut) = &c.shortcut {
                semantics = semantics.name(c.label.clone()).description(shortcut.clone());
            }
            cx.semantics(row, semantics);
            let (open, action, hl) = (self.palette.open.clone(), c.action.clone(), self.highlight.clone());
            cx.on_click(
                row,
                Rc::new(move || {
                    open.set(false);
                    action();
                }),
            );
            let _ = hl;
            cx.cursor(row, CursorIcon::Pointer);
            rows.push(row);
        }
        if matches.is_empty() {
            rows.push(util::label(
                cx,
                "No matching commands.",
                TextStyle::Callout,
                None,
                theme.color(Semantic::InkMuted),
            ));
        }
        cx.label_control(field, "Command palette", None);
        let card = panel(cx, &rows, 10.0, (viewport.width - 32.0).clamp(280.0, 560.0));
        cx.semantics(card, Semantics::new(Role::Dialog).name("Command palette").modal());
        let backdrop = cx.container(
            LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                padding: Insets { top: (viewport.height * 0.15).min(140.0), ..Insets::ZERO },
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[card],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Scrim))),
        );
        let close = self.palette.open.clone();
        cx.on_click(backdrop, Rc::new(move || close.set(false)));
        backdrop
    }
}

impl View for CommandPalette {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let query: State<String> = cx.use_state(String::new);
        let highlight: State<usize> = cx.use_state(|| 0);
        let toggle = self.open.clone();
        let (q, h) = (query.clone(), highlight.clone());
        cx.shortcut(
            Key::Character("k".into()),
            true,
            Rc::new(move || {
                q.set(String::new());
                h.set(0);
                toggle.update(|o| *o = !*o);
            }),
        );
        let anchor = cx.empty();
        if self.open.get() {
            let close = self.open.clone();
            let layer = PaletteLayer { palette: self, query, highlight };
            cx.overlay(anchor, Placement::Fill, &layer, Some(Rc::new(move || close.set(false))));
        }
        anchor
    }
}

/// A select with a search box: type to filter the options.
#[derive(Debug)]
pub struct Combobox {
    selected: Binding<Option<usize>>,
    options: Vec<String>,
    placeholder: String,
    width: f32,
    label: Option<String>,
}

impl Combobox {
    /// A combobox over `options`; `selected` holds the zero-based index of the choice, or `None`.
    pub fn new(
        selected: Binding<Option<usize>>,
        options: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            selected,
            options: options.into_iter().map(Into::into).collect(),
            placeholder: "Select…".into(),
            width: 240.0,
            label: None,
        }
    }

    /// Sets the text shown when nothing is selected.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Sets the width in logical pixels (240 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The name screen readers announce, such as "Framework". Inside a
    /// [`crate::FormField`], its label is used instead.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

struct ComboPanel<'a> {
    combo: &'a Combobox,
    query: State<String>,
    highlight: State<usize>,
    open: State<bool>,
}

impl View for ComboPanel<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let q = self.query.get();
        let matches: Vec<usize> =
            (0..self.combo.options.len()).filter(|&i| fuzzy_match(&self.combo.options[i], &q)).collect();
        let hl = self.highlight.get().min(matches.len().saturating_sub(1));
        let current = self.combo.selected.get();
        let pick = {
            let (sel, open) = (self.combo.selected.clone(), self.open.clone());
            Rc::new(move |i: usize| {
                sel.set(Some(i));
                open.set(false);
            })
        };
        let (h, m, p, open) =
            (self.highlight.clone(), Rc::new(matches.clone()), pick.clone(), self.open.clone());
        let field = cx.child(
            &TextField::new(self.query.binding())
                .placeholder("Search…")
                .icon(IconName::Search)
                .compact()
                .autofocus()
                .intercept_keys(Rc::new(move |key, _| {
                    match key {
                        Key::ArrowDown if !m.is_empty() => h.update(|x| *x = (*x + 1).min(m.len() - 1)),
                        Key::ArrowUp => h.update(|x| *x = x.saturating_sub(1)),
                        Key::Enter => {
                            if let Some(&i) = m.get(h.get().min(m.len().saturating_sub(1))) {
                                p(i);
                            }
                        }
                        Key::Escape => open.set(false),
                        _ => return false,
                    }
                    true
                })),
        );
        let mut rows = vec![field];
        for (n, &i) in matches.iter().enumerate().take(50) {
            let on = n == hl;
            let ink = theme.color(Semantic::Ink);
            let label = util::label(
                cx,
                &self.combo.options[i],
                TextStyle::Body,
                (current == Some(i)).then_some(Weight::SEMIBOLD),
                ink,
            );
            let mut items = vec![cx.container(
                LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
                &[label],
                Paint::None,
            )];
            if current == Some(i) {
                items.push(util::icon(
                    cx,
                    IconName::Check,
                    16.0,
                    theme.color(Semantic::Accent).lerp(ink, 0.3),
                ));
            }
            let fill = if on { ink.with_alpha(0.07) } else { Color::TRANSPARENT };
            let row = cx.container(
                LayoutStyle {
                    height: Some(34.0),
                    padding: Insets::symmetric(0.0, 10.0),
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &items,
                Paint::Quad(QuadPaint { corner_radius: 7.0, ..QuadPaint::new(fill) }),
            );
            cx.semantics(row, Semantics::new(Role::ListBoxOption).selected(current == Some(i)).active(on));
            let p = pick.clone();
            cx.on_click(row, Rc::new(move || p(i)));
            cx.cursor(row, CursorIcon::Pointer);
            rows.push(row);
        }
        if matches.is_empty() {
            rows.push(util::label(
                cx,
                "No results.",
                TextStyle::Callout,
                None,
                theme.color(Semantic::InkMuted),
            ));
        }
        let node = panel(cx, &rows, 6.0, self.combo.width);
        cx.semantics(node, Semantics::new(Role::ListBox));
        node
    }
}

impl View for Combobox {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let open: State<bool> = cx.use_state(|| false);
        let query: State<String> = cx.use_state(String::new);
        let highlight: State<usize> = cx.use_state(|| 0);
        let is_open = open.get();
        let hovered = cx.is_hovered();
        let (text, color) = match self.selected.get().and_then(|i| self.options.get(i)) {
            Some(t) => (t.clone(), theme.color(Semantic::Ink)),
            None => (self.placeholder.clone(), theme.color(Semantic::InkMuted)),
        };
        let label = util::label(cx, &text, TextStyle::Body, None, color);
        let label = cx.container(
            LayoutStyle { grow: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &[label],
            Paint::None,
        );
        let chevrons = util::icon(cx, IconName::ChevronDown, 16.0, theme.color(Semantic::InkMuted));
        let border = if is_open {
            Border { width: 1.5, color: theme.color(Semantic::Accent) }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base },
            }
        };
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(40.0),
                padding: Insets::symmetric(0.0, 12.0),
                spacing: 8.0,
                align: Align::Center,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[label, chevrons],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.md,
                border: Some(border),
                ..QuadPaint::new(theme.color(Semantic::Control))
            }),
        );
        cx.semantics(label, Semantics::hidden());
        let value = if self.selected.get().is_some() { text.clone() } else { String::new() };
        let mut semantics = Semantics::new(Role::ComboBox).value(value).expanded(is_open);
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        cx.semantics(node, semantics);
        let (o, q, h) = (open.clone(), query.clone(), highlight.clone());
        cx.on_click(
            node,
            Rc::new(move || {
                q.set(String::new());
                h.set(0);
                o.update(|x| *x = !*x);
            }),
        );
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Pointer);
        if is_open {
            let close = open.clone();
            let panel = ComboPanel { combo: self, query, highlight, open: open.clone() };
            cx.overlay(node, Placement::Below, &panel, Some(Rc::new(move || close.set(false))));
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use super::fuzzy_match;

    #[test]
    fn fuzzy_matching() {
        assert!(fuzzy_match("Toggle theme", "tgth"));
        assert!(fuzzy_match("Open settings", "open set"));
        assert!(!fuzzy_match("Export CSV", "xcz"));
        assert!(fuzzy_match("anything", ""));
    }
}
