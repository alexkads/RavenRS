use std::fmt;
use std::rc::Rc;

use raven_core::{Rect, Scene};
use raven_layout::LayoutStyle;

use crate::View;
use crate::context::{BuildCx, DrawFn, NodeId, Paint};

/// A box of a given size that draws whatever its closure appends to the scene.
/// The closure receives the box's absolute frame in logical pixels.
///
/// ```
/// use raven_core::{Color, Quad};
/// use raven_widgets::Canvas;
///
/// let bars = Canvas::new(|frame, scene| {
///     for (i, v) in [0.4_f32, 0.9, 0.6].iter().enumerate() {
///         let h = frame.height() * v;
///         let x = frame.x() + i as f32 * 24.0;
///         let bar = raven_core::Rect::new(x, frame.max_y() - h, 16.0, h);
///         scene.push_quad(Quad::new(bar, Color::hex(0x4c66ee)).radius(4.0));
///     }
/// })
/// .size(72.0, 48.0);
/// ```
pub struct Canvas {
    draw: DrawFn,
    style: LayoutStyle,
}

impl fmt::Debug for Canvas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Canvas").field("style", &self.style).finish_non_exhaustive()
    }
}

impl Canvas {
    pub fn new(draw: impl Fn(Rect, &mut Scene) + 'static) -> Self {
        Self { draw: Rc::new(draw), style: LayoutStyle::leaf() }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.style.width = Some(width);
        self.style.height = Some(height);
        self
    }

    /// Fills the free space along the parent stack's axis.
    pub fn grow(mut self, grow: f32) -> Self {
        self.style.grow = grow;
        self
    }
}

impl View for Canvas {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.container(self.style, &[], Paint::Custom(self.draw.clone()))
    }
}
