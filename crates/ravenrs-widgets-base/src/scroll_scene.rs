//! Scroll-driven pages: content that stays pinned while its section scrolls past
//! ([`Sticky`], [`ScrollScene`]) and effects that follow how far the page has
//! scrolled ([`ScrollLinked`], [`ScrollLink`]).
//!
//! Everything here is resolved while frames are placed and painted. Scrolling still
//! neither rebuilds the views nor runs layout again: a pinned element is moved when it
//! is placed, so clicks, focus and screen readers find it where it shows, and a linked
//! effect is one small calculation per element when it is painted.
//!
//! ```
//! use ravenrs_widgets::*;
//! use ravenrs_core::Effect;
//!
//! // A section that holds its content on screen for 1,200 pixels of scrolling,
//! // fading the first panel out and the second one in halfway through.
//! let scene = ScrollScene::new(ZStack::new((
//!     ScrollLinked::new(Text::new("State"), ScrollLink::fade_out().between(0.4, 0.6)),
//!     ScrollLinked::new(Text::new("View"), ScrollLink::fade_in().between(0.4, 0.6)),
//! )))
//! .length(1200.0)
//! .top(64.0);
//! ```

use ravenrs_core::Insets;
use ravenrs_layout::{Align, Axis, LayoutStyle};

use crate::View;
use crate::context::{BuildCx, NodeId, Paint};
use crate::scroll_link::ScrollLink;

/// Keeps its content on screen while its parent scrolls past, like CSS
/// `position: sticky`: it stays `top` pixels below the top of the scroll container
/// and leaves with the bottom of its parent.
#[derive(Debug)]
pub struct Sticky<C> {
    content: C,
    top: Option<f32>,
    left: Option<f32>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> Sticky<C> {
    /// Sticks to the very top of the scroll container.
    pub fn new(content: C) -> Self {
        Self { content, top: Some(0.0), left: None }
    }

    /// Distance from the top of the scroll container, for a bar that stays above it.
    pub fn top(mut self, top: f32) -> Self {
        self.top = Some(top);
        self
    }

    /// Sticks to the left of the scroll container instead, `left` pixels from it, for a scroll
    /// that goes sideways: the first column of a timeline, the title of a row of cards. It
    /// leaves with the right of its parent. Call [`top`](Self::top) after it to stick to both.
    pub fn left(mut self, left: f32) -> Self {
        self.left = Some(left);
        self.top = None;
        self
    }
}

impl<C: View> View for Sticky<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        if let Some(top) = self.top {
            cx.pin(node, top);
        }
        if let Some(left) = self.left {
            cx.pin_left(node, left);
        }
        node
    }
}

/// A section that holds its content on screen for `length` pixels of scrolling: the
/// stage for steps that change as the reader scrolls. Inside it, [`ScrollLinked`]
/// views measure [`ScrollRange::Scene`](crate::ScrollRange::Scene), from 0 when the content pins to 1 when the
/// scene lets go.
#[derive(Debug)]
pub struct ScrollScene<C> {
    content: C,
    length: f32,
    top: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> ScrollScene<C> {
    /// Holds `content` for one screen's worth of scrolling (800 pixels) by default.
    pub fn new(content: C) -> Self {
        Self { content, length: 800.0, top: 0.0 }
    }

    /// How many pixels of scrolling the content stays pinned.
    pub fn length(mut self, length: f32) -> Self {
        self.length = length.max(0.0);
        self
    }

    /// Distance from the top of the scroll container while pinned.
    pub fn top(mut self, top: f32) -> Self {
        self.top = top;
        self
    }
}

impl<C: View> View for ScrollScene<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let pinned = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        cx.pin(pinned, self.top);
        let style = LayoutStyle {
            padding: Insets { bottom: self.length, ..Insets::ZERO },
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let track = cx.container(style, &[pinned], Paint::None);
        cx.scroll_track(track, self.top, self.length);
        track
    }
}

/// Paints its content with the effect a [`ScrollLink`] gives for the current scroll
/// position. Like the effects of [`crate::Motion`], it changes how the content looks,
/// not its layout or where it is clicked.
#[derive(Debug)]
pub struct ScrollLinked<C> {
    content: C,
    link: ScrollLink,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> ScrollLinked<C> {
    /// `content` painted through `link`.
    pub fn new(content: C, link: ScrollLink) -> Self {
        Self { content, link }
    }
}

impl<C: View> View for ScrollLinked<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        cx.scroll_linked(node, self.link.clone());
        node
    }
}
