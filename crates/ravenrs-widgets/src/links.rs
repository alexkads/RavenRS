//! Opening addresses and jumping to anchors, as a service the platform layer provides,
//! and [`Link`], a piece of text that opens one.

use std::rc::Rc;

use ravenrs_core::{Color, CursorIcon, Insets, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{IconName, View, util};

/// Text that opens an address when clicked or activated with the keyboard. `#name`
/// scrolls to an anchor in the app.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Link {
    label: String,
    url: String,
    style: TextStyle,
    icon: Option<IconName>,
    muted: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Link {
    /// A link showing `label` that opens `url`.
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self { label: label.into(), url: url.into(), style: TextStyle::Body, icon: None, muted: false }
    }

    /// The text style (body by default).
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// An icon after the text, such as an arrow.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Muted at rest, for links among other links, such as a navigation bar.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }
}

impl View for Link {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let lit = cx.is_hovered() || cx.is_focus_visible();
        let ink = theme.color(Semantic::Ink);
        let rest = if self.muted {
            theme.color(Semantic::InkMuted)
        } else {
            // The accent moved toward the ink, so the text stays readable on every canvas.
            theme.color(Semantic::Accent).lerp(ink, 0.4)
        };
        let color = if lit { ink } else { rest };
        let mut parts =
            vec![util::label(cx, &self.label, self.style, Some(ravenrs_style::Weight::SEMIBOLD), color)];
        if let Some(icon) = self.icon {
            parts.push(util::icon(cx, icon, 16.0, color));
        }
        let fill = if lit { ink.with_alpha(0.06) } else { Color::TRANSPARENT };
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(4.0, 6.0),
                spacing: 6.0,
                align: Align::Center,
                align_self: Some(Align::Start),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &parts,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.sm, ..QuadPaint::new(fill) }),
        );
        let (links, url) = (cx.links(), self.url.clone());
        cx.on_click(node, Rc::new(move || links.open(&url)));
        cx.semantics(node, Semantics::new(Role::Link).name(self.label.clone()));
        cx.focusable(node, theme.radii.sm);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}
