//! Scroll-driven pages: content that stays pinned while its section scrolls past
//! ([`Sticky`], [`ScrollScene`]) and effects that follow how far the page has
//! scrolled ([`ScrollLinked`], [`ScrollLink`]).
//!
//! Everything here is resolved while frames are placed and painted. Scrolling still
//! neither rebuilds the views nor runs layout again: a pinned element is moved when it
//! is placed, so clicks, focus and screen readers find it where it shows, and a linked
//! effect is one small calculation per element when it is painted.
//!
//! ```
//! use ravenrs_widgets::*;
//! use ravenrs_core::Effect;
//!
//! // A section that holds its content on screen for 1,200 pixels of scrolling,
//! // fading the first panel out and the second one in halfway through.
//! let scene = ScrollScene::new(ZStack::new((
//!     ScrollLinked::new(Text::new("State"), ScrollLink::fade_out().between(0.4, 0.6)),
//!     ScrollLinked::new(Text::new("View"), ScrollLink::fade_in().between(0.4, 0.6)),
//! )))
//! .length(1200.0)
//! .top(64.0);
//! ```

use std::fmt;
use std::rc::Rc;

use ravenrs_animation::Easing;
use ravenrs_core::{Effect, Insets, Rect};
use ravenrs_layout::{Align, Axis, LayoutStyle};

use crate::View;
use crate::context::{BuildCx, NodeId, Paint};

/// Which stretch of scrolling a [`ScrollLink`] counts as going from 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ScrollRange {
    /// The nearest [`ScrollScene`] around the element: 0 when the scene pins, 1 when it
    /// lets go. Outside a scene, the same as [`Self::Through`].
    #[default]
    Scene,
    /// The element's whole pass across the visible area: 0 when its top reaches the
    /// bottom edge, 1 when its bottom leaves through the top edge.
    Through,
    /// The element coming in: 0 when its top reaches the bottom edge, 1 when it is fully
    /// visible (or, if taller than the view, when its top reaches the top edge).
    Enter,
    /// The element going out: 0 when its top reaches the top edge, 1 when its bottom does.
    Exit,
}

/// A scene's track, as [`BuildCx::scroll_track`] marks it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScrollTrack {
    pub top: f32,
    pub length: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl ScrollRange {
    /// How far along this range an element at `frame` is, inside the visible area
    /// `view` of its scroll container, from 0 to 1. `scene` is the frame and track of
    /// the nearest scene, which [`Self::Scene`] measures instead of the element.
    pub(crate) fn progress(self, frame: Rect, view: Rect, scene: Option<(Rect, ScrollTrack)>) -> f32 {
        let ratio = |num: f32, den: f32| {
            if den > 0.0 {
                (num / den).clamp(0.0, 1.0)
            } else if num >= 0.0 {
                1.0
            } else {
                0.0
            }
        };
        match (self, scene) {
            (Self::Scene, Some((track, t))) => ratio(view.y() + t.top - track.y(), t.length),
            (Self::Scene | Self::Through, _) => {
                ratio(view.max_y() - frame.y(), view.height() + frame.height())
            }
            (Self::Enter, _) => {
                let span = frame.height().min(view.height());
                ratio(view.max_y() - frame.y(), span)
            }
            (Self::Exit, _) => ratio(view.y() - frame.y(), frame.height()),
        }
    }
}

/// Turns scroll progress into an [`Effect`]: opacity, movement and scale.
///
/// Pick a [`ScrollRange`], narrow it to a window with [`Self::between`], shape it with
/// an [`Easing`], and describe the effect with keyframes or any function. Presets cover
/// the usual entrances and exits.
#[derive(Clone)]
pub struct ScrollLink {
    range: ScrollRange,
    from: f32,
    to: f32,
    easing: Easing,
    map: Rc<dyn Fn(f32) -> Effect>,
}

impl fmt::Debug for ScrollLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollLink")
            .field("range", &self.range)
            .field("from", &self.from)
            .field("to", &self.to)
            .field("easing", &self.easing)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ScrollLink {
    /// The effect is `map(t)`, where `t` runs from 0 to 1 over the window, after easing.
    pub fn custom(map: impl Fn(f32) -> Effect + 'static) -> Self {
        Self { range: ScrollRange::default(), from: 0.0, to: 1.0, easing: Easing::Linear, map: Rc::new(map) }
    }

    /// Interpolates between effects at given points of the window, like CSS keyframes.
    /// Before the first stop the first effect holds, after the last the last one does.
    /// No stops is no effect.
    pub fn keyframes(stops: impl IntoIterator<Item = (f32, Effect)>) -> Self {
        let mut stops: Vec<(f32, Effect)> = stops.into_iter().collect();
        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self::custom(move |t| keyframe_at(&stops, t))
    }

    /// Fades in and rises 24 pixels into place.
    pub fn fade_in() -> Self {
        Self::keyframes([(0.0, Effect::hidden().then(Effect::moved(0.0, 24.0))), (1.0, Effect::IDENTITY)])
    }

    /// Fades out and lifts 24 pixels away.
    pub fn fade_out() -> Self {
        Self::keyframes([(0.0, Effect::IDENTITY), (1.0, Effect::hidden().then(Effect::moved(0.0, -24.0)))])
    }

    /// Moves `distance` pixels down over the window: a negative distance moves it up.
    /// Slower than the scroll, it reads as depth.
    pub fn parallax(distance: f32) -> Self {
        Self::custom(move |t| Effect::moved(0.0, distance * t)).easing(Easing::Linear)
    }

    /// Grows from `from` to full size.
    pub fn grow_from(from: f32) -> Self {
        Self::keyframes([(0.0, Effect::scaled(from)), (1.0, Effect::IDENTITY)])
    }

    /// Measures this range instead of the default [`ScrollRange::Scene`].
    pub fn range(mut self, range: ScrollRange) -> Self {
        self.range = range;
        self
    }

    /// Plays the whole effect inside this part of the range, `0.0..=1.0`: before `from`
    /// it shows its start, after `to` its end. Scenes use it to give each step its turn.
    pub fn between(mut self, from: f32, to: f32) -> Self {
        self.from = from;
        self.to = to;
        self
    }

    /// Shapes the progress inside the window (linear by default).
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// The range this link measures.
    pub fn scroll_range(&self) -> ScrollRange {
        self.range
    }

    /// The effect at `progress` along the range, from 0 to 1.
    pub fn effect_at(&self, progress: f32) -> Effect {
        let span = self.to - self.from;
        let t = if span.abs() <= f32::EPSILON {
            if progress >= self.to { 1.0 } else { 0.0 }
        } else {
            ((progress - self.from) / span).clamp(0.0, 1.0)
        };
        (self.map)(self.easing.sample(t))
    }
}

fn keyframe_at(stops: &[(f32, Effect)], t: f32) -> Effect {
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return Effect::IDENTITY };
    if t <= first.0 {
        return first.1;
    }
    if t >= last.0 {
        return last.1;
    }
    for pair in stops.windows(2) {
        let ((a, ea), (b, eb)) = (pair[0], pair[1]);
        if t <= b {
            let local = if b > a { (t - a) / (b - a) } else { 1.0 };
            return ea.lerp(eb, local);
        }
    }
    last.1
}

/// Keeps its content on screen while its parent scrolls past, like CSS
/// `position: sticky`: it stays `top` pixels below the top of the scroll container
/// and leaves with the bottom of its parent.
#[derive(Debug)]
pub struct Sticky<C> {
    content: C,
    top: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> Sticky<C> {
    /// Sticks to the very top of the scroll container.
    pub fn new(content: C) -> Self {
        Self { content, top: 0.0 }
    }

    /// Distance from the top of the scroll container, for a bar that stays above it.
    pub fn top(mut self, top: f32) -> Self {
        self.top = top;
        self
    }
}

impl<C: View> View for Sticky<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        cx.pin(node, self.top);
        node
    }
}

/// A section that holds its content on screen for `length` pixels of scrolling: the
/// stage for steps that change as the reader scrolls. Inside it, [`ScrollLinked`]
/// views measure [`ScrollRange::Scene`], from 0 when the content pins to 1 when the
/// scene lets go.
#[derive(Debug)]
pub struct ScrollScene<C> {
    content: C,
    length: f32,
    top: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> ScrollScene<C> {
    /// Holds `content` for one screen's worth of scrolling (800 pixels) by default.
    pub fn new(content: C) -> Self {
        Self { content, length: 800.0, top: 0.0 }
    }

    /// How many pixels of scrolling the content stays pinned.
    pub fn length(mut self, length: f32) -> Self {
        self.length = length.max(0.0);
        self
    }

    /// Distance from the top of the scroll container while pinned.
    pub fn top(mut self, top: f32) -> Self {
        self.top = top;
        self
    }
}

impl<C: View> View for ScrollScene<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let pinned = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        cx.pin(pinned, self.top);
        let style = LayoutStyle {
            padding: Insets { bottom: self.length, ..Insets::ZERO },
            align: Align::Stretch,
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let track = cx.container(style, &[pinned], Paint::None);
        cx.scroll_track(track, self.top, self.length);
        track
    }
}

/// Paints its content with the effect a [`ScrollLink`] gives for the current scroll
/// position. Like the effects of [`crate::Motion`], it changes how the content looks,
/// not its layout or where it is clicked.
#[derive(Debug)]
pub struct ScrollLinked<C> {
    content: C,
    link: ScrollLink,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> ScrollLinked<C> {
    /// `content` painted through `link`.
    pub fn new(content: C, link: ScrollLink) -> Self {
        Self { content, link }
    }
}

impl<C: View> View for ScrollLinked<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        cx.scroll_linked(node, self.link.clone());
        node
    }
}

#[cfg(test)]
mod tests {
    use ravenrs_core::Point;

    use super::*;

    fn view() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    #[test]
    fn through_runs_from_the_bottom_edge_to_the_top_edge() {
        let at = |y| ScrollRange::Through.progress(Rect::new(0.0, y, 800.0, 200.0), view(), None);
        assert_eq!(at(600.0), 0.0);
        assert_eq!(at(-200.0), 1.0);
        assert!((at(200.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn scene_progress_follows_the_track_through_its_length() {
        let track = ScrollTrack { top: 64.0, length: 1000.0 };
        let at = |y| {
            ScrollRange::Scene.progress(Rect::ZERO, view(), Some((Rect::new(0.0, y, 800.0, 1600.0), track)))
        };
        assert_eq!(at(64.0), 0.0);
        assert!((at(-436.0) - 0.5).abs() < 1e-6);
        assert_eq!(at(-936.0), 1.0);
    }

    #[test]
    fn enter_and_exit_cover_their_own_edges() {
        let r = |y| Rect::new(0.0, y, 800.0, 200.0);
        assert_eq!(ScrollRange::Enter.progress(r(600.0), view(), None), 0.0);
        assert_eq!(ScrollRange::Enter.progress(r(400.0), view(), None), 1.0);
        assert_eq!(ScrollRange::Exit.progress(r(0.0), view(), None), 0.0);
        assert!((ScrollRange::Exit.progress(r(-100.0), view(), None) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_window_holds_its_ends_and_keyframes_interpolate() {
        let link = ScrollLink::fade_in().between(0.4, 0.6);
        assert_eq!(link.effect_at(0.1).opacity, 0.0);
        assert_eq!(link.effect_at(0.9), Effect::IDENTITY);
        let mid = link.effect_at(0.5);
        assert!((mid.opacity - 0.5).abs() < 1e-6);
        assert!((mid.offset.y - 12.0).abs() < 1e-4);
        let stops = ScrollLink::keyframes([
            (1.0, Effect::scaled(2.0)),
            (0.0, Effect::IDENTITY),
            (0.5, Effect::scaled(1.5)),
        ]);
        assert!((stops.effect_at(0.75).scale - 1.75).abs() < 1e-6, "stops are sorted");
        assert_eq!(ScrollLink::keyframes([]).effect_at(0.5), Effect::IDENTITY);
    }

    #[test]
    fn effects_compose_and_interpolate() {
        let a = Effect::moved(0.0, 10.0).with_opacity(0.5);
        let b = Effect::scaled(2.0).with_opacity(0.5);
        let both = a.then(b);
        assert_eq!(both.opacity, 0.25);
        assert_eq!(both.scale, 2.0);
        assert_eq!(both.offset, Point::new(0.0, 10.0));
        assert_eq!(Effect::IDENTITY.lerp(Effect::hidden(), 0.25).opacity, 0.75);
    }
}
