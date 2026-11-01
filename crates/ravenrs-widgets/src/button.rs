use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::{IconName, View, util};

/// Visual emphasis of a [`Button`] or other control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
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
pub enum ButtonSize {
    /// 32 px tall, smaller text; dense toolbars and table rows.
    Small,
    /// 40 px tall; the default.
    #[default]
    Medium,
    /// 48 px tall; prominent calls to action.
    Large,
}

impl ButtonSize {
    fn height(self) -> f32 {
        match self {
            Self::Small => 32.0,
            Self::Medium => 40.0,
            Self::Large => 48.0,
        }
    }

    fn padding_x(self) -> f32 {
        match self {
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
        let fade = |c: Color| c.with_alpha(c.a * 0.45);
        (fade(fill), fade(label), border)
    } else {
        (fill, label, border)
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
pub struct Button {
    label: String,
    on_click: Option<Rc<dyn Fn()>>,
    style: ButtonStyle,
    size: ButtonSize,
    icon: Option<IconName>,
    trailing_icon: Option<IconName>,
    disabled: bool,
    full_width: bool,
}

impl fmt::Debug for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Button").field("label", &self.label).field("style", &self.style).finish()
    }
}

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
        }
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
}

impl View for Button {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (fill, label_color, border) =
            control_colors(&theme, self.style, cx.is_hovered(), cx.is_pressed(), self.disabled);
        let icon_size = if self.size == ButtonSize::Small { 14.0 } else { 16.0 };
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, icon_size, label_color));
        }
        if !self.label.is_empty() {
            let style = if self.size == ButtonSize::Small { TextStyle::Callout } else { TextStyle::Body };
            children.push(util::label(cx, &self.label, style, Some(Weight::SEMIBOLD), label_color));
        }
        if let Some(icon) = self.trailing_icon {
            children.push(util::icon(cx, icon, icon_size, label_color));
        }
        let height = self.size.height();
        let icon_only = self.label.is_empty();
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, if icon_only { 0.0 } else { self.size.padding_x() }),
            width: icon_only.then_some(height),
            height: Some(height),
            spacing: 8.0,
            align: Align::Center,
            justify: Justify::Center,
            align_self: Some(if self.full_width { Align::Stretch } else { Align::Start }),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let radius = theme.radii.md;
        let paint = Paint::Quad(QuadPaint { fill: fill.into(), corner_radius: radius, border, shadow: None });
        let node = cx.container(style, &children, paint);
        if !self.disabled {
            if let Some(handler) = &self.on_click {
                cx.on_click(node, handler.clone());
            }
            cx.focusable(node, radius);
            cx.cursor(node, CursorIcon::Pointer);
        }
        node
    }
}

/// A square, icon-only button (ghost by default), for toolbars.
#[derive(Debug)]
pub struct IconButton {
    button: Button,
}

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
}

impl View for IconButton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.button.build(cx)
    }
}
