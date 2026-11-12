use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Effect, Insets, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{IconName, Motion, View, util};

/// Visual emphasis of a [`Button`] or other control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ButtonStyle {
    /// Accent fill; one per view.
    #[default]
    Primary,
    /// Control fill with a hairline, for secondary actions.
    Secondary,
    /// No fill until hovered; toolbars and low-emphasis actions.
    Ghost,
    /// Destructive actions.
    Danger,
}

/// Height and horizontal padding of a [`Button`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ButtonSize {
    /// 32 px tall, smaller text; dense toolbars and table rows.
    Small,
    /// 40 px tall; the default.
    #[default]
    Medium,
    /// 48 px tall; prominent calls to action.
    Large,
    /// 22 px tall, caption text; a [`StatusBar`](crate::StatusBar)'s actions.
    Mini,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ButtonSize {
    pub(crate) fn height(self) -> f32 {
        match self {
            Self::Mini => 22.0,
            Self::Small => 32.0,
            Self::Medium => 40.0,
            Self::Large => 48.0,
        }
    }

    /// The text of a control of this size.
    pub(crate) fn text_style(self) -> TextStyle {
        match self {
            Self::Mini => TextStyle::Caption,
            Self::Small => TextStyle::Callout,
            _ => TextStyle::Body,
        }
    }

    fn padding_x(self) -> f32 {
        match self {
            Self::Mini => 8.0,
            Self::Small => 12.0,
            Self::Medium => 18.0,
            Self::Large => 24.0,
        }
    }
}

/// Fill, label color and border of a control for a style and interaction state.
pub(crate) fn control_colors(
    theme: &Theme,
    style: ButtonStyle,
    hovered: bool,
    pressed: bool,
    disabled: bool,
) -> (Color, Color, Option<Border>) {
    let ink = theme.color(Semantic::Ink);
    let (fill, label, border) = match style {
        ButtonStyle::Primary => {
            let fill = if disabled {
                Semantic::Accent
            } else if pressed {
                Semantic::AccentPressed
            } else if hovered {
                Semantic::AccentHover
            } else {
                Semantic::Accent
            };
            (theme.color(fill), theme.color(Semantic::OnAccent), None)
        }
        ButtonStyle::Secondary => {
            let base = theme.color(Semantic::Control);
            let fill = if disabled {
                base
            } else if pressed {
                base.lerp(ink, 0.12)
            } else if hovered {
                base.lerp(ink, 0.06)
            } else {
                base
            };
            (fill, ink, Some(Border { width: 1.0, color: theme.color(Semantic::Border) }))
        }
        ButtonStyle::Ghost => {
            let fill = if disabled {
                Color::TRANSPARENT
            } else if pressed {
                ink.with_alpha(0.12)
            } else if hovered {
                ink.with_alpha(0.07)
            } else {
                Color::TRANSPARENT
            };
            (fill, ink, None)
        }
        ButtonStyle::Danger => {
            let base = theme.color(Semantic::Danger);
            let fill = if disabled {
                base
            } else if pressed {
                base.lerp(Color::BLACK, 0.18)
            } else if hovered {
                base.lerp(Color::WHITE, 0.08)
            } else {
                base
            };
            (fill, Color::WHITE, None)
        }
    };
    if disabled {
        (crate::util::faded(fill), crate::util::faded(label), border)
    } else {
        (fill, label, border)
    }
}

/// How a control that stays on looks, in any style.
fn selected_colors(
    theme: &Theme,
    style: ButtonStyle,
    hovered: bool,
    disabled: bool,
) -> (Color, Color, Border) {
    let (ink, accent) = (theme.color(Semantic::Ink), theme.color(Semantic::Accent));
    let (fill, label, border) = match style {
        ButtonStyle::Primary => {
            let on_accent = theme.color(Semantic::OnAccent);
            (theme.color(Semantic::AccentPressed), on_accent, on_accent.with_alpha(0.6))
        }
        ButtonStyle::Danger => {
            let fill = theme.color(Semantic::Danger).lerp(Color::BLACK, 0.18);
            (fill, Color::WHITE, Color::WHITE.with_alpha(0.6))
        }
        // A tint that follows the theme, so the two neutral styles light up the same way.
        _ => (accent.with_alpha(0.18), accent.lerp(ink, 0.3), accent.with_alpha(0.55)),
    };
    let fill = if hovered && !disabled { fill.lerp(ink, 0.06) } else { fill };
    if disabled {
        (util::faded(fill), util::faded(label), Border { width: 1.0, color: util::faded(border) })
    } else {
        (fill, label, Border { width: 1.0, color: border })
    }
}

/// A clickable, focusable button. Activates on click, Enter or Space.
///
/// ```
/// use std::cell::Cell;
/// use std::rc::Rc;
/// use ravenrs_widgets::{Button, ButtonStyle, IconName, Ui};
///
/// let saved = Rc::new(Cell::new(false));
/// let flag = saved.clone();
/// let button = Button::new("Save")
///     .icon(IconName::Check)
///     .style(ButtonStyle::Secondary)
///     .on_click(move || flag.set(true));
/// let mut ui = Ui::new(button, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Button {
    label: String,
    on_click: Option<Rc<dyn Fn()>>,
    style: ButtonStyle,
    size: ButtonSize,
    icon: Option<IconName>,
    trailing_icon: Option<IconName>,
    disabled: bool,
    full_width: bool,
    accessible_name: Option<String>,
    accessible_description: Option<String>,
    loading: bool,
    selected: Option<bool>,
    shortcut: Option<String>,
    join: Option<Join>,
}

/// Where a button stands in a [`ButtonGroup`].
#[derive(Clone, Copy, Debug)]
struct Join {
    first: bool,
    last: bool,
}

impl fmt::Debug for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Button").field("label", &self.label).field("style", &self.style).finish()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Button {
    /// A primary, medium button with this label. An empty label makes it icon-only.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
            style: ButtonStyle::Primary,
            size: ButtonSize::Medium,
            icon: None,
            trailing_icon: None,
            disabled: false,
            full_width: false,
            accessible_name: None,
            accessible_description: None,
            loading: false,
            selected: None,
            shortcut: None,
            join: None,
        }
    }

    /// Sets the name screen readers announce, instead of the label. Icon-only
    /// buttons should always have one.
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// Sets what screen readers say after its name, such as what it does. Its shortcut and
    /// whether it is busy follow.
    pub fn accessible_description(mut self, text: impl Into<String>) -> Self {
        self.accessible_description = Some(text.into());
        self
    }

    /// Runs `handler` when the button is clicked or activated with Enter or Space.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Sets the visual emphasis (`Primary` by default).
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets the height and padding (`Medium` by default).
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// An icon after the label (e.g. a chevron for menus).
    pub fn trailing_icon(mut self, icon: IconName) -> Self {
        self.trailing_icon = Some(icon);
        self
    }

    /// Greys the button out and ignores clicks and focus.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Stretches across the parent stack.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Shows an animated indicator over the content, which keeps its place so that the
    /// button does not change width, and ignores clicks until it is done. It keeps
    /// keyboard focus, and screen readers hear that it is loading and unavailable.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Lights the button up as being on, and announces it as a button that toggles.
    /// What a click does is still up to [`Self::on_click`].
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Shows a key cap after the label, in the label's color, such as `"F8"`. It only
    /// shows the key: [`BuildCx::shortcut`] makes the key work.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }
}

impl View for Button {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let inert = self.disabled || self.loading;
        let pressed = cx.is_pressed() && !inert;
        let hovered = cx.is_hovered() && !inert;
        let (fill, label_color, border) = if self.selected == Some(true) {
            let (fill, label, border) = selected_colors(&theme, self.style, hovered, self.disabled);
            (fill, label, Some(border))
        } else {
            control_colors(&theme, self.style, hovered, pressed, self.disabled)
        };
        // While loading, the content is still laid out, unseen, to hold the width.
        let spinner_color = label_color;
        let label_color = if self.loading { label_color.with_alpha(0.0) } else { label_color };
        let icon_size = match self.size {
            ButtonSize::Mini => 12.0,
            ButtonSize::Small => 14.0,
            _ => 16.0,
        };
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, icon_size, label_color));
        }
        if !self.label.is_empty() {
            let style = self.size.text_style();
            children.push(util::label(cx, &self.label, style, Some(Weight::SEMIBOLD), label_color));
        }
        if let Some(icon) = self.trailing_icon {
            children.push(util::icon(cx, icon, icon_size, label_color));
        }
        if let Some(keys) = &self.shortcut {
            let cap = crate::Kbd::new(keys.clone()).inherit_color();
            let cap = cx.with_foreground(label_color, |cx| cx.child(&cap));
            // Part of the description, not of the name.
            cx.semantics(cap, Semantics::hidden());
            children.push(cap);
        }
        let height = self.size.height();
        let icon_only = self.label.is_empty();
        let spacing = if self.size == ButtonSize::Mini { 4.0 } else { 8.0 };
        if self.loading {
            let content = cx.container(
                LayoutStyle {
                    spacing,
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &children,
                crate::extras::spinner_paint(icon_size, spinner_color),
            );
            children = vec![content];
        }
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, if icon_only { 0.0 } else { self.size.padding_x() }),
            width: icon_only.then_some(height),
            height: Some(height),
            spacing,
            align: Align::Center,
            justify: Justify::Center,
            align_self: Some(if self.full_width { Align::Stretch } else { Align::Fit }),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let radius = theme.radii.md;
        let node = if let Some(join) = self.join {
            // No squeeze when pressed, which would open a gap between neighbors.
            cx.container(style, &children, joined_paint(join, fill, radius, border))
        } else {
            let paint =
                Paint::Quad(QuadPaint { fill: fill.into(), corner_radius: radius, border, shadow: None });
            let node = cx.container(style, &children, paint);
            let squeeze = if pressed { Effect::scaled(0.97) } else { Effect::IDENTITY };
            cx.motion(node, Motion::new().paint(theme.motion.fast).effect(squeeze, theme.motion.snappy));
            node
        };
        let mut semantics = Semantics::new(Role::Button).disabled(inert);
        if let Some(selected) = self.selected {
            semantics = semantics.toggled(selected);
        }
        // The accessibility tree has no "busy" state: the word carries it.
        let busy = self.loading.then(|| cx.labels().loading.clone());
        let parts: Vec<&str> =
            [self.accessible_description.as_deref(), busy.as_deref(), self.shortcut.as_deref()]
                .into_iter()
                .flatten()
                .collect();
        let description = (!parts.is_empty()).then(|| parts.join(". "));
        if let Some(description) = description {
            semantics = semantics.description(description);
        }
        if let Some(name) = &self.accessible_name {
            semantics = semantics.name(name.clone());
        } else if let (true, Some(icon)) = (icon_only, self.icon) {
            // A guess a surrounding tooltip replaces.
            semantics = semantics.name(icon.name().replace('-', " "));
            semantics.fallback_name = true;
        }
        cx.semantics(node, semantics);
        if !self.disabled {
            if let (Some(handler), false) = (&self.on_click, self.loading) {
                cx.on_click(node, handler.clone());
            }
            cx.focusable(node, radius);
            if !self.loading {
                cx.cursor(node, CursorIcon::Pointer);
            }
        }
        node
    }
}

/// A button in a group: rounded only on the group's outer corners, and over the edge of the
/// one before it, so that a line between two is one line.
fn joined_paint(join: Join, fill: Color, radius: f32, border: Option<Border>) -> Paint {
    let (left, right) = (if join.first { radius } else { 0.0 }, if join.last { radius } else { 0.0 });
    let overlap = if join.first || border.is_none() { 0.0 } else { border.map_or(0.0, |b| b.width) };
    Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
        let rect = Rect::new(frame.x() - overlap, frame.y(), frame.width() + overlap, frame.height());
        let mut quad = Quad::new(rect, fill);
        quad.corner_radii = [left, right, right, left];
        quad.border = border;
        scene.push_quad(quad);
    }))
}

/// Buttons glued side by side, as one control. Each one stays a whole [`Button`], with its own
/// style, icon, state and handler; marking one as chosen is up to whoever makes them.
///
/// ```
/// use ravenrs_widgets::{Button, ButtonGroup, ButtonStyle};
///
/// let group = ButtonGroup::new([
///     Button::new("Day"),
///     Button::new("Week").style(ButtonStyle::Secondary),
///     Button::new("Month").style(ButtonStyle::Secondary),
/// ])
/// .label("Period");
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ButtonGroup {
    buttons: Vec<Button>,
    label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ButtonGroup {
    /// Glues these buttons, in order.
    pub fn new(buttons: impl IntoIterator<Item = Button>) -> Self {
        let mut buttons: Vec<Button> = buttons.into_iter().collect();
        let last = buttons.len().saturating_sub(1);
        for (index, button) in buttons.iter_mut().enumerate() {
            button.join = Some(Join { first: index == 0, last: index == last });
        }
        Self { buttons, label: None }
    }

    /// What the buttons are about, for screen readers.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl View for ButtonGroup {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let buttons: Vec<NodeId> = self.buttons.iter().map(|button| cx.child(button)).collect();
        let node = cx.container(
            LayoutStyle {
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &buttons,
            Paint::None,
        );
        if let Some(label) = &self.label {
            cx.semantics(node, Semantics::new(Role::Group).name(label.clone()));
        }
        node
    }
}

/// A square, icon-only button (ghost by default), for toolbars.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct IconButton {
    button: Button,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl IconButton {
    /// A ghost icon button showing `icon`.
    pub fn new(icon: IconName) -> Self {
        Self { button: Button::new("").icon(icon).style(ButtonStyle::Ghost) }
    }

    /// Runs `handler` when the button is clicked or activated with Enter or Space.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.button = self.button.on_click(handler);
        self
    }

    /// Makes it inert and dimmed.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.button = self.button.disabled(disabled);
        self
    }

    /// Sets the name screen readers announce, such as "Search". Without it,
    /// the icon's name is used, which rarely describes the action well.
    pub fn label(mut self, name: impl Into<String>) -> Self {
        self.button = self.button.accessible_name(name);
        self
    }

    /// Sets what screen readers say after its name.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.button = self.button.accessible_description(text);
        self
    }

    /// Sets the visual emphasis (`Ghost` by default).
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.button = self.button.style(style);
        self
    }

    /// Sets the size; the button stays square (`Medium`, 40 px, by default).
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.button = self.button.size(size);
        self
    }

    /// Lights the button up as being on, and announces it as a button that toggles.
    pub fn selected(mut self, selected: bool) -> Self {
        self.button = self.button.selected(selected);
        self
    }
}

impl View for IconButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.button.build(cx)
    }
}
