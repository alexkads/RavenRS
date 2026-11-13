//! [`Motion`], how a node animates, as the build records it. The runtime that plays it
//! lives in `motion`.

use ravenrs_animation::Transition;
use ravenrs_core::Effect;

use crate::ids::{NodeId, WidgetId};

/// How a node animates. Attach it with [`BuildCx::motion`](crate::BuildCx::motion).
///
/// ```
/// use ravenrs_animation::Transition;
/// use ravenrs_core::Effect;
/// use ravenrs_widgets::Motion;
///
/// // Colors ease on hover; the node pops in and fades out.
/// let motion = Motion::new()
///     .paint(Transition::fast())
///     .enter(Effect::hidden().with_scale(0.96), Transition::snappy())
///     .exit(Effect::hidden(), Transition::fast());
/// ```
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Motion {
    pub(crate) paint: Option<Transition>,
    pub(crate) layout: Option<Transition>,
    pub(crate) shape_only: bool,
    pub(crate) underlay: bool,
    pub(crate) shared: Option<(WidgetId, u64)>,
    pub(crate) surface_exit: bool,
    pub(crate) effect: (Effect, Transition),
    pub(crate) enter: Option<(Effect, Transition)>,
    pub(crate) exit: Option<(Effect, Transition)>,
    pub(crate) key: Option<u64>,
    pub(crate) anchored: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Motion {
    /// Nothing animates yet.
    pub const fn new() -> Self {
        Self {
            paint: None,
            layout: None,
            shape_only: false,
            underlay: false,
            shared: None,
            surface_exit: false,
            effect: (Effect::IDENTITY, Transition::Instant),
            enter: None,
            exit: None,
            key: None,
            anchored: false,
        }
    }

    /// Eases this node's own fill, border, shadow, corner radius and text color
    /// whenever a build changes them, like CSS `transition`. Colors travel
    /// through OKLab. Gradients and `Paint::Custom` change at once.
    pub const fn paint(mut self, transition: Transition) -> Self {
        self.paint = Some(transition);
        self
    }

    /// Glides the node from its old frame to its new one when layout moves it.
    /// Its children travel with it. Input keeps using the final frame.
    pub const fn layout(mut self, transition: Transition) -> Self {
        self.layout = Some(transition);
        self
    }

    /// Makes this node the same visual object as the node of a previous build
    /// with the same `scope` and `tag`, wherever it is now: a selection marker
    /// that slides from the old tab to the new one. `scope` is usually the
    /// widget id of the common parent.
    pub const fn shared_layout(mut self, scope: WidgetId, tag: u64, transition: Transition) -> Self {
        self.shared = Some((scope, tag));
        self.layout = Some(transition);
        self
    }

    /// The exit fades this node as one surface: its content goes first, over the still
    /// solid panel, and then the panel. Opacity applies to each shape on its own, so
    /// fading all at once would show the panel through its fields, in a gray like metal.
    /// The panel must be the first thing the node paints.
    pub const fn surface_exit(mut self) -> Self {
        self.surface_exit = true;
        self
    }

    /// With [`Self::layout`], moves only the node's own shape. Its children stay
    /// where layout put them.
    pub const fn shape_only(mut self) -> Self {
        self.shape_only = true;
        self
    }

    /// Paints the node's own shape behind all its siblings, so a moving marker
    /// slides under the labels it crosses. The node must be a direct child of
    /// the container holding those siblings.
    pub const fn underlay(mut self) -> Self {
        self.underlay = true;
        self
    }

    /// Eases the node and its subtree to this effect, such as a slight shrink
    /// while pressed. Layout and hit testing are unaffected.
    pub const fn effect(mut self, effect: Effect, transition: Transition) -> Self {
        self.effect = (effect, transition);
        self
    }

    /// When the node first appears, it starts from this effect.
    pub const fn enter(mut self, from: Effect, transition: Transition) -> Self {
        self.enter = Some((from, transition));
        self
    }

    /// When the node stops being built, its last picture stays on screen and
    /// plays this effect. The picture takes no input and is hidden from
    /// assistive technology: for the app, the node is already gone. Reopening
    /// during the exit reverses it from where it is.
    pub const fn exit(mut self, to: Effect, transition: Transition) -> Self {
        self.exit = Some((to, transition));
        self
    }

    /// The node's identity among the motions of its view. Defaults to call
    /// order, so use it for nodes built in a loop whose items come and go.
    pub const fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    /// For the root of an overlay: scales around the point nearest its anchor,
    /// and a vertical offset flips when the overlay opened on the other side.
    pub const fn anchored(mut self) -> Self {
        self.anchored = true;
        self
    }
}

/// The identity of an animated visual object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MotionKey {
    Node(WidgetId, u64),
    Shared(WidgetId, u64),
}

/// One [`BuildCx::motion`](crate::BuildCx::motion) call of the latest build.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MotionEntry {
    pub node: NodeId,
    pub key: MotionKey,
    pub motion: Motion,
}
