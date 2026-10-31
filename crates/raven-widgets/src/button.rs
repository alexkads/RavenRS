use std::fmt;
use std::rc::Rc;

use raven_core::{Border, CursorIcon, Insets};
use raven_layout::{Align, Axis, Justify, LayoutStyle};
use raven_style::{Semantic, Weight};
use raven_text::TextAttrs;

use crate::View;
use crate::context::{BuildCx, NodeId, Paint, QuadPaint};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Accent fill; one per view.
    #[default]
    Primary,
    /// Control fill for secondary actions.
    Secondary,
}

/// A clickable, focusable button. Activates on click, Enter or Space.
pub struct Button {
    label: String,
    on_click: Option<Rc<dyn Fn()>>,
    style: ButtonStyle,
}

impl fmt::Debug for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Button").field("label", &self.label).field("style", &self.style).finish()
    }
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), on_click: None, style: ButtonStyle::Primary }
    }

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
}

impl View for Button {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (pressed, hovered) = (cx.is_pressed(), cx.is_hovered());
        let (fill, label_color, border) = match self.style {
            ButtonStyle::Primary => {
                let fill = if pressed {
                    Semantic::AccentPressed
                } else if hovered {
                    Semantic::AccentHover
                } else {
                    Semantic::Accent
                };
                (theme.color(fill), theme.color(Semantic::OnAccent), None)
            }
            ButtonStyle::Secondary => {
                let base = theme.color(Semantic::Control);
                let fill = if pressed {
                    base.lerp(theme.color(Semantic::Ink), 0.12)
                } else if hovered {
                    base.lerp(theme.color(Semantic::Ink), 0.06)
                } else {
                    base
                };
                let border = Border { width: 1.0, color: theme.color(Semantic::Border) };
                (fill, theme.color(Semantic::Ink), Some(border))
            }
        };

        let body = theme.text(raven_style::TextStyle::Body);
        let attrs = TextAttrs {
            size: body.size,
            line_height: body.line_height,
            weight: Weight::SEMIBOLD.0,
            letter_spacing: 0.0,
            family: None,
        };
        let label = cx.text(&self.label, &attrs, label_color, LayoutStyle::leaf());
        let style = LayoutStyle {
            padding: Insets::symmetric(theme.spacing.s3, theme.spacing.s5),
            min_height: Some(theme.interaction.min_target),
            align: Align::Center,
            justify: Justify::Center,
            align_self: Some(Align::Start),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let radius = theme.radii.md;
        let paint = Paint::Quad(QuadPaint { fill: fill.into(), corner_radius: radius, border, shadow: None });
        let node = cx.container(style, &[label], paint);
        if let Some(handler) = &self.on_click {
            cx.on_click(node, handler.clone());
        }
        cx.focusable(node, radius);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}
