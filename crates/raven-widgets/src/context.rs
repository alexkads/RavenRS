use std::fmt;
use std::rc::Rc;

use raven_core::{Border, Brush, Color, CursorIcon, Rect, Scene, Shadow};
use raven_layout::{LayoutError, LayoutId, LayoutStyle, LayoutTree};
use raven_style::Theme;
use raven_text::{TextAttrs, TextLayout, TextSystem};

use crate::View;
use crate::view::ViewSeq;

/// Stable identity of a widget across rebuilds, derived from its position in the
/// view tree (or from an explicit key, see [`BuildCx::keyed_child`]). Hover,
/// press and focus are remembered per `WidgetId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(u64);

impl WidgetId {
    pub(crate) const ROOT: Self = Self(0x5241_5645_4e52_5321);

    fn child(self, key: u64) -> Self {
        let mut h = self.0 ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Self(h ^ (h >> 31))
    }
}

/// A node of the element tree produced by one build. Only valid during the
/// build that created it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) u32);

/// A rounded rectangle painted behind an element's children.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadPaint {
    pub fill: Brush,
    pub corner_radius: f32,
    pub border: Option<Border>,
    pub shadow: Option<Shadow>,
}

impl QuadPaint {
    pub fn new(fill: impl Into<Brush>) -> Self {
        Self { fill: fill.into(), corner_radius: 0.0, border: None, shadow: None }
    }
}

/// Custom drawing callback: receives the element's absolute frame in logical pixels.
pub type DrawFn = Rc<dyn Fn(Rect, &mut Scene)>;

/// What an element draws before its children.
#[derive(Clone, Default)]
pub enum Paint {
    #[default]
    None,
    Quad(QuadPaint),
    /// Arbitrary drawing into the scene, given the element's absolute frame.
    Custom(DrawFn),
}

impl fmt::Debug for Paint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Quad(q) => f.debug_tuple("Quad").field(q).finish(),
            Self::Custom(_) => f.write_str("Custom"),
        }
    }
}

pub(crate) struct Element {
    pub widget: WidgetId,
    pub layout: LayoutId,
    pub children: Vec<NodeId>,
    pub paint: Paint,
    /// Color of the text this element shows, if it is a text leaf.
    pub text_color: Option<Color>,
    pub on_click: Option<Rc<dyn Fn()>>,
    /// Corner radius of the focus ring; `Some` makes the element focusable.
    pub focus_radius: Option<f32>,
    pub cursor: Option<CursorIcon>,
}

impl Element {
    pub fn is_interactive(&self) -> bool {
        self.on_click.is_some() || self.cursor.is_some() || self.focus_radius.is_some()
    }
}

/// Layout context of measured leaves.
pub(crate) struct TextLeaf {
    pub layout: TextLayout,
}

/// Which widgets the pointer and keyboard are interacting with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Interaction {
    pub hovered: Option<WidgetId>,
    pub pressed: Option<WidgetId>,
    pub focused: Option<WidgetId>,
    /// Focus came from the keyboard, so the ring is shown.
    pub focus_visible: bool,
}

struct Scope {
    id: WidgetId,
    next_child: u64,
}

/// What a [`View`] uses to describe itself during a build: theme access,
/// interaction state, child building, layout boxes, painting and behavior.
pub struct BuildCx<'a> {
    pub(crate) theme: &'a Theme,
    pub(crate) text: &'a mut TextSystem,
    pub(crate) layout: &'a mut LayoutTree<TextLeaf>,
    pub(crate) elements: &'a mut Vec<Element>,
    pub(crate) interaction: Interaction,
    pub(crate) placeholder: LayoutId,
    pub(crate) error: Option<LayoutError>,
    scopes: Vec<Scope>,
}

impl fmt::Debug for BuildCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildCx").field("widget", &self.widget_id()).finish_non_exhaustive()
    }
}

impl<'a> BuildCx<'a> {
    pub(crate) fn new(
        theme: &'a Theme,
        text: &'a mut TextSystem,
        layout: &'a mut LayoutTree<TextLeaf>,
        elements: &'a mut Vec<Element>,
        interaction: Interaction,
        placeholder: LayoutId,
    ) -> Self {
        Self {
            theme,
            text,
            layout,
            elements,
            interaction,
            placeholder,
            error: None,
            scopes: vec![Scope { id: WidgetId::ROOT, next_child: 0 }],
        }
    }

    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// The identity of the view being built.
    pub fn widget_id(&self) -> WidgetId {
        self.scopes.last().map_or(WidgetId::ROOT, |s| s.id)
    }

    /// The pointer is over this view's interactive element.
    pub fn is_hovered(&self) -> bool {
        self.interaction.hovered == Some(self.widget_id())
    }

    /// The primary button went down on this view and has not been released.
    pub fn is_pressed(&self) -> bool {
        self.interaction.pressed == Some(self.widget_id())
    }

    pub fn is_focused(&self) -> bool {
        self.interaction.focused == Some(self.widget_id())
    }

    /// Focus is on this view and came from the keyboard.
    pub fn is_focus_visible(&self) -> bool {
        self.is_focused() && self.interaction.focus_visible
    }

    /// Builds a child view; its identity comes from its position among this
    /// view's children.
    pub fn child(&mut self, view: &dyn View) -> NodeId {
        let key = match self.scopes.last_mut() {
            Some(scope) => {
                scope.next_child += 1;
                scope.next_child
            }
            None => 0,
        };
        self.build_scoped(key, view)
    }

    /// Builds a child with an explicit key, so it keeps its hover/focus state
    /// when siblings are inserted or removed (use it for dynamic lists).
    pub fn keyed_child(&mut self, key: u64, view: &dyn View) -> NodeId {
        // Keep keyed and positional identities apart.
        self.build_scoped(key ^ (1 << 63), view)
    }

    fn build_scoped(&mut self, key: u64, view: &dyn View) -> NodeId {
        let id = self.widget_id().child(key);
        self.scopes.push(Scope { id, next_child: 0 });
        let node = view.build(self);
        self.scopes.pop();
        node
    }

    /// Builds every view of a sequence as children.
    pub fn children(&mut self, seq: &dyn ViewSeq) -> Vec<NodeId> {
        let mut out = Vec::new();
        seq.build_each(self, &mut out);
        out
    }

    /// Creates a box that lays out `children` and paints `paint` behind them.
    pub fn container(&mut self, style: LayoutStyle, children: &[NodeId], paint: Paint) -> NodeId {
        let child_layouts: Vec<LayoutId> =
            children.iter().filter_map(|c| self.elements.get(c.0 as usize).map(|e| e.layout)).collect();
        let layout = match self.layout.node(style, &child_layouts) {
            Ok(id) => id,
            Err(e) => self.fail(e),
        };
        self.push(Element {
            widget: self.widget_id(),
            layout,
            children: children.to_vec(),
            paint,
            text_color: None,
            on_click: None,
            focus_radius: None,
            cursor: None,
        })
    }

    /// Creates a text leaf that wraps to the width its parent offers.
    pub fn text(&mut self, text: &str, attrs: &TextAttrs, color: Color, style: LayoutStyle) -> NodeId {
        let layout = self.text.layout(text, attrs);
        let id = match self.layout.measured_leaf(style, TextLeaf { layout }) {
            Ok(id) => id,
            Err(e) => self.fail(e),
        };
        self.push(Element {
            widget: self.widget_id(),
            layout: id,
            children: Vec::new(),
            paint: Paint::None,
            text_color: Some(color),
            on_click: None,
            focus_radius: None,
            cursor: None,
        })
    }

    /// An empty box that takes no space.
    pub fn empty(&mut self) -> NodeId {
        self.container(LayoutStyle::leaf(), &[], Paint::None)
    }

    /// Calls `handler` when the node is clicked, or activated with Enter/Space while focused.
    pub fn on_click(&mut self, node: NodeId, handler: Rc<dyn Fn()>) {
        if let Some(e) = self.element_mut(node) {
            e.on_click = Some(handler);
        }
    }

    /// Makes the node reachable with Tab; the focus ring uses `corner_radius`.
    pub fn focusable(&mut self, node: NodeId, corner_radius: f32) {
        if let Some(e) = self.element_mut(node) {
            e.focus_radius = Some(corner_radius);
        }
    }

    /// Shows `icon` while the pointer is over the node, and makes it hoverable.
    pub fn cursor(&mut self, node: NodeId, icon: CursorIcon) {
        if let Some(e) = self.element_mut(node) {
            e.cursor = Some(icon);
        }
    }

    fn element_mut(&mut self, node: NodeId) -> Option<&mut Element> {
        self.elements.get_mut(node.0 as usize)
    }

    fn push(&mut self, element: Element) -> NodeId {
        self.elements.push(element);
        NodeId((self.elements.len() - 1) as u32)
    }

    fn fail(&mut self, e: LayoutError) -> LayoutId {
        tracing::error!(error = %e, "layout node could not be created");
        if self.error.is_none() {
            self.error = Some(e);
        }
        self.placeholder
    }
}
