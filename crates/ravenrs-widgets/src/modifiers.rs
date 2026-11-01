use ravenrs_core::{Border, Insets, Shadow};
use ravenrs_layout::{Align, Axis, LayoutStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::{Fill, View};

/// A view wrapped in a box with layout and background modifiers; see [`ViewExt`].
#[derive(Debug)]
pub struct Styled<V> {
    content: V,
    style: LayoutStyle,
    background: Option<Fill>,
    radius: f32,
    border: Option<(f32, Fill)>,
    shadow: Option<Shadow>,
}

/// SwiftUI-style modifiers available on every view.
///
/// ```
/// use ravenrs_style::Semantic;
/// use ravenrs_widgets::{Text, ViewExt};
///
/// let chip = Text::new("New").padding((4.0, 10.0)).background(Semantic::Control).corner_radius(999.0);
/// ```
pub trait ViewExt: View + Sized {
    /// Adds padding inside the box, in logical pixels.
    fn padding(self, padding: impl Into<Insets>) -> Styled<Self> {
        Styled::new(self).padding(padding)
    }
    /// Paints a background behind the content.
    fn background(self, fill: impl Into<Fill>) -> Styled<Self> {
        Styled::new(self).background(fill)
    }
    /// Fixes the width and/or height in logical pixels; `None` keeps the content size.
    fn frame(self, width: Option<f32>, height: Option<f32>) -> Styled<Self> {
        Styled::new(self).frame(width, height)
    }
    /// Fixes the width in logical pixels.
    fn width(self, width: f32) -> Styled<Self> {
        Styled::new(self).width(width)
    }
    /// Fixes the height in logical pixels.
    fn height(self, height: f32) -> Styled<Self> {
        Styled::new(self).height(height)
    }
    /// Takes a share of the free space in the parent stack.
    fn grow(self, grow: f32) -> Styled<Self> {
        Styled::new(self).grow(grow)
    }
    /// Stretches across the parent stack's cross axis.
    fn stretch(self) -> Styled<Self> {
        Styled::new(self).stretch()
    }
    /// Centers the content inside the box.
    fn centered(self) -> Styled<Self> {
        Styled::new(self).centered()
    }
}

impl<V: View> ViewExt for V {}

impl<V: View> Styled<V> {
    fn new(content: V) -> Self {
        Self {
            content,
            style: LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            background: None,
            radius: 0.0,
            border: None,
            shadow: None,
        }
    }

    /// Sets the padding inside the box, in logical pixels.
    pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
        self.style.padding = padding.into();
        self
    }

    /// Paints a background behind the content.
    pub fn background(mut self, fill: impl Into<Fill>) -> Self {
        self.background = Some(fill.into());
        self
    }

    /// Rounds the box corners; radius in logical pixels.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Draws a border of `width` logical pixels.
    pub fn border(mut self, width: f32, fill: impl Into<Fill>) -> Self {
        self.border = Some((width, fill.into()));
        self
    }

    /// Casts a drop shadow behind the box.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = Some(shadow);
        self
    }

    /// Fixes the width and/or height in logical pixels; `None` keeps the content size.
    pub fn frame(mut self, width: Option<f32>, height: Option<f32>) -> Self {
        self.style.width = width;
        self.style.height = height;
        self
    }

    /// Fixes the width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.style.width = Some(width);
        self
    }

    /// Fixes the height in logical pixels.
    pub fn height(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self
    }

    /// Caps the width in logical pixels.
    pub fn max_width(mut self, width: f32) -> Self {
        self.style.max_width = Some(width);
        self
    }

    /// Takes a share of the free space in the parent stack.
    pub fn grow(mut self, grow: f32) -> Self {
        self.style.grow = grow;
        self.style.min_width.get_or_insert(0.0);
        self.style.min_height.get_or_insert(0.0);
        self
    }

    /// Stretches across the parent stack's cross axis.
    pub fn stretch(mut self) -> Self {
        self.style.align_self = Some(Align::Stretch);
        self
    }

    /// Centers the content inside the box.
    pub fn centered(mut self) -> Self {
        self.style.align = Align::Center;
        self.style.justify = ravenrs_layout::Justify::Center;
        self
    }
}

impl<V: View> View for Styled<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let theme = *cx.theme();
        let paint = if self.background.is_some() || self.border.is_some() || self.shadow.is_some() {
            let fill = self.background.map_or(ravenrs_core::Color::TRANSPARENT, |f| f.resolve(&theme));
            Paint::Quad(QuadPaint {
                fill: fill.into(),
                corner_radius: self.radius,
                border: self.border.map(|(width, f)| Border { width, color: f.resolve(&theme) }),
                shadow: self.shadow,
            })
        } else {
            Paint::None
        };
        cx.container(self.style, &[child], paint)
    }
}
