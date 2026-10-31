use raven_core::{Border, Color, Insets, Shadow};
use raven_layout::{Align, Axis, LayoutStyle};
use raven_style::Semantic;

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::{Fill, View};

/// A grouped surface lifted off the canvas: `bg-200` fill, `radius-xl` corners,
/// `shadow-soft` and a hairline border, padded by `space-6`.
#[derive(Debug)]
pub struct Card<V> {
    content: V,
    fill: Fill,
    corner_radius: Option<f32>,
    shadow: Option<Option<Shadow>>,
    padding: Option<Insets>,
    border: bool,
    spacing: Option<f32>,
}

impl<V: View> Card<V> {
    pub fn new(content: V) -> Self {
        Self {
            content,
            fill: Semantic::SurfaceElevated.into(),
            corner_radius: None,
            shadow: None,
            padding: None,
            border: true,
            spacing: None,
        }
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius);
        self
    }

    /// Replaces the theme's `shadow-soft`.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = Some(Some(shadow));
        self
    }

    pub fn no_shadow(mut self) -> Self {
        self.shadow = Some(None);
        self
    }

    pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
        self.padding = Some(padding.into());
        self
    }

    pub fn fill(mut self, fill: impl Into<Fill>) -> Self {
        self.fill = fill.into();
        self
    }

    pub fn border(mut self, visible: bool) -> Self {
        self.border = visible;
        self
    }

    fn paint(&self, cx: &BuildCx<'_>, fill: Color) -> QuadPaint {
        let theme = cx.theme();
        QuadPaint {
            fill: fill.into(),
            corner_radius: self.corner_radius.unwrap_or(theme.radii.xl),
            border: self.border.then(|| Border { width: 1.0, color: theme.color(Semantic::Border) }),
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
        let child = cx.child(&self.content);
        cx.container(style, &[child], Paint::Quad(paint))
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
    pub fn new(content: V) -> Self {
        Self { card: Card::new(content).fill(Color::hex_alpha(0xffffff1a)) }
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.card = self.card.corner_radius(radius);
        self
    }

    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.card = self.card.shadow(shadow);
        self
    }

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
