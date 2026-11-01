use ravenrs_style::Semantic;

use crate::context::{BuildCx, NodeId};
use crate::{Fill, IconName, View, util};

/// A vector icon from the built-in Lucide set, drawn as a glyph so it stays
/// crisp at any size and takes the text color.
#[derive(Clone, Copy, Debug)]
pub struct Icon {
    name: IconName,
    size: f32,
    color: Fill,
}

impl Icon {
    pub fn new(name: IconName) -> Self {
        Self { name, size: 16.0, color: Semantic::Ink.into() }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: impl Into<Fill>) -> Self {
        self.color = color.into();
        self
    }
}

impl View for Icon {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let color = self.color.resolve(cx.theme());
        util::icon(cx, self.name, self.size, color)
    }
}
