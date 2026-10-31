//! Small helpers shared by the built-in components.

use raven_core::Color;
use raven_layout::LayoutStyle;
use raven_style::{TextStyle, Theme, Weight};
use raven_text::{ICON_FAMILY, TextAttrs};

use crate::IconName;
use crate::context::{BuildCx, NodeId};

pub(crate) fn attrs(theme: &Theme, style: TextStyle, weight: Option<Weight>) -> TextAttrs {
    let t = theme.text(style);
    TextAttrs {
        size: t.size,
        line_height: t.line_height,
        weight: weight.unwrap_or(t.weight).0,
        letter_spacing: t.letter_spacing,
        family: None,
    }
}

/// A one-line label in the type scale.
pub(crate) fn label(
    cx: &mut BuildCx<'_>,
    text: &str,
    style: TextStyle,
    weight: Option<Weight>,
    color: Color,
) -> NodeId {
    let a = attrs(cx.theme(), style, weight);
    cx.text(text, &a, color, LayoutStyle::leaf())
}

pub(crate) fn icon_attrs(size: f32) -> TextAttrs {
    TextAttrs {
        size,
        line_height: size,
        weight: 400.0,
        letter_spacing: 0.0,
        family: Some(ICON_FAMILY.into()),
    }
}

/// An icon glyph of `size` logical pixels.
pub(crate) fn icon(cx: &mut BuildCx<'_>, name: IconName, size: f32, color: Color) -> NodeId {
    let mut glyph = [0u8; 4];
    let s = name.glyph().encode_utf8(&mut glyph);
    let style = LayoutStyle { width: Some(size), height: Some(size), ..LayoutStyle::leaf() };
    cx.text(s, &icon_attrs(size), color, style)
}
