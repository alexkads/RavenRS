//! Small helpers shared by the built-in components.

use ravenrs_core::Color;
use ravenrs_layout::LayoutStyle;
use ravenrs_style::{TextStyle, Theme, Weight};
use ravenrs_text::{ICON_FAMILY, TextAttrs};

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

/// A pending focus move inside a group: which item takes focus on its next
/// build. Requesting one also bumps a reactive tick, so the group rebuilds even
/// when nothing else changed (arrow keys in a toolbar only move focus).
#[derive(Clone, Debug)]
pub(crate) struct FocusTarget {
    target: std::rc::Rc<std::cell::Cell<Option<usize>>>,
    tick: ravenrs_reactive::State<u32>,
}

impl FocusTarget {
    pub(crate) fn new(initial: Option<usize>) -> Self {
        Self {
            target: std::rc::Rc::new(std::cell::Cell::new(initial)),
            tick: ravenrs_reactive::State::new(0),
        }
    }

    pub(crate) fn request(&self, index: usize) {
        self.target.set(Some(index));
        self.tick.update(|t| *t = t.wrapping_add(1));
    }

    /// True once if focus should move to `index`; reading subscribes the build.
    pub(crate) fn take(&self, index: usize) -> bool {
        let _ = self.tick.get();
        let hit = self.target.get() == Some(index);
        if hit {
            self.target.set(None);
        }
        hit
    }
}

/// Roving focus for item `index` of `count` in a row or column (radio buttons,
/// tabs, toolbar buttons, menu items): arrow keys move focus to the neighbor,
/// wrapping; Home and End jump to the ends.
#[derive(Clone, Debug)]
pub(crate) struct Roving {
    pub(crate) index: usize,
    pub(crate) count: usize,
    pub(crate) focus_to: FocusTarget,
}

/// The shared [`FocusTarget`] of a group; call once per build of the group.
/// `initial` focuses that item when the group first appears (an opened menu).
pub(crate) fn roving_cell(cx: &mut BuildCx<'_>, initial: Option<usize>) -> FocusTarget {
    cx.use_state(|| FocusTarget::new(initial)).get()
}

impl Roving {
    /// Takes focus if an arrow key moved it here, and handles arrow keys on
    /// `node`. `on_move` runs with the new index (to select it, for patterns
    /// where selection follows focus).
    pub(crate) fn wire(
        &self,
        cx: &mut BuildCx<'_>,
        node: NodeId,
        on_move: Option<std::rc::Rc<dyn Fn(usize)>>,
    ) {
        self.wire_with(cx, node, on_move, None);
    }

    /// Like [`Self::wire`], with `before` seeing keys first (a menubar title
    /// opens its menu with Down; menu items switch menus with Left and Right).
    pub(crate) fn wire_with(
        &self,
        cx: &mut BuildCx<'_>,
        node: NodeId,
        on_move: Option<std::rc::Rc<dyn Fn(usize)>>,
        before: Option<crate::KeyHandler>,
    ) {
        if self.focus_to.take(self.index) {
            // By node: the focusable element may belong to a child widget (a button).
            cx.request_focus_node(node);
        }
        let (index, count, focus_to) = (self.index, self.count, self.focus_to.clone());
        cx.on_key(
            node,
            std::rc::Rc::new(move |key, modifiers| {
                use ravenrs_core::Key;
                if let Some(before) = &before
                    && before(key, modifiers)
                {
                    return true;
                }
                if count == 0 {
                    return false;
                }
                let target = match key {
                    Key::ArrowRight | Key::ArrowDown => (index + 1) % count,
                    Key::ArrowLeft | Key::ArrowUp => (index + count - 1) % count,
                    Key::Home => 0,
                    Key::End => count - 1,
                    _ => return false,
                };
                focus_to.request(target);
                if let Some(on_move) = &on_move {
                    on_move(target);
                }
                true
            }),
        );
    }
}
