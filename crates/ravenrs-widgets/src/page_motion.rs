//! Motion for showcase pages: effects that follow the clock ([`TimeLink`]), a strip
//! that keeps moving ([`Ticker`]), a number that counts up when it is seen
//! ([`CountUp`]) and a place links scroll to ([`Anchor`]).

use std::fmt;
use std::rc::Rc;

use ravenrs_animation::Easing;
use ravenrs_core::{Effect, Rect};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint};
use crate::{Text, View};

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
#[derive(Debug)]
pub struct Ticker<C> {
    content: C,
    speed: f32,
    gap: f32,
    reverse: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View> Ticker<C> {
    /// A strip of `content` moving left at 40 pixels a second.
    pub fn new(content: C) -> Self {
        Self { content, speed: 40.0, gap: 16.0, reverse: false }
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
        cx.time_linked(
            strip,
            TimeLink::new(move |t, frame| {
                // One copy and its gap: after that distance the strip looks the same again.
                let span = (frame.width() + gap) / 2.0;
                if span <= 0.0 {
                    return Effect::IDENTITY;
                }
                let x = (t as f32 * speed) % span;
                Effect::moved(if reverse { x - span } else { -x }, 0.0)
            }),
        );
        let node = cx.container(LayoutStyle::stack(Axis::Horizontal), &[strip], Paint::None);
        cx.clip(node);
        node
    }
}

/// A number that counts up from zero to its value, once, when it first shows on
/// screen. Under reduced motion it shows the value at once.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct CountUp {
    value: f64,
    format: Rc<dyn Fn(f64) -> String>,
    style: TextStyle,
    seconds: f64,
    easing: Easing,
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
        }
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

    /// How long the count takes (1.5 seconds by default).
    pub fn seconds(mut self, seconds: f64) -> Self {
        self.seconds = seconds.max(0.0);
        self
    }

    /// The pace of the count (fast, then settling, by default).
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

impl View for CountUp {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let seen = cx.use_state(|| false);
        let start = cx.use_state(|| None::<f64>);
        let reduced = cx.reduced_motion();
        let now = cx.now();
        if seen.get() && start.get().is_none() {
            start.set(Some(now));
        }
        let shown = match start.get() {
            _ if reduced || self.seconds == 0.0 => self.value,
            None => 0.0,
            Some(began) => {
                let t = ((now - began) / self.seconds).clamp(0.0, 1.0) as f32;
                if t < 1.0 {
                    cx.wake_at(now + 1.0 / 60.0);
                }
                self.value * f64::from(self.easing.sample(t))
            }
        };
        let text = cx.child(&Text::new((self.format)(shown)).style(self.style).foreground(Semantic::Ink));
        let node = cx.container(
            LayoutStyle { align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
            &[text],
            Paint::None,
        );
        cx.on_appear(node, Rc::new(move || seen.set(true)));
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
