//! Motion for showcase pages: effects that follow the clock ([`TimeLink`]), a strip
//! that keeps moving ([`Ticker`]), a number that counts up when it is seen
//! ([`CountUp`]) and a place links scroll to ([`Anchor`]).

use std::fmt;
use std::rc::Rc;

use ravenrs_animation::{Easing, Transition};
use ravenrs_core::{Effect, Rect, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, Semantics};
use crate::{View, util};

/// Turns the clock (seconds) and an element's frame into an [`Effect`], worked out
/// on every painted frame while the element shows. See [`BuildCx::time_linked`].
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct TimeLink(Rc<dyn Fn(f64, Rect) -> Effect>);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl TimeLink {
    /// The effect is `f(seconds, frame)`.
    pub fn new(f: impl Fn(f64, Rect) -> Effect + 'static) -> Self {
        Self(Rc::new(f))
    }

    /// Floats up and down by `distance` pixels every `period` seconds.
    pub fn float(distance: f32, period: f32) -> Self {
        Self::new(move |t, _| {
            let phase = (t as f32 / period.max(0.01)) * std::f32::consts::TAU;
            Effect::moved(0.0, phase.sin() * distance)
        })
    }

    /// The effect at `seconds` for an element at `frame`.
    pub fn effect_at(&self, seconds: f64, frame: Rect) -> Effect {
        (self.0)(seconds, frame)
    }
}

impl fmt::Debug for TimeLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TimeLink")
    }
}

/// Repeats its content in a strip that keeps moving sideways, for logos, quotes or
/// samples of components. It moves on the GPU's clock without rebuilding, and stands
/// still under reduced motion.
///
/// The strip stops while the pointer is over it, so what it carries can be read and
/// used, and goes on from the same place when the pointer leaves. With
/// [`Self::fade_edges`] its ends melt into the color behind it instead of cutting the
/// content off.
#[derive(Debug)]
pub struct Ticker<C> {
    content: C,
    speed: f32,
    gap: f32,
    reverse: bool,
    pause_on_hover: bool,
    fade: f32,
    fade_color: Semantic,
}

/// Where a [`Ticker`]'s clock stands: how much time its pauses took, and since when it
/// is paused. The strip is at `(t - shift) * speed`, with `t` frozen while paused.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TickerClock {
    shift: f64,
    paused_at: Option<f64>,
}

impl TickerClock {
    /// The clock after the pointer is over the strip (`hovered`) or not, at `now`.
    fn follow(self, hovered: bool, now: f64) -> Self {
        match (hovered, self.paused_at) {
            (true, None) => Self { paused_at: Some(now), ..self },
            (false, Some(at)) => Self { shift: self.shift + (now - at).max(0.0), paused_at: None },
            _ => self,
        }
    }

    /// The moving time at clock `t`.
    fn at(self, t: f64) -> f64 {
        self.paused_at.unwrap_or(t).min(t) - self.shift
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> Ticker<C> {
    /// A strip of `content` moving left at 40 pixels a second.
    pub fn new(content: C) -> Self {
        Self {
            content,
            speed: 40.0,
            gap: 16.0,
            reverse: false,
            pause_on_hover: true,
            fade: 0.0,
            fade_color: Semantic::Canvas,
        }
    }

    /// Whether the strip stops while the pointer is over it (it does by default).
    pub fn pause_on_hover(mut self, pause: bool) -> Self {
        self.pause_on_hover = pause;
        self
    }

    /// Fades the content out over `width` pixels at each end, into `color`, which
    /// should be what is behind the strip (the canvas, a card).
    pub fn fade_edges(mut self, width: f32, color: Semantic) -> Self {
        self.fade = width.max(0.0);
        self.fade_color = color;
        self
    }

    /// Pixels per second.
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.0);
        self
    }

    /// The space between one copy of the content and the next.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Moves right instead of left.
    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

impl<C: View> View for Ticker<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (first, second) = (cx.child(&self.content), cx.child(&self.content));
        let strip = cx.container(
            LayoutStyle { spacing: self.gap, align: Align::Center, ..LayoutStyle::stack(Axis::Horizontal) },
            &[first, second],
            Paint::None,
        );
        let (speed, gap, reverse) = (self.speed, self.gap, self.reverse);
        let clock = cx.use_state(TickerClock::default);
        let hovered = self.pause_on_hover && cx.is_hover_within();
        let now = cx.now();
        let next = clock.get().follow(hovered, now);
        if next != clock.get() {
            clock.set(next);
        }
        cx.time_linked(
            strip,
            TimeLink::new(move |t, frame| {
                // One copy and its gap: after that distance the strip looks the same again.
                let span = (frame.width() + gap) / 2.0;
                if span <= 0.0 {
                    return Effect::IDENTITY;
                }
                let x = (next.at(t) as f32 * speed).rem_euclid(span);
                Effect::moved(if reverse { x - span } else { -x }, 0.0)
            }),
        );
        let mut children = vec![strip];
        let mut paint = Paint::None;
        if self.fade > 0.0 {
            let band = Rc::new(std::cell::Cell::new(Rect::ZERO));
            children.push(edge_fades(cx, self.fade, self.fade_color, band.clone()));
            // The strip's own paint runs before its children's in the same pass: it tells
            // the fades where its ends are.
            paint = Paint::Custom(Rc::new(move |frame: Rect, _: &mut ravenrs_core::Scene| band.set(frame)));
        }
        let node = cx.container(LayoutStyle::stack(Axis::Horizontal), &children, paint);
        cx.clip(node);
        if self.pause_on_hover {
            // Asking about the pointer needs an element the pointer can be over.
            cx.on_pointer_move(node, Rc::new(|_, _| {}));
        }
        node
    }
}

/// A sizeless element, out of the flow at the top-left of its parent, that paints a
/// fade at each end of `band`, the parent's frame, over what comes before it.
fn edge_fades(cx: &mut BuildCx<'_>, width: f32, color: Semantic, band: Rc<std::cell::Cell<Rect>>) -> NodeId {
    let from = cx.theme().color(color);
    let seen = band;
    let paint = Paint::Custom(Rc::new(move |_frame: Rect, scene: &mut ravenrs_core::Scene| {
        let band = seen.get();
        let w = width.min(band.width() / 2.0);
        if w <= 0.0 {
            return;
        }
        for (x, start) in [(band.x(), 0.0), (band.max_x() - w, 1.0)] {
            let gradient = ravenrs_core::LinearGradient {
                start: ravenrs_core::Point::new(start, 0.5),
                end: ravenrs_core::Point::new(1.0 - start, 0.5),
                from,
                to: from.with_alpha(0.0),
            };
            scene.push_quad(ravenrs_core::Quad::new(Rect::new(x, band.y(), w, band.height()), gradient));
        }
    }));
    let fades = cx.container(LayoutStyle { absolute: Some((0.0, 0.0)), ..LayoutStyle::leaf() }, &[], paint);
    cx.semantics(fades, Semantics::hidden());
    fades
}

/// A number that counts up from zero to its value when it first shows on screen, and
/// from where it is to its new value whenever the value changes, even halfway through a
/// count. Its digits keep one width, so the number does not shake as it counts. Under
/// reduced motion it shows the value at once.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CountUp {
    value: f64,
    format: Rc<dyn Fn(f64) -> String>,
    style: TextStyle,
    seconds: f64,
    easing: Easing,
    from_zero: bool,
    whole: bool,
}

impl fmt::Debug for CountUp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CountUp").field("value", &self.value).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CountUp {
    /// Counts up to `value`, shown as a whole number with thousands separators.
    pub fn new(value: f64) -> Self {
        Self {
            value,
            format: Rc::new(|v| group_thousands(v.round() as i64)),
            style: TextStyle::Metric,
            seconds: 1.5,
            easing: Easing::EaseOut,
            from_zero: true,
            whole: false,
        }
    }

    /// Waits to count until the whole number shows, not just its top edge, so the count
    /// is watched from the start rather than found halfway.
    pub fn when_fully_visible(mut self) -> Self {
        self.whole = true;
        self
    }

    /// How each step of the count is written, such as `|v| format!("{v:.0}+")`.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// The text style (metric by default).
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// How long a count takes (1.5 seconds by default).
    pub fn seconds(mut self, seconds: f64) -> Self {
        self.seconds = seconds.max(0.0);
        self
    }

    /// The pace of the count (fast, then settling, by default).
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Shows its value from the start: only later changes count, as for a figure on a
    /// dashboard that follows a filter.
    pub fn start_at_value(mut self) -> Self {
        self.from_zero = false;
        self
    }
}

impl View for CountUp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let seen = cx.use_state(|| false);
        let waiting = self.from_zero && !seen.get() && !cx.reduced_motion();
        let target = if waiting { 0.0 } else { self.value };
        let transition = Transition::tween(self.seconds as f32, self.easing);
        let shown = cx.animated_numbers(&[target], transition)[0];
        let attrs = util::attrs(&theme, self.style, None);
        // The widest digit, measured once for each size, is the width of every digit.
        let cell = cx.use_state(|| (0.0_f32, 0.0_f32));
        let (size, mut digit) = cell.get();
        if size != attrs.size {
            digit = (0..10).map(|d| cx.measure_text(&d.to_string(), &attrs).width).fold(0.0, f32::max);
            cell.set((attrs.size, digit));
        }
        let ink = theme.color(Semantic::Ink);
        let text = (self.format)(shown);
        // Each digit alone in a cell of that width; what is between digits as it comes, with
        // its spaces kept from being trimmed at the end of a piece.
        let mut glyphs = Vec::with_capacity(text.len());
        let pieces = text.split_inclusive(|c: char| c.is_ascii_digit()).flat_map(|piece| {
            let (before, last) = piece.split_at(piece.len() - piece.chars().last().map_or(0, char::len_utf8));
            let digit = last.starts_with(|c: char| c.is_ascii_digit());
            if digit { [before, last] } else { [piece, ""] }
        });
        for piece in pieces.filter(|p| !p.is_empty()) {
            let is_digit = piece.starts_with(|c: char| c.is_ascii_digit());
            let glyph = cx.text(&piece.replace(' ', "\u{a0}"), &attrs, ink, LayoutStyle::leaf());
            cx.semantics(glyph, Semantics::hidden());
            glyphs.push(if is_digit {
                cx.container(
                    LayoutStyle {
                        width: Some(digit),
                        align: Align::Center,
                        ..LayoutStyle::stack(Axis::Vertical)
                    },
                    &[glyph],
                    Paint::None,
                )
            } else {
                glyph
            });
        }
        let node = cx.container(LayoutStyle::stack(Axis::Horizontal), &glyphs, Paint::None);
        cx.semantics(node, Semantics::new(Role::Label).name(text));
        let appear = Rc::new(move || seen.set(true));
        if self.whole {
            cx.on_appear_whole(node, appear);
        } else {
            cx.on_appear(node, appear);
        }
        node
    }
}

/// `1234567` as `1,234,567`.
fn group_thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A place links with `#name` scroll to, landing `offset` pixels below the top of
/// the scroll container. See [`BuildCx::anchor`].
#[derive(Debug)]
pub struct Anchor<C> {
    name: String,
    offset: f32,
    content: C,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> Anchor<C> {
    /// `content`, reachable as `#name`.
    pub fn new(name: impl Into<String>, content: C) -> Self {
        Self { name: name.into(), offset: 0.0, content }
    }

    /// Lands this far below the top, below a bar pinned over the page.
    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }
}

impl<C: View> View for Anchor<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.content);
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[child],
            Paint::None,
        );
        cx.anchor(node, &self.name, self.offset);
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(100_000), "100,000");
        assert_eq!(group_thousands(-1_234_567), "-1,234,567");
    }

    #[test]
    fn a_float_returns_to_its_start_each_period() {
        let link = TimeLink::float(6.0, 2.0);
        let frame = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(link.effect_at(0.0, frame).offset.y.abs() < 1e-5);
        assert!((link.effect_at(0.5, frame).offset.y - 6.0).abs() < 1e-4);
        assert!(link.effect_at(2.0, frame).offset.y.abs() < 1e-4);
    }
}
