use raven_core::Insets;
use raven_layout::{Align, Axis, Justify, LayoutStyle};

use crate::context::{BuildCx, NodeId, Paint};
use crate::{View, ViewSeq};

macro_rules! stack {
    ($(#[$doc:meta])* $name:ident, $axis:expr) => {
        $(#[$doc])*
        pub struct $name<C> {
            children: C,
            style: LayoutStyle,
        }

        impl<C: ViewSeq> $name<C> {
            /// Children are a tuple of views (up to 12) or a `Vec`.
            pub fn new(children: C) -> Self {
                Self { children, style: LayoutStyle { spacing: 16.0, ..LayoutStyle::stack($axis) } }
            }

            /// Gap between children, in logical pixels (default 16, `space-4`).
            pub fn spacing(mut self, spacing: f32) -> Self {
                self.style.spacing = spacing;
                self
            }

            pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
                self.style.padding = padding.into();
                self
            }

            /// Cross-axis alignment of the children.
            pub fn align(mut self, align: Align) -> Self {
                self.style.align = align;
                self
            }

            /// Main-axis distribution of the children.
            pub fn justify(mut self, justify: Justify) -> Self {
                self.style.justify = justify;
                self
            }

            pub fn width(mut self, width: f32) -> Self {
                self.style.width = Some(width);
                self
            }

            pub fn height(mut self, height: f32) -> Self {
                self.style.height = Some(height);
                self
            }

            pub fn max_width(mut self, width: f32) -> Self {
                self.style.max_width = Some(width);
                self
            }

            /// Takes a share of the free space in the parent stack. A growing
            /// stack may also shrink below its content (CSS `min-height: 0`), so
            /// scroll views inside it get a bounded height.
            pub fn grow(mut self, grow: f32) -> Self {
                self.style.grow = grow;
                self.style.min_width.get_or_insert(0.0);
                self.style.min_height.get_or_insert(0.0);
                self
            }

            /// Moves children that do not fit onto new lines (rows wrap down,
            /// columns wrap right).
            pub fn wrap(mut self) -> Self {
                self.style.wrap = true;
                self
            }
        }

        impl<C> std::fmt::Debug for $name<C> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name)).field("style", &self.style).finish_non_exhaustive()
            }
        }

        impl<C: ViewSeq> View for $name<C> {
            fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
                let children = cx.children(&self.children);
                cx.container(self.style, &children, Paint::None)
            }
        }
    };
}

stack!(
    /// Arranges children top to bottom.
    VStack,
    Axis::Vertical
);
stack!(
    /// Arranges children left to right.
    HStack,
    Axis::Horizontal
);

/// Flexible empty space that pushes its siblings apart inside a stack.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spacer {
    min: f32,
}

impl Spacer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The smallest size the spacer keeps along the stack's axis.
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }
}

impl View for Spacer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let style = LayoutStyle {
            grow: 1.0,
            min_width: Some(self.min),
            min_height: Some(self.min),
            ..LayoutStyle::leaf()
        };
        cx.container(style, &[], Paint::None)
    }
}

/// A vertically scrolling stack. It fills the space its parent gives it (or
/// `height`) and scrolls its children with the wheel or trackpad, showing a
/// thin scrollbar when they overflow.
pub struct ScrollView<C> {
    children: C,
    style: LayoutStyle,
}

impl<C> std::fmt::Debug for ScrollView<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollView").field("style", &self.style).finish_non_exhaustive()
    }
}

impl<C: ViewSeq> ScrollView<C> {
    pub fn new(children: C) -> Self {
        Self {
            children,
            style: LayoutStyle {
                spacing: 16.0,
                grow: 1.0,
                align: Align::Stretch,
                overflow_hidden: true,
                min_height: Some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
        }
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.style.spacing = spacing;
        self
    }

    pub fn padding(mut self, padding: impl Into<Insets>) -> Self {
        self.style.padding = padding.into();
        self
    }

    /// A fixed height instead of filling the parent.
    pub fn height(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self.style.grow = 0.0;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.style.align = align;
        self
    }
}

impl<C: ViewSeq> View for ScrollView<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let children = cx.children(&self.children);
        let node = cx.container(self.style, &children, Paint::None);
        cx.scrollable(node);
        node
    }
}

/// Lays children out in equal-width columns, wrapping into rows.
pub struct Grid<C> {
    children: C,
    style: LayoutStyle,
}

impl<C> std::fmt::Debug for Grid<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grid").field("style", &self.style).finish_non_exhaustive()
    }
}

impl<C: ViewSeq> Grid<C> {
    pub fn new(columns: u16, children: C) -> Self {
        Self {
            children,
            style: LayoutStyle {
                spacing: 16.0,
                grid_columns: Some(columns.max(1)),
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
        }
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.style.spacing = spacing;
        self
    }
}

impl<C: ViewSeq> View for Grid<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let children = cx.children(&self.children);
        cx.container(self.style, &children, Paint::None)
    }
}
