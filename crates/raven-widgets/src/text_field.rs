use std::fmt;
use std::rc::Rc;

use raven_core::{Border, CursorIcon, Insets, Key, Quad, Rect};
use raven_layout::{Align, Axis, LayoutStyle};
use raven_reactive::{Binding, State};
use raven_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint};
use crate::{IconName, View, util};

/// A single-line text input bound to a `String`.
///
/// Supports typing, Backspace/Delete, arrows, Home/End and Enter (submit).
/// Clipboard and IME composition are planned.
pub struct TextField {
    value: Binding<String>,
    placeholder: String,
    icon: Option<IconName>,
    secure: bool,
    width: Option<f32>,
    compact: bool,
    autofocus: bool,
    on_submit: Option<Rc<dyn Fn(String)>>,
    on_cancel: Option<Rc<dyn Fn()>>,
    intercept: Option<crate::KeyHandler>,
}

impl fmt::Debug for TextField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextField").field("placeholder", &self.placeholder).finish_non_exhaustive()
    }
}

impl TextField {
    pub fn new(value: Binding<String>) -> Self {
        Self {
            value,
            placeholder: String::new(),
            icon: None,
            secure: false,
            width: None,
            compact: false,
            autofocus: false,
            on_submit: None,
            on_cancel: None,
            intercept: None,
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// An icon inside the field, before the text.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Shows dots instead of the characters (passwords).
    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// A shorter field with smaller text, for toolbars, filter rows and cells.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Takes keyboard focus when it first appears.
    pub fn autofocus(mut self) -> Self {
        self.autofocus = true;
        self
    }

    /// Called when Escape is pressed while the field is focused.
    pub fn on_cancel(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// Sees keys before the field edits; return true to consume them (lists
    /// navigated from a search field use it for arrows and Enter).
    pub fn intercept_keys(mut self, handler: crate::KeyHandler) -> Self {
        self.intercept = Some(handler);
        self
    }

    /// Called with the current text when Enter is pressed.
    pub fn on_submit(mut self, handler: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }
}

fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices().nth(char_index).map_or(s.len(), |(i, _)| i)
}

/// Applies an editing key to `text` and the caret (a char index). Returns false
/// for keys the field does not handle.
pub(crate) fn edit(text: &mut String, caret: &mut usize, key: &Key) -> bool {
    let len = text.chars().count();
    *caret = (*caret).min(len);
    match key {
        Key::Character(s) => {
            let at = byte_index(text, *caret);
            text.insert_str(at, s);
            *caret += s.chars().count();
        }
        Key::Space => {
            let at = byte_index(text, *caret);
            text.insert(at, ' ');
            *caret += 1;
        }
        Key::Backspace if *caret > 0 => {
            let at = byte_index(text, *caret - 1);
            text.remove(at);
            *caret -= 1;
        }
        Key::Delete if *caret < len => {
            let at = byte_index(text, *caret);
            text.remove(at);
        }
        Key::Backspace | Key::Delete => {}
        Key::ArrowLeft => *caret = caret.saturating_sub(1),
        Key::ArrowRight => *caret = (*caret + 1).min(len),
        Key::Home | Key::ArrowUp => *caret = 0,
        Key::End | Key::ArrowDown => *caret = len,
        _ => return false,
    }
    true
}

impl View for TextField {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let caret: State<usize> = cx.use_state(|| usize::MAX);
        let focus_requested: State<bool> = cx.use_state(|| false);
        if self.autofocus && !*focus_requested.peek() {
            focus_requested.set_if_changed(true);
            cx.request_focus();
        }
        let focused = cx.is_focused();
        let hovered = cx.is_hovered();
        let text = self.value.get();
        let len = text.chars().count();
        let caret_at = caret.get().min(len);
        let shown: String = if self.secure { "•".repeat(len) } else { text.clone() };
        let attrs =
            util::attrs(&theme, if self.compact { TextStyle::Callout } else { TextStyle::Body }, None);

        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, 16.0, theme.color(Semantic::InkMuted)));
        }
        let prefix: String = shown.chars().take(caret_at).collect();
        let caret_x = cx.measure_text(&prefix, &attrs).width;
        let content = if shown.is_empty() {
            cx.text(&self.placeholder, &attrs, theme.color(Semantic::InkMuted), LayoutStyle::leaf())
        } else {
            cx.text(&shown, &attrs, theme.color(Semantic::Ink), LayoutStyle::leaf())
        };
        let accent = theme.color(Semantic::Accent);
        let line = attrs.line_height;
        let text_area = cx.container(
            LayoutStyle {
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Center,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[content],
            Paint::Custom(Rc::new(move |f: Rect, scene| {
                if focused {
                    let x = (f.x() + caret_x).min(f.max_x() - 1.5);
                    let y = f.center().y - line / 2.0 + 2.0;
                    scene.push_quad(Quad::new(Rect::new(x, y, 1.5, line - 4.0), accent).radius(0.75));
                }
            })),
        );
        cx.clip(text_area);
        children.push(text_area);

        let border = if focused {
            Border { width: 1.5, color: accent }
        } else {
            let base = theme.color(Semantic::Border);
            Border {
                width: 1.0,
                color: if hovered { base.lerp(theme.color(Semantic::Ink), 0.25) } else { base },
            }
        };
        let style = LayoutStyle {
            padding: Insets::symmetric(0.0, if self.compact { 8.0 } else { 12.0 }),
            height: Some(if self.compact { 30.0 } else { 40.0 }),
            width: self.width,
            min_width: Some(if self.compact { 0.0 } else { 160.0 }),
            spacing: 8.0,
            align: Align::Center,
            align_self: Some(if self.width.is_some() { Align::Start } else { Align::Stretch }),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            corner_radius: if self.compact { theme.radii.sm } else { theme.radii.md },
            border: Some(border),
            ..QuadPaint::new(theme.color(Semantic::Surface))
        };
        let node = cx.container(style, &children, Paint::Quad(paint));

        let (value, submit, cancel) = (self.value.clone(), self.on_submit.clone(), self.on_cancel.clone());
        let intercept = self.intercept.clone();
        cx.on_key(
            node,
            Rc::new(move |key, modifiers| {
                if let Some(i) = &intercept
                    && i(key, modifiers)
                {
                    return true;
                }
                if modifiers.ctrl || modifiers.meta {
                    return false;
                }
                if *key == Key::Escape
                    && let Some(cancel) = &cancel
                {
                    cancel();
                    return true;
                }
                if *key == Key::Enter {
                    if let Some(submit) = &submit {
                        submit(value.get());
                    }
                    return true;
                }
                let mut text = value.get();
                let mut c = caret.get();
                let handled = edit(&mut text, &mut c, key);
                if handled {
                    caret.set(c);
                    value.set(text);
                }
                handled
            }),
        );
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Text);
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_keys() {
        let (mut text, mut caret) = (String::from("héllo"), usize::MAX);
        assert!(edit(&mut text, &mut caret, &Key::Backspace));
        assert_eq!((text.as_str(), caret), ("héll", 4));
        edit(&mut text, &mut caret, &Key::Home);
        edit(&mut text, &mut caret, &Key::ArrowRight);
        edit(&mut text, &mut caret, &Key::Delete);
        assert_eq!(text, "hll");
        edit(&mut text, &mut caret, &Key::Character("é".into()));
        edit(&mut text, &mut caret, &Key::Space);
        assert_eq!((text.as_str(), caret), ("hé ll", 3));
        assert!(!edit(&mut text, &mut caret, &Key::Tab));
    }
}
