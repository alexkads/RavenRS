//! [`QuarterTurn`]: a view turned on its side, with its box turned too.

use std::f32::consts::FRAC_PI_2;

use ravenrs_core::Size;
use ravenrs_layout::{Align, Axis, LayoutStyle};

use crate::View;
use crate::context::{BuildCx, NodeId, Paint};

/// Which way a [`QuarterTurn`] turns what it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Turn {
    /// A quarter clockwise: text reads from top to bottom, its tops toward the right.
    #[default]
    Clockwise,
    /// A quarter counterclockwise: text reads from bottom to top, its tops toward the left.
    CounterClockwise,
}

/// A view turned a quarter, in a box as wide as the view is tall and as tall as it is wide: a name
/// down a narrow strip at the edge of a window, or the title beside the side of a chart. The
/// pointer, the focus ring and screen readers follow the turn; what is read stays the same.
///
/// The box takes the size of what it holds from the last layout, so the first frame it shows in
/// has it empty.
///
/// ```
/// use ravenrs_widgets::{QuarterTurn, Text, Turn};
///
/// let down_the_side = QuarterTurn::new(Text::new("Files"), Turn::Clockwise);
/// ```
#[derive(Debug)]
pub struct QuarterTurn<V> {
    content: V,
    turn: Turn,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<V: View> QuarterTurn<V> {
    /// `content` turned `turn`.
    pub fn new(content: V, turn: Turn) -> Self {
        Self { content, turn }
    }
}

/// How wide the room is where the content is measured lying down: wide enough that a name never
/// breaks into lines before it turns.
const PROBE: f32 = 4096.0;

impl<V: View> View for QuarterTurn<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        // The content at its own size, which the next build reads, and builds again when it changes.
        let natural_box = cx.container(LayoutStyle::stack(Axis::Vertical), &[child], Paint::None);
        cx.report_frame(natural_box);
        let natural = cx.scroll_viewport().unwrap_or(Size::ZERO);
        let (width, height) = (natural.width, natural.height);
        let radians = match self.turn {
            Turn::CounterClockwise => -FRAC_PI_2,
            _ => FRAC_PI_2,
        };
        cx.rotate(natural_box, radians);
        // Laid out lying down in a room far wider than it, so that it keeps its own width, with its
        // center on the center of the box: turning it around its center stands it up in the box.
        let lying = cx.container(
            LayoutStyle {
                absolute: Some(((height - width) / 2.0, (width - height) / 2.0)),
                width: Some(PROBE),
                align: Align::Start,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[natural_box],
            Paint::None,
        );
        cx.container(
            LayoutStyle { width: Some(height), height: Some(width), ..LayoutStyle::stack(Axis::Vertical) },
            &[lying],
            Paint::None,
        )
    }
}
