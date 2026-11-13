//! The zoom of a picture: [`ZoomView`], shared by everything that shows one, and [`ZoomControl`].

use ravenrs_core::{Point, Rect, Role, Size};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{Button, ButtonSize, ButtonStyle, IconButton, IconName, View, util};

/// Where zoom steps stop, as screen pixels per unit of content.
const ZOOM_STOPS: [f32; 17] =
    [0.05, 0.1, 0.125, 0.25, 0.33, 0.5, 0.67, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 16.0, 32.0, 64.0];
/// How far in a [`ZoomView`] goes by default, relative to the content's own size.
const ZOOM_DEFAULT_MAX: f32 = 16.0;

/// The next stop from `scale`, in the direction of `up` or down.
#[doc(hidden)]
pub fn zoom_stop(scale: f32, up: bool) -> f32 {
    let next = if up {
        ZOOM_STOPS.iter().copied().find(|s| *s > scale * 1.01)
    } else {
        ZOOM_STOPS.iter().rev().copied().find(|s| *s < scale * 0.99)
    };
    next.unwrap_or(scale)
}

/// How a picture is being looked at: how much it is enlarged and which part is in the middle.
/// `ZoomPanView`, [`ZoomControl`] and `Minimap` share one through a `Binding`, and each
/// changes what it is responsible for.
///
/// While `fit` is on, `ZoomPanView` keeps the whole content in view and writes the scale
/// this takes, so the value also reads correctly when the window is resized.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ZoomView {
    /// Screen pixels per unit of content: 1 is 100%.
    pub scale: f32,
    /// The point of the content in the middle of the view, from 0 to 1 on each axis.
    pub center: Point,
    /// Follows the view's size to show the whole content.
    pub fit: bool,
    /// The size of the view on screen, written by `ZoomPanView` and read by `Minimap`.
    pub viewport: Size,
}

impl Default for ZoomView {
    fn default() -> Self {
        Self::fit()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ZoomView {
    /// Shows the whole content.
    pub const fn fit() -> Self {
        Self { scale: 1.0, center: Point::new(0.5, 0.5), fit: true, viewport: Size::ZERO }
    }

    /// At this scale, centered.
    pub const fn at(scale: f32) -> Self {
        Self { scale, center: Point::new(0.5, 0.5), fit: false, viewport: Size::ZERO }
    }

    /// The same view around this point of the content, from 0 to 1 on each axis.
    pub const fn centered_on(mut self, center: Point) -> Self {
        self.center = center;
        self
    }

    /// Where the whole content lands, relative to the top left corner of the view.
    pub fn placed(&self, content: Size) -> Rect {
        let (w, h) = (content.width * self.scale, content.height * self.scale);
        Rect::new(
            self.viewport.width / 2.0 - self.center.x * w,
            self.viewport.height / 2.0 - self.center.y * h,
            w,
            h,
        )
    }

    /// The part of the content in view, from 0 to 1 on each axis.
    pub fn visible(&self, content: Size) -> Rect {
        // No view has measured itself yet, and nothing can be said to be out of it.
        if self.viewport.width <= 0.0 || self.viewport.height <= 0.0 {
            return Rect::new(0.0, 0.0, 1.0, 1.0);
        }
        let share = |view: f32, whole: f32| (view / (whole * self.scale).max(f32::EPSILON)).min(1.0);
        let (w, h) = (share(self.viewport.width, content.width), share(self.viewport.height, content.height));
        Rect::new(self.center.x - w / 2.0, self.center.y - h / 2.0, w, h)
    }

    /// The scale that shows the whole content in the view.
    #[doc(hidden)]
    pub fn fit_scale(&self, content: Size) -> f32 {
        let ratio = |view: f32, whole: f32| view / whole.max(f32::EPSILON);
        ratio(self.viewport.width, content.width).min(ratio(self.viewport.height, content.height))
    }

    /// This view within its limits: the scale in range and no empty space on an axis where
    /// the content is larger than the view, so it cannot run away.
    #[doc(hidden)]
    pub fn settled(mut self, viewport: Size, content: Size, range: ZoomRange) -> Self {
        self.viewport = viewport;
        let fit = self.fit_scale(content).max(f32::EPSILON);
        let (least, most) = range.limits(fit);
        self.scale = if self.fit { fit } else { self.scale.clamp(least, most) };
        let axis = |center: f32, view: f32, whole: f32| {
            let half = view / (2.0 * whole * self.scale).max(f32::EPSILON);
            if half >= 0.5 { 0.5 } else { center.clamp(half, 1.0 - half) }
        };
        self.center = Point::new(
            axis(self.center.x, viewport.width, content.width),
            axis(self.center.y, viewport.height, content.height),
        );
        self
    }

    /// At another scale, with the point of the content at `anchor` in the view staying there.
    #[doc(hidden)]
    pub fn zoomed(self, scale: f32, anchor: Point, content: Size) -> Self {
        let before = self.placed(content);
        let fraction = |at: f32, start: f32, size: f32| (at - start) / size.max(f32::EPSILON);
        let (fx, fy) =
            (fraction(anchor.x, before.x(), before.width()), fraction(anchor.y, before.y(), before.height()));
        let (w, h) = (content.width * scale, content.height * scale);
        let center = Point::new(
            fx + (self.viewport.width / 2.0 - anchor.x) / w.max(f32::EPSILON),
            fy + (self.viewport.height / 2.0 - anchor.y) / h.max(f32::EPSILON),
        );
        Self { scale, center, fit: false, ..self }
    }
}

/// How far a view may zoom: `least` unset means down to showing the whole content.
#[derive(Clone, Copy, Debug)]
#[doc(hidden)]
pub struct ZoomRange {
    #[doc(hidden)]
    pub least: Option<f32>,
    #[doc(hidden)]
    pub most: f32,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ZoomRange {
    #[doc(hidden)]
    pub const DEFAULT: Self = Self { least: None, most: ZOOM_DEFAULT_MAX };

    /// The limits for content that fits at scale `fit`, which is always reachable.
    fn limits(self, fit: f32) -> (f32, f32) {
        (self.least.unwrap_or(fit).min(fit), self.most.max(fit))
    }
}

/// A scale as people read it: `50%`, and `12,5%` with the decimal only when it matters.
#[doc(hidden)]
pub fn zoom_percent(scale: f32, decimal: char) -> String {
    let percent = scale * 100.0;
    if percent < 100.0 && (percent - percent.round()).abs() >= 0.05 {
        format!("{percent:.1}%").replace('.', &decimal.to_string())
    } else {
        format!("{}%", percent.round())
    }
}

// ---------------------------------------------------------------------------------------------
// ZoomControl
// ---------------------------------------------------------------------------------------------

/// Fit, 1:1, the current percentage and the buttons that zoom in and out, for a [`ZoomView`].
/// It works from a toolbar of its own or inside a
/// `FloatingToolbar`.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::{ZoomControl, ZoomView};
///
/// let view = State::new(ZoomView::fit());
/// let control = ZoomControl::new(view.binding()).range(0.1, 8.0);
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ZoomControl {
    view: Binding<ZoomView>,
    least: f32,
    #[doc(hidden)]
    pub most: f32,
    size: ButtonSize,
    label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ZoomControl {
    /// A control bound to `view`, with small buttons.
    pub fn new(view: Binding<ZoomView>) -> Self {
        Self {
            view,
            least: ZOOM_STOPS[0],
            most: ZOOM_STOPS[ZOOM_STOPS.len() - 1],
            size: ButtonSize::Small,
            label: None,
        }
    }

    /// The least and the most its buttons reach, as a scale where 1 is 100%. Match it to
    /// the range of the `ZoomPanView` that shares the view.
    pub fn range(mut self, least: f32, most: f32) -> Self {
        self.least = least;
        self.most = most.max(least);
        self
    }

    /// The size of its buttons.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl View for ZoomControl {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let current = self.view.get();
        let (least, most) = (self.least, self.most);
        let change = |f: fn(&mut ZoomView, f32, f32)| {
            let view = self.view.clone();
            move || view.update(|v| f(v, least, most))
        };
        let fit = Button::new(labels.zoom_fit.clone())
            .style(ButtonStyle::Ghost)
            .size(self.size)
            .selected(current.fit)
            .on_click(change(|v, _, _| v.fit = true));
        let actual = Button::new("1:1")
            .accessible_name(labels.zoom_actual.clone())
            .style(ButtonStyle::Ghost)
            .size(self.size)
            .selected(!current.fit && (current.scale - 1.0).abs() < 0.001)
            .on_click(change(|v, _, _| *v = ZoomView { scale: 1.0, fit: false, ..*v }));
        let out = IconButton::new(IconName::Minus)
            .label(labels.zoom_out.clone())
            .size(self.size)
            .disabled(current.scale <= least * 1.001)
            .on_click(change(|v, least, _| {
                *v = ZoomView { scale: zoom_stop(v.scale, false).max(least), fit: false, ..*v };
            }));
        let more = IconButton::new(IconName::Plus)
            .label(labels.zoom_in.clone())
            .size(self.size)
            .disabled(current.scale >= most * 0.999)
            .on_click(change(|v, _, most| {
                *v = ZoomView { scale: zoom_stop(v.scale, true).min(most), fit: false, ..*v };
            }));
        let nodes = [cx.child(&fit), cx.child(&actual)];
        let line = cx.container(
            LayoutStyle { width: Some(1.0), height: Some(16.0), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Border))),
        );
        let out = cx.child(&out);
        let percent = zoom_percent(current.scale, labels.decimal_separator);
        let attrs = util::attrs(&theme, TextStyle::Callout, None);
        // As wide as the widest percentage, so the buttons do not move as it changes.
        let widest = cx.measure_text("1600%", &attrs).width.ceil();
        let text = util::label(cx, &percent, TextStyle::Callout, None, theme.color(Semantic::Ink));
        let number = cx.container(
            LayoutStyle {
                min_width: Some(widest),
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[text],
            Paint::None,
        );
        let more = cx.child(&more);
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s1,
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[nodes[0], nodes[1], line, out, number, more],
            Paint::None,
        );
        let name = self.label.clone().unwrap_or_else(|| labels.zoom.clone());
        cx.semantics(node, Semantics::new(Role::Group).name(name));
        node
    }
}

// ---------------------------------------------------------------------------------------------
// ZoomPanView
// ---------------------------------------------------------------------------------------------
