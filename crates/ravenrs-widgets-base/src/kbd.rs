//! [`Kbd`]: a keyboard key, drawn as shortcuts show it.

use ravenrs_core::{Border, Color, Insets};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::{View, util};

/// A keyboard key, for shortcuts: `Kbd::new("⌘K")`.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Kbd {
    keys: String,
    inherit: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Kbd {
    /// A key cap showing `keys`.
    pub fn new(keys: impl Into<String>) -> Self {
        Self { keys: keys.into(), inherit: false }
    }

    /// Takes its color from what it is in ([`BuildCx::with_foreground`]) and drops its own
    /// fill, so that on a filled button the key reads as part of the button instead of a
    /// gray box. [`Button::shortcut`](crate::Button::shortcut) and [`Tooltip::shortcut`](crate::Tooltip::shortcut)
    /// do this. With no color around it, it uses its usual one.
    pub fn inherit_color(mut self) -> Self {
        self.inherit = true;
        self
    }
}

impl View for Kbd {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let inherited = cx.foreground().filter(|_| self.inherit);
        let ink = inherited.unwrap_or_else(|| theme.color(Semantic::InkMuted));
        let text = util::label(cx, &self.keys, TextStyle::Caption, Some(Weight::MEDIUM), ink);
        let style = LayoutStyle {
            padding: Insets { top: 1.0, bottom: 2.0, left: 6.0, right: 6.0 },
            align_self: Some(Align::Center),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            fill: if inherited.is_some() { Color::TRANSPARENT } else { theme.color(Semantic::Control) }
                .into(),
            corner_radius: theme.radii.sm,
            border: Some(Border {
                width: 1.0,
                color: inherited.map_or(theme.color(Semantic::Border), |c| c.with_alpha(c.a * 0.4)),
            }),
            shadow: None,
        };
        cx.container(style, &[text], Paint::Quad(paint))
    }
}
