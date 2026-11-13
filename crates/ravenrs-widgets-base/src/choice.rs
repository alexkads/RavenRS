//! Toggle buttons, toggle groups, pagination, labels and aspect-ratio boxes.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Key, Modifiers, PathFill, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{ButtonSize, ButtonStyle, IconButton, IconName, Tone, View, util};

/// A button that stays pressed: bold, italic, "pin", "mute".
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ToggleButton {
    value: Binding<bool>,
    label: String,
    icon: Option<IconName>,
    look: ToggleLook,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ToggleButton {
    /// A toggle button bound to `value`; clicking flips it.
    pub fn new(value: Binding<bool>, label: impl Into<String>) -> Self {
        Self { value, label: label.into(), icon: None, look: ToggleLook::default() }
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Shows how many things this button stands for after the label, as in a filter.
    pub fn count(mut self, count: usize) -> Self {
        self.look.count = Some(count);
        self
    }

    /// Fully rounded ends instead of the usual corners, for filter chips.
    pub fn pill(mut self) -> Self {
        self.look.pill = true;
        self
    }
}

/// The options of a toggle item that a toggle group does not use.
#[derive(Clone, Copy, Debug, Default)]
struct ToggleLook {
    count: Option<usize>,
    pill: bool,
}

/// The look shared by toggle buttons and toggle group items.
fn toggle_item(
    cx: &mut BuildCx<'_>,
    on: bool,
    icon: Option<IconName>,
    label: &str,
    look: ToggleLook,
    on_click: Rc<dyn Fn()>,
) -> NodeId {
    let theme = *cx.theme();
    let hovered = cx.is_hovered();
    let ink = theme.color(Semantic::Ink);
    let color = if on || hovered { ink } else { theme.color(Semantic::InkMuted) };
    let mut children = Vec::new();
    if let Some(i) = icon {
        children.push(util::icon(cx, i, 16.0, color));
    }
    if !label.is_empty() {
        children.push(util::label(cx, label, TextStyle::Callout, Some(Weight::SEMIBOLD), color));
    }
    if let Some(count) = look.count {
        let text = util::label(cx, &count.to_string(), TextStyle::Caption, Some(Weight::SEMIBOLD), color);
        let chip = cx.container(
            LayoutStyle { padding: Insets::symmetric(0.0, 6.0), ..LayoutStyle::stack(Axis::Horizontal) },
            &[text],
            Paint::Quad(QuadPaint { corner_radius: 999.0, ..QuadPaint::new(ink.with_alpha(0.1)) }),
        );
        // Announced as the value, so that the name stays the label.
        cx.semantics(chip, Semantics::hidden());
        children.push(chip);
    }
    let fill = if on {
        theme.color(Semantic::Control)
    } else if hovered {
        ink.with_alpha(0.06)
    } else {
        Color::TRANSPARENT
    };
    let border = on.then(|| Border { width: 1.0, color: theme.color(Semantic::Border) });
    let icon_only = label.is_empty() && look.count.is_none();
    let radius = if look.pill { 17.0 } else { 8.0 };
    let node = cx.container(
        LayoutStyle {
            height: Some(34.0),
            width: icon_only.then_some(34.0),
            padding: Insets::symmetric(0.0, if icon_only { 0.0 } else { 12.0 }),
            spacing: 6.0,
            align: Align::Center,
            justify: Justify::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        },
        &children,
        Paint::Quad(QuadPaint { corner_radius: radius, border, ..QuadPaint::new(fill) }),
    );
    let mut semantics = Semantics::new(Role::Button).toggled(on);
    if let Some(count) = look.count {
        semantics = semantics.value(count.to_string());
    }
    if icon_only && let Some(i) = icon {
        semantics = semantics.name(i.name().replace('-', " "));
        semantics.fallback_name = true;
    }
    cx.semantics(node, semantics);
    cx.on_click(node, on_click);
    cx.focusable(node, radius);
    cx.cursor(node, CursorIcon::Pointer);
    node
}

impl View for ToggleButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let v = self.value.clone();
        let toggle = Rc::new(move || v.update(|b| *b = !*b));
        toggle_item(cx, self.value.get(), self.icon, &self.label, self.look, toggle)
    }
}

/// A row of toggle buttons where one (or several) can be on. Arrow keys move
/// the focus between buttons; Space or Enter toggles the focused one.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ToggleGroup {
    items: Vec<(Option<IconName>, String)>,
    single: Option<Binding<usize>>,
    multiple: Option<Binding<BTreeSet<usize>>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ToggleGroup {
    /// Exactly one item is on (like text alignment).
    pub fn single(
        selected: Binding<usize>,
        items: impl IntoIterator<Item = (Option<IconName>, &'static str)>,
    ) -> Self {
        Self { items: collect(items), single: Some(selected), multiple: None }
    }

    /// Any number of items are on (like bold / italic / underline).
    pub fn multiple(
        selected: Binding<BTreeSet<usize>>,
        items: impl IntoIterator<Item = (Option<IconName>, &'static str)>,
    ) -> Self {
        Self { items: collect(items), single: None, multiple: Some(selected) }
    }
}

fn collect(
    items: impl IntoIterator<Item = (Option<IconName>, &'static str)>,
) -> Vec<(Option<IconName>, String)> {
    items.into_iter().map(|(i, l)| (i, l.to_owned())).collect()
}

struct GroupItem<'a> {
    index: usize,
    group: &'a ToggleGroup,
    roving: util::Roving,
}

impl View for GroupItem<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let g = self.group;
        let i = self.index;
        let (icon, label) = &g.items[i];
        let (on, action): (bool, Rc<dyn Fn()>) = match (&g.single, &g.multiple) {
            (Some(b), _) => {
                let b2 = b.clone();
                (b.get() == i, Rc::new(move || b2.set(i)))
            }
            (_, Some(b)) => {
                let b2 = b.clone();
                (
                    b.get().contains(&i),
                    Rc::new(move || {
                        b2.update(|s| {
                            if !s.remove(&i) {
                                s.insert(i);
                            }
                        })
                    }),
                )
            }
            _ => (false, Rc::new(|| {})),
        };
        let node = toggle_item(cx, on, *icon, label, ToggleLook::default(), action);
        // A toolbar: arrows move focus only; Space or Enter toggles.
        self.roving.wire(cx, node, None);
        node
    }
}

impl View for ToggleGroup {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (count, focus_to) = (self.items.len(), util::roving_cell(cx, None));
        let items: Vec<NodeId> = (0..count)
            .map(|index| {
                let roving = util::Roving { index, count, focus_to: focus_to.clone() };
                cx.keyed_child(index as u64, &GroupItem { index, group: self, roving })
            })
            .collect();
        let node = cx.container(
            LayoutStyle {
                spacing: 4.0,
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Toolbar));
        node
    }
}

/// Page navigation: Previous, numbered pages with ellipses, Next.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Pagination {
    page: Binding<usize>,
    pages: usize,
    previous: String,
    next: String,
    page_name: String,
    compact: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Pagination {
    /// `page` is zero-based; `pages` is the total count.
    pub fn new(page: Binding<usize>, pages: usize) -> Self {
        Self {
            page,
            pages: pages.max(1),
            previous: "Previous".into(),
            next: "Next".into(),
            page_name: "Page".into(),
            compact: false,
        }
    }

    /// Shows "1 / 12" between the buttons instead of the page numbers, so that the
    /// buttons stay in the same place whatever the number of pages.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// The text of the two buttons and the word screen readers say before a
    /// page number, for apps in other languages.
    pub fn labels(
        mut self,
        previous: impl Into<String>,
        next: impl Into<String>,
        page_name: impl Into<String>,
    ) -> Self {
        self.previous = previous.into();
        self.next = next.into();
        self.page_name = page_name.into();
        self
    }
}

/// Room for "123 / 456" in a compact [`Pagination`].
const COMPACT_PAGE_WIDTH: f32 = 72.0;

/// The page numbers to show (zero-based), with `None` for an ellipsis.
#[doc(hidden)]
pub fn page_window(current: usize, pages: usize) -> Vec<Option<usize>> {
    if pages <= 7 {
        return (0..pages).map(Some).collect();
    }
    let mut out = vec![Some(0)];
    let (lo, hi) = if current <= 3 {
        (1, 4)
    } else if current >= pages - 4 {
        (pages - 5, pages - 2)
    } else {
        (current - 1, current + 1)
    };
    if lo > 1 {
        out.push(None);
    }
    out.extend((lo..=hi).map(Some));
    if hi < pages - 2 {
        out.push(None);
    }
    out.push(Some(pages - 1));
    out
}

impl View for Pagination {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let current = self.page.get().min(self.pages - 1);
        let mut items = Vec::new();
        let prev = self.page.clone();
        items.push(
            cx.child(
                &crate::Button::new(self.previous.clone())
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .icon(IconName::ChevronLeft)
                    .disabled(current == 0)
                    .on_click(move || prev.update(|p| *p = p.saturating_sub(1))),
            ),
        );
        if self.compact {
            let color = theme.color(Semantic::Ink);
            let text = format!("{} / {}", current + 1, self.pages);
            let label = util::label(cx, &text, TextStyle::Callout, Some(Weight::MEDIUM), color);
            let node = cx.container(
                LayoutStyle {
                    width: Some(COMPACT_PAGE_WIDTH),
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[label],
                Paint::None,
            );
            cx.semantics(node, Semantics::new(Role::Label).name(format!("{} {text}", self.page_name)));
            items.push(node);
        }
        let window = if self.compact { Vec::new() } else { page_window(current, self.pages) };
        for (k, entry) in window.into_iter().enumerate() {
            match entry {
                Some(p) => {
                    let b = self.page.clone();
                    let style = if p == current { ButtonStyle::Secondary } else { ButtonStyle::Ghost };
                    let button = crate::Button::new((p + 1).to_string())
                        .accessible_name(format!("{} {}", self.page_name, p + 1))
                        .style(style)
                        .size(ButtonSize::Small)
                        .on_click(move || b.set(p));
                    let node = cx.keyed_child(p as u64, &button);
                    cx.update_semantics(node, |s| s.selected = Some(p == current));
                    items.push(node);
                }
                None => {
                    let dots = util::icon(cx, IconName::Ellipsis, 16.0, theme.color(Semantic::InkMuted));
                    items.push(cx.container(
                        LayoutStyle {
                            width: Some(28.0),
                            justify: Justify::Center,
                            ..LayoutStyle::stack(Axis::Horizontal)
                        },
                        &[dots],
                        Paint::None,
                    ));
                    let _ = k;
                }
            }
        }
        let (next, last) = (self.page.clone(), self.pages - 1);
        items.push(
            cx.child(
                &crate::Button::new(self.next.clone())
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .trailing_icon(IconName::ChevronRight)
                    .disabled(current == last)
                    .on_click(move || next.update(|p| *p = (*p + 1).min(last))),
            ),
        );
        let node = cx.container(
            LayoutStyle {
                spacing: 4.0,
                align: Align::Center,
                wrap: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Group).name("Pagination"));
        node
    }
}

/// A round color to pick, from a palette of [`ColorSwatch`]es. Its hit target is
/// 32 px; the color itself is 20 px.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ColorSwatch {
    color: Option<Color>,
    label: String,
    selected: bool,
    on_click: Option<Rc<dyn Fn()>>,
}

impl std::fmt::Debug for ColorSwatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorSwatch").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ColorSwatch {
    /// A swatch of `color`. `label` is what screen readers announce, such as "Accent".
    pub fn new(color: Color, label: impl Into<String>) -> Self {
        Self { color: Some(color), label: label.into(), selected: false, on_click: None }
    }

    /// The "no color" choice, drawn empty with a bar across it in `danger`.
    pub fn none(label: impl Into<String>) -> Self {
        Self { color: None, label: label.into(), selected: false, on_click: None }
    }

    /// Marks the swatch as the current choice.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Runs `handler` when the swatch is clicked, or on Enter and Space while focused.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl View for ColorSwatch {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (color, selected) = (self.color, self.selected);
        let ring = theme.color(if selected { Semantic::Accent } else { Semantic::Border });
        let strike = theme.color(Semantic::Danger);
        let dot = cx.container(
            LayoutStyle { width: Some(20.0), height: Some(20.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                let fill = color.unwrap_or(Color::TRANSPARENT);
                scene.push_quad(
                    Quad::new(f, fill).radius(10.0).border(if selected { 2.5 } else { 1.0 }, ring),
                );
                if color.is_none() {
                    let bar = Rect::new(f.x() + 3.0, f.center().y - 0.75, f.width() - 6.0, 1.5);
                    scene.push_quad(Quad::new(bar, strike));
                }
            })),
        );
        let fill =
            if cx.is_hovered() { theme.color(Semantic::Ink).with_alpha(0.06) } else { Color::TRANSPARENT };
        let node = cx.container(
            LayoutStyle {
                width: Some(32.0),
                height: Some(32.0),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[dot],
            Paint::Quad(QuadPaint { corner_radius: theme.radii.full, ..QuadPaint::new(fill) }),
        );
        cx.semantics(node, Semantics::new(Role::Button).name(self.label.clone()).selected(selected));
        if let Some(handler) = &self.on_click {
            cx.on_click(node, handler.clone());
        }
        cx.focusable(node, theme.radii.full);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// A heart that marks something as a favorite: filled when on, an outline when off.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::FavoriteToggle;
///
/// let liked = State::new(false);
/// let heart = FavoriteToggle::new(liked.binding()).label("Warm portrait");
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FavoriteToggle {
    value: Binding<bool>,
    label: Option<String>,
    star: bool,
    size: f32,
    color: Option<Color>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FavoriteToggle {
    /// A heart bound to `value`; clicking flips it.
    pub fn new(value: Binding<bool>) -> Self {
        Self { value, label: None, star: false, size: 16.0, color: None }
    }

    /// What it marks, for screen readers, instead of the word in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A star instead of a heart.
    pub fn star(mut self) -> Self {
        self.star = true;
        self
    }

    /// Sets the size of the mark in logical pixels (16 by default). The target around it follows.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Sets the color when on (the danger tone for a heart, the warning tone for a star).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl View for FavoriteToggle {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let on = self.value.get();
        let hovered = cx.is_hovered();
        let (size, star) = (self.size, self.star);
        let mark = if on {
            let tone = if star { Tone::Warning } else { Tone::Danger };
            let color = self.color.unwrap_or_else(|| tone.color(&theme));
            cx.container(
                LayoutStyle { width: Some(size), height: Some(size), ..LayoutStyle::leaf() },
                &[],
                Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                    let path = if star {
                        util::star_path(frame.center(), size * 0.5)
                    } else {
                        util::heart_path(frame)
                    };
                    scene.push_path(PathFill::solid(path, color));
                })),
            )
        } else {
            let ink = theme.color(if hovered { Semantic::Ink } else { Semantic::InkMuted });
            let glyph = util::icon(cx, if star { IconName::Star } else { IconName::Heart }, size, ink);
            cx.semantics(glyph, Semantics::hidden());
            glyph
        };
        let side = size + 3.0 * theme.spacing.s1;
        let fill = if hovered { theme.color(Semantic::Ink).with_alpha(0.06) } else { Color::TRANSPARENT };
        let node = cx.container(
            LayoutStyle {
                width: Some(side),
                height: Some(side),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[mark],
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, ..QuadPaint::new(fill) }),
        );
        let name = self.label.clone().unwrap_or_else(|| cx.labels().favorite.clone());
        cx.semantics(node, Semantics::new(Role::Button).name(name).toggled(on));
        let value = self.value.clone();
        cx.on_click(node, Rc::new(move || value.update(|v| *v = !*v)));
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// A grid of [`ColorSwatch`]es. The arrow keys walk through it, and the handler gets the keys
/// held with the click, so that a modifier can mean something else, such as "for the background".
///
/// ```
/// use ravenrs_core::Color;
/// use ravenrs_widgets::SwatchGrid;
///
/// let grid = SwatchGrid::new([(Color::hex(0xe11d48), "Rose"), (Color::hex(0x2563eb), "Blue")])
///     .columns(8)
///     .selected(Some(Color::hex(0x2563eb)))
///     .on_pick(|color, modifiers| {
///         let for_background = modifiers.meta || modifiers.ctrl;
///         # let _ = (color, for_background);
///     });
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SwatchGrid {
    colors: Vec<(Color, String)>,
    selected: Option<Color>,
    columns: usize,
    label: Option<String>,
    on_pick: Option<Rc<dyn Fn(Color, Modifiers)>>,
}

impl std::fmt::Debug for SwatchGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwatchGrid").field("colors", &self.colors.len()).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SwatchGrid {
    /// A grid of these colors, each with the name screen readers announce.
    pub fn new<S: Into<String>>(colors: impl IntoIterator<Item = (Color, S)>) -> Self {
        Self {
            colors: colors.into_iter().map(|(color, name)| (color, name.into())).collect(),
            selected: None,
            columns: 8,
            label: None,
            on_pick: None,
        }
    }

    /// Marks the swatch of this color as the current one.
    pub fn selected(mut self, color: Option<Color>) -> Self {
        self.selected = color;
        self
    }

    /// Sets how many swatches go in a row (8 by default).
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// What the colors are for, for screen readers, instead of the word in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Runs `handler` with the color and the keys held, on a click or on Enter and Space.
    pub fn on_pick(mut self, handler: impl Fn(Color, Modifiers) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

/// Where an arrow key leads from `index`, in a grid of `count` items in rows of `columns`.
fn grid_step(key: &Key, index: usize, count: usize, columns: usize) -> Option<usize> {
    let target = match key {
        Key::ArrowRight => index + 1,
        Key::ArrowLeft => index.checked_sub(1)?,
        Key::ArrowDown => index + columns,
        Key::ArrowUp => index.checked_sub(columns)?,
        Key::Home => 0,
        Key::End => count.checked_sub(1)?,
        _ => return None,
    };
    (target < count).then_some(target)
}

impl View for SwatchGrid {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let focus_to = util::roving_cell(cx, None);
        let held = cx.modifiers();
        let (count, columns) = (self.colors.len(), self.columns);
        let mut rows = Vec::with_capacity(count.div_ceil(columns));
        let mut row = Vec::with_capacity(columns);
        for (index, (color, name)) in self.colors.iter().enumerate() {
            let mut swatch = ColorSwatch::new(*color, name.clone()).selected(self.selected == Some(*color));
            if let Some(on_pick) = &self.on_pick {
                let (on_pick, held, color) = (on_pick.clone(), held.clone(), *color);
                swatch = swatch.on_click(move || on_pick(color, held.get()));
            }
            let node = cx.child(&swatch);
            if focus_to.take(index) {
                cx.request_focus_node(node);
            }
            let focus_to = focus_to.clone();
            cx.on_key(
                node,
                Rc::new(move |key, _| match grid_step(key, index, count, columns) {
                    Some(target) => {
                        focus_to.request(target);
                        true
                    }
                    // At an edge the key is used up all the same, so the page does not scroll.
                    None => matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown),
                }),
            );
            row.push(node);
            if row.len() == columns || index + 1 == count {
                rows.push(cx.container(LayoutStyle::stack(Axis::Horizontal), &row, Paint::None));
                row.clear();
            }
        }
        let node = cx.container(
            LayoutStyle { align_self: Some(Align::Fit), ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| cx.labels().swatches.clone());
        cx.semantics(node, Semantics::new(Role::Group).name(name));
        node
    }
}

/// One of the nine points of a [`NinePointAnchor`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum AnchorPoint {
    /// The top left corner.
    TopLeft,
    /// The middle of the top edge.
    Top,
    /// The top right corner.
    TopRight,
    /// The middle of the left edge.
    Left,
    /// The middle.
    #[default]
    Center,
    /// The middle of the right edge.
    Right,
    /// The bottom left corner.
    BottomLeft,
    /// The middle of the bottom edge.
    Bottom,
    /// The bottom right corner.
    BottomRight,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl AnchorPoint {
    /// All nine, row by row from the top left.
    pub const ALL: [Self; 9] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Left,
        Self::Center,
        Self::Right,
        Self::BottomLeft,
        Self::Bottom,
        Self::BottomRight,
    ];

    /// Where it falls in a box, as a share of the width and of the height.
    ///
    /// ```
    /// use ravenrs_widgets::AnchorPoint;
    ///
    /// assert_eq!(AnchorPoint::Right.fraction(), (1.0, 0.5));
    /// ```
    pub fn fraction(self) -> (f32, f32) {
        let index = self as usize;
        ((index % 3) as f32 / 2.0, (index / 3) as f32 / 2.0)
    }

    /// The point nearest to these shares of a box.
    pub fn from_fraction(x: f32, y: f32) -> Self {
        let cell = |share: f32| (share.clamp(0.0, 1.0) * 2.0).round() as usize;
        Self::ALL[cell(y) * 3 + cell(x)]
    }
}

/// A three by three grid for choosing a point of reference on a box. The arrow keys move it.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{AnchorPoint, NinePointAnchor};
///
/// let point = State::new(AnchorPoint::Center);
/// let picker = NinePointAnchor::new(point.binding());
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct NinePointAnchor {
    value: Binding<AnchorPoint>,
    cell: f32,
    label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl NinePointAnchor {
    /// A grid bound to `value`.
    pub fn new(value: Binding<AnchorPoint>) -> Self {
        Self { value, cell: 10.0, label: None }
    }

    /// Sets the side of a square in logical pixels (10 by default).
    pub fn cell_size(mut self, size: f32) -> Self {
        self.cell = size;
        self
    }

    /// What it is for, for screen readers, instead of the words in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// One square of a [`NinePointAnchor`]. The grid as a whole takes the focus, so the keyboard
/// moves the value and not a cursor over the squares.
struct AnchorCell {
    point: AnchorPoint,
    on: bool,
    size: f32,
    name: String,
    value: Binding<AnchorPoint>,
}

impl View for AnchorCell {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let ink = theme.color(if self.on || cx.is_hovered() { Semantic::Ink } else { Semantic::InkMuted });
        let node = cx.container(
            LayoutStyle { width: Some(self.size), height: Some(self.size), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint {
                border: Some(Border { width: 1.0, color: ink }),
                ..QuadPaint::new(if self.on { ink } else { Color::TRANSPARENT })
            }),
        );
        cx.semantics(
            node,
            Semantics::new(Role::RadioButton).name(self.name.clone()).selected(self.on).toggled(self.on),
        );
        let (value, point) = (self.value.clone(), self.point);
        cx.on_click(node, Rc::new(move || value.set(point)));
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

impl View for NinePointAnchor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let current = self.value.get();
        let mut rows = Vec::with_capacity(3);
        for row in AnchorPoint::ALL.chunks(3) {
            let cells: Vec<NodeId> = row
                .iter()
                .map(|&point| {
                    cx.child(&AnchorCell {
                        point,
                        on: point == current,
                        size: self.cell,
                        name: labels.anchor_names[point as usize].clone(),
                        value: self.value.clone(),
                    })
                })
                .collect();
            rows.push(cx.container(
                LayoutStyle { spacing: 1.0, ..LayoutStyle::stack(Axis::Horizontal) },
                &cells,
                Paint::None,
            ));
        }
        let node = cx.container(
            LayoutStyle { spacing: 1.0, align_self: Some(Align::Fit), ..LayoutStyle::stack(Axis::Vertical) },
            &rows,
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.anchor_point.clone());
        cx.semantics(
            node,
            Semantics::new(Role::RadioGroup).name(name).value(labels.anchor_names[current as usize].clone()),
        );
        let value = self.value.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                if !matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown) {
                    return false;
                }
                let index = value.get() as usize;
                // Stays put at an edge: on a grid of positions, going round would be a jump.
                if let Some(target) = grid_step(key, index, 9, 3).filter(|target| match key {
                    Key::ArrowLeft | Key::ArrowRight => target / 3 == index / 3,
                    _ => true,
                }) {
                    value.set(AnchorPoint::ALL[target]);
                }
                true
            }),
        );
        cx.focusable(node, theme.radii.sm);
        node
    }
}

/// A foreground and a background color, one over the other, with a button that swaps them.
///
/// It holds the two colors and swaps them. Choosing a new one is up to whoever uses it, as
/// when a click on a square opens a [`ColorPicker`](crate::ColorPicker).
///
/// ```
/// use ravenrs_core::Color;
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::ColorPair;
///
/// let (front, back) = (State::new(Color::BLACK), State::new(Color::WHITE));
/// let pair = ColorPair::new(front.binding(), back.binding())
///     .defaults(Color::BLACK, Color::WHITE)
///     .on_foreground(|| { /* open a picker */ });
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ColorPair {
    foreground: Binding<Color>,
    background: Binding<Color>,
    defaults: Option<(Color, Color)>,
    on_foreground: Option<Rc<dyn Fn()>>,
    on_background: Option<Rc<dyn Fn()>>,
    size: f32,
}

impl std::fmt::Debug for ColorPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorPair").field("size", &self.size).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ColorPair {
    /// A pair bound to these two colors.
    pub fn new(foreground: Binding<Color>, background: Binding<Color>) -> Self {
        Self { foreground, background, defaults: None, on_foreground: None, on_background: None, size: 24.0 }
    }

    /// Adds a button that puts these colors back.
    pub fn defaults(mut self, foreground: Color, background: Color) -> Self {
        self.defaults = Some((foreground, background));
        self
    }

    /// Runs `handler` when the foreground square is clicked, or on Enter and Space while focused.
    pub fn on_foreground(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_foreground = Some(Rc::new(handler));
        self
    }

    /// Runs `handler` when the background square is clicked, or on Enter and Space while focused.
    pub fn on_background(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_background = Some(Rc::new(handler));
        self
    }

    /// Sets the side of each square in logical pixels (24 by default).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

/// One square of a [`ColorPair`]. Its color is also said in words, and without a handler it
/// only shows the color.
struct PairSquare {
    color: Color,
    name: String,
    at: (f32, f32),
    size: f32,
    on_click: Option<Rc<dyn Fn()>>,
}

impl View for PairSquare {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let node = cx.container(
            LayoutStyle {
                absolute: Some(self.at),
                width: Some(self.size),
                height: Some(self.size),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(self.color)
            }),
        );
        let role = if self.on_click.is_some() { Role::Button } else { Role::Image };
        let spoken = crate::util::to_hex(self.color);
        cx.semantics(node, Semantics::new(role).name(self.name.clone()).value(spoken));
        if let Some(handler) = &self.on_click {
            cx.on_click(node, handler.clone());
            cx.focusable(node, theme.radii.sm);
            cx.cursor(node, CursorIcon::Pointer);
        }
        node
    }
}

impl View for ColorPair {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let (front, back) = (self.foreground.get(), self.background.get());
        let offset = (self.size / 2.0).round();
        // The background first, so that it stays under the foreground.
        let lower = cx.child(&PairSquare {
            color: back,
            name: labels.color_background.clone(),
            at: (offset, offset),
            size: self.size,
            on_click: self.on_background.clone(),
        });
        let upper = cx.child(&PairSquare {
            color: front,
            name: labels.color_foreground.clone(),
            at: (0.0, 0.0),
            size: self.size,
            on_click: self.on_foreground.clone(),
        });
        let side = self.size + offset;
        let squares = cx.container(
            LayoutStyle { width: Some(side), height: Some(side), ..LayoutStyle::stack(Axis::Horizontal) },
            &[lower, upper],
            Paint::None,
        );
        let (foreground, background) = (self.foreground.clone(), self.background.clone());
        let swap = cx.child(
            &IconButton::new(IconName::ArrowUpDown)
                .label(labels.color_swap.clone())
                .size(ButtonSize::Mini)
                .on_click(move || {
                    let front = foreground.get();
                    foreground.set(background.get());
                    background.set(front);
                }),
        );
        let mut buttons = vec![swap];
        if let Some((default_front, default_back)) = self.defaults {
            let (foreground, background) = (self.foreground.clone(), self.background.clone());
            buttons.push(
                cx.child(
                    &IconButton::new(IconName::RotateCcw)
                        .label(labels.color_reset.clone())
                        .size(ButtonSize::Mini)
                        .on_click(move || {
                            foreground.set(default_front);
                            background.set(default_back);
                        }),
                ),
            );
        }
        let buttons = cx.container(
            LayoutStyle { spacing: theme.spacing.s1, ..LayoutStyle::stack(Axis::Vertical) },
            &buttons,
            Paint::None,
        );
        cx.container(
            LayoutStyle {
                spacing: theme.spacing.s1,
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[squares, buttons],
            Paint::None,
        )
    }
}

/// A form label with an optional required marker and hint.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Label {
    text: String,
    required: bool,
    hint: Option<String>,
    ink: Option<Semantic>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Label {
    /// A label with this text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), required: false, hint: None, ink: None }
    }

    /// The label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Adds a red `*` after the text.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// A muted caption under the text.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The color of the text, instead of the usual one.
    pub fn ink(mut self, ink: Semantic) -> Self {
        self.ink = Some(ink);
        self
    }
}

impl View for Label {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut row = vec![util::label(
            cx,
            &self.text,
            TextStyle::Callout,
            Some(Weight::SEMIBOLD),
            theme.color(self.ink.unwrap_or(Semantic::Ink)),
        )];
        if self.required {
            row.push(util::label(
                cx,
                "*",
                TextStyle::Callout,
                Some(Weight::SEMIBOLD),
                theme.color(Semantic::Danger),
            ));
        }
        let row = cx.container(
            LayoutStyle { spacing: 3.0, ..LayoutStyle::stack(Axis::Horizontal) },
            &row,
            Paint::None,
        );
        let mut children = vec![row];
        if let Some(h) = &self.hint {
            children.push(util::label(cx, h, TextStyle::Caption, None, theme.color(Semantic::InkMuted)));
        }
        cx.container(
            LayoutStyle { spacing: 2.0, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

/// Keeps its content at a width-to-height ratio while filling the width.
#[derive(Debug)]
pub struct AspectRatio<V> {
    ratio: f32,
    content: V,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<V: View> AspectRatio<V> {
    /// Sizes `content` to `ratio` = width / height (e.g. `16.0 / 9.0`); at least 0.01.
    pub fn new(ratio: f32, content: V) -> Self {
        Self { ratio: ratio.max(0.01), content }
    }
}

impl<V: View> View for AspectRatio<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        cx.container(
            LayoutStyle {
                aspect_ratio: Some(self.ratio),
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                // A one-cell grid stretches the content over both axes, so any
                // view (an image, a styled box) fills the ratio box.
                grid_columns: Some(1),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[child],
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::page_window;

    #[test]
    fn page_window_keeps_ends_and_neighbors() {
        let show = |c, n| {
            page_window(c, n)
                .iter()
                .map(|p| p.map_or("…".into(), |p: usize| (p + 1).to_string()))
                .collect::<Vec<String>>()
                .join(" ")
        };
        assert_eq!(show(0, 5), "1 2 3 4 5");
        assert_eq!(show(0, 20), "1 2 3 4 5 … 20");
        assert_eq!(show(9, 20), "1 … 9 10 11 … 20");
        assert_eq!(show(19, 20), "1 … 16 17 18 19 20");
    }
}
