use std::fmt;
use std::rc::Rc;

use raven_layout::LayoutStyle;
use raven_reactive::State;
use raven_style::{Semantic, TextStyle, Weight};
use raven_text::TextAttrs;

use crate::context::{BuildCx, NodeId};
use crate::{Fill, View};

enum Content {
    Static(String),
    Dynamic(Rc<dyn Fn() -> String>),
}

/// Text in the theme's type scale. It wraps to the width its container offers.
pub struct Text {
    content: Content,
    style: TextStyle,
    size: Option<f32>,
    weight: Option<Weight>,
    foreground: Fill,
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let content = match &self.content {
            Content::Static(s) => s.as_str(),
            Content::Dynamic(_) => "<dynamic>",
        };
        f.debug_struct("Text").field("content", &content).field("style", &self.style).finish()
    }
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_content(Content::Static(text.into()))
    }

    /// Text derived from a state; the view updates when the state changes.
    pub fn bound<T: 'static>(state: &State<T>, format: impl Fn(&T) -> String + 'static) -> Self {
        let state = state.clone();
        Self::with_content(Content::Dynamic(Rc::new(move || state.with(&format))))
    }

    /// Text computed by a closure on every build; any state it reads is tracked.
    pub fn dynamic(f: impl Fn() -> String + 'static) -> Self {
        Self::with_content(Content::Dynamic(Rc::new(f)))
    }

    fn with_content(content: Content) -> Self {
        Self { content, style: TextStyle::Body, size: None, weight: None, foreground: Semantic::Ink.into() }
    }

    /// A step of the type scale (`Body` by default).
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Overrides the style's size; line height scales with it.
    pub fn font_size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    pub fn font_weight(mut self, weight: Weight) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Text color, as a theme role (`Ink` by default) or a literal color.
    pub fn foreground(mut self, fill: impl Into<Fill>) -> Self {
        self.foreground = fill.into();
        self
    }
}

impl View for Text {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = cx.theme();
        let base = theme.text(self.style);
        let size = self.size.unwrap_or(base.size);
        let attrs = TextAttrs {
            size,
            line_height: base.line_height * size / base.size,
            weight: self.weight.unwrap_or(base.weight).0,
            letter_spacing: base.letter_spacing,
            family: None,
        };
        let color = self.foreground.resolve(theme);
        match &self.content {
            Content::Static(s) => cx.text(s, &attrs, color, LayoutStyle::leaf()),
            Content::Dynamic(f) => {
                let s = f();
                cx.text(&s, &attrs, color, LayoutStyle::leaf())
            }
        }
    }
}
