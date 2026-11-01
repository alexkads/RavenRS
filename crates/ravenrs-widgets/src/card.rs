use std::fmt;
use std::rc::Rc;

use ravenrs_core::{Border, Color, CursorIcon, Insets, Role, Shadow};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::Semantic;

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{Fill, View};

/// A grouped surface lifted off the canvas: `bg-200` fill, `radius-xl` corners,
/// `shadow-soft` and a hairline border, padded by `space-6`.
pub struct Card<V> {
    content: V,
    fill: Fill,
    corner_radius: Option<f32>,
    shadow: Option<Option<Shadow>>,
    padding: Option<Insets>,
    border: bool,
    spacing: Option<f32>,
    on_click: Option<Rc<dyn Fn()>>,
}

impl<V: fmt::Debug> fmt::Debug for Card<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Card").field("content", &self.content).finish_non_exhaustive()
    }
}

impl<V: View> Card<V> {
    /// A card around `content` with the theme's surface, radius, shadow and padding.
    pub fn new(content: V) -> Self {
        Self {
            content,
            fill: Semantic::SurfaceElevated.into(),
            corner_radius: None,
            shadow: None,
            padding: None,
            border: true,
            spacing: None,
            on_click: None,
        }
    }

    /// Corner radius in logical pixels (theme `radius-xl` by default).
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius);
        self
    }

    /// Replaces the theme's `shadow-soft`.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = Some(Some(shadow));
        self
    }

    /// Removes the shadow.
    pub fn no_shadow(mut self) -> Self {
        self.shadow = Some(None);
        self
    }

    /// Replaces the theme's card padding.
    pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
        self.padding = Some(padding.into());
        self
    }

    /// Background, as a theme role (`SurfaceElevated` by default) or a literal color.
    pub fn fill(mut self, fill: impl Into<Fill>) -> Self {
        self.fill = fill.into();
        self
    }

    /// Shows or hides the hairline border (shown by default).
    pub fn border(mut self, visible: bool) -> Self {
        self.border = visible;
        self
    }

    /// Makes the whole card a button: it runs `handler` when clicked, or on
    /// Enter and Space while focused, and marks hover with the accent.
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn paint(&self, cx: &BuildCx<'_>, fill: Color) -> QuadPaint {
        let theme = cx.theme();
        let active = self.on_click.is_some() && (cx.is_hovered() || cx.is_focus_visible());
        QuadPaint {
            fill: fill.into(),
            corner_radius: self.corner_radius.unwrap_or(theme.radii.xl),
            border: self.border.then(|| Border {
                width: 1.0,
                color: theme.color(if active { Semantic::Accent } else { Semantic::Border }),
            }),
            shadow: self.shadow.unwrap_or(Some(theme.elevation.soft)),
        }
    }

    fn build_with(&self, cx: &mut BuildCx<'_>, fill: Color) -> NodeId {
        let paint = self.paint(cx, fill);
        let style = LayoutStyle {
            padding: self.padding.unwrap_or_else(|| cx.theme().card_padding()),
            spacing: self.spacing.unwrap_or(cx.theme().spacing.s2),
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let corner_radius = paint.corner_radius;
        let child = cx.child(&self.content);
        let node = cx.container(style, &[child], Paint::Quad(paint));
        if self.on_click.is_some() {
            crate::motion::control(cx, node);
        }
        if let Some(handler) = &self.on_click {
            cx.semantics(node, Semantics::new(Role::Button));
            cx.on_click(node, handler.clone());
            cx.focusable(node, corner_radius);
            cx.cursor(node, CursorIcon::Pointer);
        }
        node
    }
}

impl<V: View> View for Card<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let fill = self.fill.resolve(cx.theme());
        self.build_with(cx, fill)
    }
}

/// A translucent card for Raven Glass compositions: a light, see-through fill
/// and a brighter hairline over whatever is behind it.
///
/// Status: the backdrop blur of the glass material needs the renderer's layer
/// compositor (planned); until then the card renders its translucent fill only.
#[derive(Debug)]
pub struct GlassCard<V> {
    card: Card<V>,
}

impl<V: View> GlassCard<V> {
    /// A glass card around `content`.
    pub fn new(content: V) -> Self {
        Self { card: Card::new(content).fill(Color::hex_alpha(0xffffff1a)) }
    }

    /// Corner radius in logical pixels (theme `radius-xl` by default).
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.card = self.card.corner_radius(radius);
        self
    }

    /// Replaces the theme's `shadow-soft`.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.card = self.card.shadow(shadow);
        self
    }

    /// Replaces the theme's card padding.
    pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
        self.card = self.card.padding(padding);
        self
    }
}

impl<V: View> View for GlassCard<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let fill = self.card.fill.resolve(cx.theme());
        self.card.build_with(cx, fill)
    }
}
