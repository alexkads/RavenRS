//! Layout for RavenRS, built on [Taffy](https://github.com/DioxusLabs/taffy).
//!
//! Widgets describe boxes with [`LayoutStyle`] (a deliberately small subset of
//! flexbox: stacks, gaps, padding, alignment, sizes and growth). Leaves that size
//! themselves, such as text, carry a context `C` and are measured through a
//! callback, so this crate does not depend on the text engine.

use std::fmt;

use ravenrs_core::{Insets, Point, Rect, Size};
use taffy::prelude::{
    AvailableSpace, Dimension, FlexDirection, LengthPercentage, LengthPercentageAuto, TaffyTree,
};

pub use taffy::NodeId as LayoutId;

/// Direction in which a stack places its children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Axis {
    /// Children top to bottom.
    #[default]
    Vertical,
    /// Children left to right.
    Horizontal,
}

/// Cross-axis alignment of a stack's children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Align {
    /// Align to the leading edge of the cross axis.
    #[default]
    Start,
    /// Center on the cross axis.
    Center,
    /// Align to the trailing edge of the cross axis.
    End,
    /// Fill the whole cross axis.
    Stretch,
    /// As `align_self`: keep its own size. It follows a parent that centers or
    /// ends its children, and goes to the start of one that stretches them.
    /// This is what a button wants, so it never grows to a column's width.
    Fit,
}

/// Main-axis distribution of a stack's children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Justify {
    /// Pack children at the start of the main axis.
    #[default]
    Start,
    /// Center children on the main axis.
    Center,
    /// Pack children at the end of the main axis.
    End,
    /// Put free space evenly between children, none at the ends.
    SpaceBetween,
}

/// The box model a widget asks for. `axis: None` makes a leaf.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutStyle {
    /// Stack direction, or `None` for a leaf.
    pub axis: Option<Axis>,
    /// Gap between consecutive children, in logical pixels.
    pub spacing: f32,
    /// Inner padding, in logical pixels.
    pub padding: Insets,
    /// Cross-axis alignment of children.
    pub align: Align,
    /// Main-axis distribution of children.
    pub justify: Justify,
    /// Fixed width in logical pixels; `None` sizes to content.
    pub width: Option<f32>,
    /// Fixed height in logical pixels; `None` sizes to content.
    pub height: Option<f32>,
    /// Minimum width in logical pixels.
    pub min_width: Option<f32>,
    /// Minimum height in logical pixels.
    pub min_height: Option<f32>,
    /// Maximum width in logical pixels.
    pub max_width: Option<f32>,
    /// Share of free main-axis space this box takes inside its parent stack.
    pub grow: f32,
    /// Overrides the parent's cross-axis alignment for this box.
    pub align_self: Option<Align>,
    /// Content may overflow this box (it scrolls or clips): the box does not
    /// grow to fit its content.
    pub overflow_hidden: bool,
    /// Lays children out in this many equal-width grid columns instead of a stack.
    pub grid_columns: Option<u16>,
    /// Children move to a new line when they do not fit (flex-wrap).
    pub wrap: bool,
    /// Width divided by height; with one dimension known, the other follows.
    pub aspect_ratio: Option<f32>,
    /// Takes the box out of the flow and places it at this `(x, y)` offset
    /// from its parent's top-left corner (design surfaces, diagrams, canvases).
    pub absolute: Option<(f32, f32)>,
}

impl LayoutStyle {
    /// A stack along `axis` with default spacing, padding and alignment.
    pub fn stack(axis: Axis) -> Self {
        Self { axis: Some(axis), ..Self::default() }
    }

    /// A leaf box with no children and default sizing.
    pub fn leaf() -> Self {
        Self::default()
    }

    fn to_taffy(self) -> taffy::Style {
        let len = LengthPercentage::length;
        let dim = |v: Option<f32>| v.map_or(Dimension::auto(), Dimension::length);
        let lpa = |v: Option<f32>| v.map_or(LengthPercentageAuto::auto(), LengthPercentageAuto::length);
        let align = |a: Align| match a {
            Align::Start | Align::Fit => taffy::AlignItems::FLEX_START,
            Align::Center => taffy::AlignItems::CENTER,
            Align::End => taffy::AlignItems::FLEX_END,
            Align::Stretch => taffy::AlignItems::STRETCH,
        };
        let mut style = taffy::Style {
            display: taffy::Display::Flex,
            flex_direction: match self.axis {
                Some(Axis::Horizontal) => FlexDirection::Row,
                _ => FlexDirection::Column,
            },
            gap: taffy::Size { width: len(self.spacing), height: len(self.spacing) },
            padding: taffy::Rect {
                left: len(self.padding.left),
                right: len(self.padding.right),
                top: len(self.padding.top),
                bottom: len(self.padding.bottom),
            },
            align_items: align(self.align),
            align_self: self.align_self.map(align),
            justify_content: match self.justify {
                Justify::Start => taffy::JustifyContent::FLEX_START,
                Justify::Center => taffy::JustifyContent::CENTER,
                Justify::End => taffy::JustifyContent::FLEX_END,
                Justify::SpaceBetween => taffy::JustifyContent::SPACE_BETWEEN,
            },
            size: taffy::Size { width: dim(self.width), height: dim(self.height) },
            min_size: taffy::Size { width: lpa(self.min_width), height: lpa(self.min_height) },
            max_size: taffy::Size { width: lpa(self.max_width), height: LengthPercentageAuto::auto() },
            flex_grow: self.grow,
            flex_shrink: 1.0,
            ..taffy::Style::default()
        };
        style.aspect_ratio = self.aspect_ratio;
        // A box with an explicit size keeps it: it never shrinks to make room.
        if self.width.is_some() || self.height.is_some() {
            style.flex_shrink = 0.0;
        }
        if self.wrap {
            style.flex_wrap = taffy::FlexWrap::Wrap;
        }
        if let Some((x, y)) = self.absolute {
            style.position = taffy::Position::Absolute;
            style.inset = taffy::Rect {
                left: LengthPercentageAuto::length(x),
                top: LengthPercentageAuto::length(y),
                right: LengthPercentageAuto::auto(),
                bottom: LengthPercentageAuto::auto(),
            };
        }
        if let Some(columns) = self.grid_columns {
            style.display = taffy::Display::Grid;
            style.grid_template_columns = taffy::style_helpers::evenly_sized_tracks(columns);
            style.align_items = taffy::AlignItems::STRETCH;
        }
        if self.overflow_hidden {
            style.overflow = taffy::Point { x: taffy::Overflow::Scroll, y: taffy::Overflow::Scroll };
            style.scrollbar_width = 0.0;
        }
        // Text and other measured leaves must be allowed to shrink below their
        // max-content width so they can wrap.
        if self.axis.is_none() {
            style.min_size.width = LengthPercentageAuto::length(self.min_width.unwrap_or(0.0));
            style.margin = taffy::Rect {
                left: LengthPercentageAuto::length(0.0),
                right: LengthPercentageAuto::length(0.0),
                top: LengthPercentageAuto::length(0.0),
                bottom: LengthPercentageAuto::length(0.0),
            };
        }
        style
    }
}

/// Space offered to a measured leaf along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Available {
    /// A definite number of logical pixels.
    Definite(f32),
    /// Lay out as narrow as possible (e.g. wrap at every opportunity).
    MinContent,
    /// Lay out without constraints (e.g. never wrap).
    MaxContent,
}

impl Available {
    /// The width to wrap at, if any.
    pub fn limit(self) -> Option<f32> {
        match self {
            Self::Definite(v) => Some(v),
            Self::MinContent => Some(0.0),
            Self::MaxContent => None,
        }
    }
}

/// What a measured leaf is asked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureInput {
    /// Dimensions already fixed by the parent.
    pub known_width: Option<f32>,
    /// Height already fixed by the parent, if any.
    pub known_height: Option<f32>,
    /// Horizontal space offered by the parent.
    pub available_width: Available,
    /// Vertical space offered by the parent.
    pub available_height: Available,
}

/// A failure reported by the underlying Taffy tree, e.g. an unknown node id.
#[derive(Debug)]
pub struct LayoutError(taffy::TaffyError);

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout error: {}", self.0)
    }
}

impl std::error::Error for LayoutError {}

impl From<taffy::TaffyError> for LayoutError {
    fn from(e: taffy::TaffyError) -> Self {
        Self(e)
    }
}

/// A layout tree whose measured leaves carry a context `C`.
pub struct LayoutTree<C> {
    taffy: TaffyTree<C>,
    /// Nodes whose `align_self` is [`Align::Fit`], until their parent is known.
    fit: std::collections::HashSet<LayoutId>,
}

impl<C> Default for LayoutTree<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> fmt::Debug for LayoutTree<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayoutTree").field("nodes", &self.taffy.total_node_count()).finish()
    }
}

fn to_available(a: AvailableSpace) -> Available {
    match a {
        AvailableSpace::Definite(v) => Available::Definite(v),
        AvailableSpace::MinContent => Available::MinContent,
        AvailableSpace::MaxContent => Available::MaxContent,
    }
}

impl<C> LayoutTree<C> {
    /// Creates an empty tree.
    ///
    /// ```
    /// use ravenrs_core::Size;
    /// use ravenrs_layout::{Axis, LayoutStyle, LayoutTree};
    ///
    /// let mut tree = LayoutTree::<()>::new();
    /// let child = tree.node(LayoutStyle { height: Some(20.0), ..LayoutStyle::leaf() }, &[]).unwrap();
    /// let root = tree.node(LayoutStyle { spacing: 8.0, ..LayoutStyle::stack(Axis::Vertical) }, &[child]).unwrap();
    /// tree.compute(root, Size::new(200.0, 100.0), |_, _| Size::ZERO).unwrap();
    /// assert_eq!(tree.frame(child).unwrap().height(), 20.0);
    /// ```
    pub fn new() -> Self {
        Self { taffy: TaffyTree::new(), fit: std::collections::HashSet::new() }
    }

    /// Removes every node, keeping allocations for the next build.
    pub fn clear(&mut self) {
        self.taffy.clear();
        self.fit.clear();
    }

    /// Adds a container (or a fixed-size leaf) with the given children.
    pub fn node(&mut self, style: LayoutStyle, children: &[LayoutId]) -> Result<LayoutId, LayoutError> {
        if matches!(style.align, Align::Center | Align::End) {
            for &child in children {
                if self.fit.contains(&child) {
                    let mut own = self.taffy.style(child)?.clone();
                    own.align_self = None;
                    self.taffy.set_style(child, own)?;
                }
            }
        }
        let id = self.taffy.new_with_children(style.to_taffy(), children)?;
        if style.align_self == Some(Align::Fit) {
            self.fit.insert(id);
        }
        Ok(id)
    }

    /// Adds a leaf whose size comes from the measure callback given to [`Self::compute`].
    pub fn measured_leaf(&mut self, style: LayoutStyle, context: C) -> Result<LayoutId, LayoutError> {
        Ok(self.taffy.new_leaf_with_context(style.to_taffy(), context)?)
    }

    /// Computes the layout of `root` inside `available` logical pixels.
    pub fn compute(
        &mut self,
        root: LayoutId,
        available: Size,
        mut measure: impl FnMut(&mut C, MeasureInput) -> Size,
    ) -> Result<(), LayoutError> {
        let space = taffy::Size {
            width: AvailableSpace::Definite(available.width),
            height: AvailableSpace::Definite(available.height),
        };
        self.taffy.compute_layout_with_measure(root, space, |inputs, _id, context, style| {
            taffy::compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, avail| {
                    let Some(ctx) = context else {
                        return taffy::Size::ZERO;
                    };
                    let size = measure(
                        ctx,
                        MeasureInput {
                            known_width: known.width,
                            known_height: known.height,
                            available_width: to_available(avail.width),
                            available_height: to_available(avail.height),
                        },
                    );
                    taffy::Size { width: size.width, height: size.height }
                },
            )
        })?;
        Ok(())
    }

    /// The node's frame relative to its parent's origin.
    pub fn frame(&self, node: LayoutId) -> Result<Rect, LayoutError> {
        let l = self.taffy.layout(node)?;
        Ok(Rect::from_origin_size(
            Point::new(l.location.x, l.location.y),
            Size::new(l.size.width, l.size.height),
        ))
    }

    /// The content box (frame minus padding) relative to the node's own origin.
    pub fn content_box(&self, node: LayoutId) -> Result<Rect, LayoutError> {
        let l = self.taffy.layout(node)?;
        Ok(Rect::new(
            l.padding.left,
            l.padding.top,
            (l.size.width - l.padding.left - l.padding.right).max(0.0),
            (l.size.height - l.padding.top - l.padding.bottom).max(0.0),
        ))
    }

    /// The size of the node's content, which can exceed its frame when it overflows.
    pub fn content_size(&self, node: LayoutId) -> Result<Size, LayoutError> {
        let l = self.taffy.layout(node)?;
        let r = l.scrollable_overflow_rect;
        Ok(Size::new(r.right.max(0.0), r.bottom.max(0.0)))
    }

    /// Mutable access to a measured leaf's context, or `None` for nodes without one.
    pub fn context_mut(&mut self, node: LayoutId) -> Option<&mut C> {
        self.taffy.get_node_context_mut(node)
    }

    /// Total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.taffy.total_node_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(tree: &mut LayoutTree<()>, w: f32, h: f32) -> LayoutId {
        let style = LayoutStyle { width: Some(w), height: Some(h), ..LayoutStyle::leaf() };
        tree.node(style, &[]).unwrap()
    }

    #[test]
    fn vstack_applies_padding_and_spacing() {
        let mut tree = LayoutTree::<()>::new();
        let a = fixed(&mut tree, 100.0, 20.0);
        let b = fixed(&mut tree, 50.0, 30.0);
        let style = LayoutStyle {
            spacing: 24.0,
            padding: Insets::uniform(32.0),
            width: Some(800.0),
            height: Some(600.0),
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let root = tree.node(style, &[a, b]).unwrap();
        tree.compute(root, Size::new(800.0, 600.0), |_, _| Size::ZERO).unwrap();
        assert_eq!(tree.frame(a).unwrap(), Rect::new(32.0, 32.0, 100.0, 20.0));
        assert_eq!(tree.frame(b).unwrap(), Rect::new(32.0, 76.0, 50.0, 30.0));
        assert_eq!(tree.frame(root).unwrap().size, Size::new(800.0, 600.0));
    }

    #[test]
    fn absolute_children_sit_at_their_offset_outside_the_flow() {
        let mut tree = LayoutTree::<()>::new();
        let flow = fixed(&mut tree, 40.0, 10.0);
        let pinned = tree
            .node(
                LayoutStyle {
                    width: Some(30.0),
                    height: Some(12.0),
                    absolute: Some((50.0, 70.0)),
                    ..LayoutStyle::leaf()
                },
                &[],
            )
            .unwrap();
        let style = LayoutStyle {
            padding: Insets::uniform(8.0),
            width: Some(200.0),
            height: Some(100.0),
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let root = tree.node(style, &[pinned, flow]).unwrap();
        tree.compute(root, Size::new(200.0, 100.0), |_, _| Size::ZERO).unwrap();
        assert_eq!(tree.frame(pinned).unwrap(), Rect::new(50.0, 70.0, 30.0, 12.0));
        assert_eq!(tree.frame(flow).unwrap().origin, Point::new(8.0, 8.0), "the flow ignores absolute boxes");
    }

    #[test]
    fn hstack_centers_on_cross_axis() {
        let mut tree = LayoutTree::<()>::new();
        let a = fixed(&mut tree, 10.0, 10.0);
        let b = fixed(&mut tree, 10.0, 30.0);
        let style = LayoutStyle {
            spacing: 5.0,
            align: Align::Center,
            height: Some(100.0),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let root = tree.node(style, &[a, b]).unwrap();
        tree.compute(root, Size::new(100.0, 100.0), |_, _| Size::ZERO).unwrap();
        assert_eq!(tree.frame(b).unwrap().origin, Point::new(15.0, 35.0));
        assert_eq!(tree.frame(a).unwrap().origin, Point::new(0.0, 45.0));
    }

    #[test]
    fn explicit_sizes_do_not_shrink_next_to_overflowing_content() {
        let mut tree = LayoutTree::<()>::new();
        let header = tree
            .node(LayoutStyle { height: Some(40.0), overflow_hidden: true, ..LayoutStyle::leaf() }, &[])
            .unwrap();
        let tall = fixed(&mut tree, 10.0, 5000.0);
        let body = tree
            .node(
                LayoutStyle {
                    grow: 1.0,
                    min_height: Some(0.0),
                    overflow_hidden: true,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[tall],
            )
            .unwrap();
        let root = tree
            .node(LayoutStyle { height: Some(300.0), ..LayoutStyle::stack(Axis::Vertical) }, &[header, body])
            .unwrap();
        tree.compute(root, Size::new(100.0, 300.0), |_, _| Size::ZERO).unwrap();
        assert_eq!(tree.frame(header).unwrap().size.height, 40.0);
        let body = tree.frame(body).unwrap();
        assert_eq!((body.y(), body.height()), (40.0, 260.0), "the overflowing body takes the rest");
    }

    #[test]
    fn grid_splits_width_into_equal_columns() {
        let mut tree = LayoutTree::<()>::new();
        let cells: Vec<_> = (0..4).map(|_| fixed(&mut tree, 10.0, 10.0)).collect();
        let style = LayoutStyle {
            grid_columns: Some(2),
            spacing: 10.0,
            width: Some(210.0),
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let root = tree.node(style, &cells).unwrap();
        tree.compute(root, Size::new(500.0, 500.0), |_, _| Size::ZERO).unwrap();
        assert_eq!(tree.frame(cells[1]).unwrap().origin, Point::new(110.0, 0.0));
        assert_eq!(tree.frame(cells[3]).unwrap().origin, Point::new(110.0, 20.0));
    }

    #[test]
    fn measured_leaf_wraps_to_available_width() {
        // A fake "text" of 300px that wraps into 100px-tall lines of at most the available width.
        let mut tree = LayoutTree::<f32>::new();
        let text = tree.measured_leaf(LayoutStyle::leaf(), 300.0).unwrap();
        let style = LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) };
        let root = tree.node(LayoutStyle { width: Some(120.0), ..style }, &[text]).unwrap();
        tree.compute(root, Size::new(500.0, 500.0), |natural, input| {
            let limit = input.known_width.or(input.available_width.limit()).unwrap_or(*natural);
            let width = natural.min(limit.max(1.0));
            Size::new(width, (*natural / width).ceil() * 10.0)
        })
        .unwrap();
        assert_eq!(tree.frame(text).unwrap().size, Size::new(120.0, 30.0));
        assert_eq!(*tree.context_mut(text).unwrap(), 300.0);
    }
}
