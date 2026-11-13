//! Components that present information: badges, avatars, alerts, stats, progress.

use std::rc::Rc;

use ravenrs_core::{Border, Color, Insets, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{Button, ButtonSize, ButtonStyle, Fill, IconName, Image, View, ViewSeq, util};

/// How far a badge's text goes from its tone toward the ink.
const BADGE_TEXT_TOWARD_INK: f32 = 0.3;
/// How far an avatar's initials go from its color toward the ink.
const AVATAR_TEXT_TOWARD_INK: f32 = 0.5;

/// Tone of a badge or alert. Status tones always come with a word or an icon,
/// never color alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Tone {
    /// Gray, for neutral information.
    #[default]
    Neutral,
    /// The theme accent color.
    Accent,
    /// Green, for success.
    Success,
    /// Amber, for warnings.
    Warning,
    /// Red, for errors and destructive states.
    Danger,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Tone {
    pub(crate) fn color(self, theme: &Theme) -> Color {
        match self {
            Tone::Neutral => theme.color(Semantic::InkMuted),
            Tone::Accent => theme.color(Semantic::FocusRing).lerp(theme.color(Semantic::Accent), 0.35),
            Tone::Success => theme.color(Semantic::Success),
            Tone::Warning => theme.color(Semantic::Warning),
            Tone::Danger => theme.color(Semantic::Danger),
        }
    }

    fn icon(self) -> IconName {
        match self {
            Tone::Neutral | Tone::Accent => IconName::Info,
            Tone::Success => IconName::CircleCheck,
            Tone::Warning => IconName::TriangleAlert,
            Tone::Danger => IconName::CircleAlert,
        }
    }
}

/// A small pill label: status, counts, categories.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Badge {
    text: String,
    tone: Tone,
    icon: Option<IconName>,
    color: Option<Color>,
    outline: bool,
    height: Option<f32>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Badge {
    /// A neutral badge with this text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), tone: Tone::Neutral, icon: None, color: None, outline: false, height: None }
    }

    /// Sets the color tone.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Shows an icon before the text.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Uses this color instead of the tone's, for categories that have their own.
    /// The text moves toward the ink, so that it stays readable on the tint.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Only the outline, without the tinted fill.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// Fixes the height in logical pixels, to line up with what is beside it.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

impl View for Badge {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let color = self.color.unwrap_or_else(|| self.tone.color(&theme));
        // Toward the ink, so that small text on its own tint keeps 4.5:1 in every theme.
        let ink = color.lerp(theme.color(Semantic::Ink), BADGE_TEXT_TOWARD_INK);
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, 12.0, ink));
        }
        children.push(util::label(cx, &self.text, TextStyle::Caption, Some(Weight::SEMIBOLD), ink));
        let style = LayoutStyle {
            padding: Insets::symmetric(if self.height.is_some() { 0.0 } else { 3.0 }, 9.0),
            height: self.height,
            spacing: 4.0,
            align: Align::Center,
            align_self: Some(Align::Fit),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let (fill, edge) =
            if self.outline { (Color::TRANSPARENT, 0.6) } else { (color.with_alpha(0.14), 0.28) };
        let paint = QuadPaint {
            fill: fill.into(),
            corner_radius: 999.0,
            border: Some(Border { width: 1.0, color: color.with_alpha(edge) }),
            shadow: None,
        };
        cx.container(style, &children, Paint::Quad(paint))
    }
}

/// A round avatar showing a picture, or initials on a color picked from the name.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Avatar {
    name: String,
    size: f32,
    image: Option<Image>,
    border: Option<(f32, Fill)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Avatar {
    /// An avatar showing the initials of `name`, 36 logical pixels across.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), size: 36.0, image: None, border: None }
    }

    /// Sets the diameter in logical pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Shows this picture instead of the initials, cropped to the circle. The name still
    /// tells screen readers who it is.
    pub fn image(mut self, image: Image) -> Self {
        self.image = Some(image);
        self
    }

    /// Draws a ring of `width` logical pixels, as on the selected one of a row. A picture
    /// shrinks to fit inside it.
    pub fn border(mut self, width: f32, fill: impl Into<Fill>) -> Self {
        self.border = Some((width, fill.into()));
        self
    }

    /// The picture inside its ring, which would otherwise paint over the ring.
    fn draw_image(&self, cx: &mut BuildCx<'_>, image: &Image) -> NodeId {
        let theme = *cx.theme();
        let ring = self.border.map(|(width, fill)| Border { width, color: fill.resolve(&theme) });
        let inset = ring.map_or(0.0, |r| r.width);
        let inner = (self.size - 2.0 * inset).max(0.0);
        let picture = cx.child(
            &image
                .clone()
                .size(inner, inner)
                .corner_radius(inner / 2.0)
                .content_mode(crate::ContentMode::Cover),
        );
        cx.semantics(picture, Semantics::hidden());
        let style = LayoutStyle {
            width: Some(self.size),
            height: Some(self.size),
            padding: Insets::uniform(inset),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            fill: Color::TRANSPARENT.into(),
            corner_radius: self.size / 2.0,
            border: ring,
            shadow: None,
        };
        let node = cx.container(style, &[picture], Paint::Quad(paint));
        cx.semantics(node, Semantics::new(Role::Image).name(self.name.clone()));
        node
    }

    fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .flat_map(char::to_uppercase)
            .collect()
    }
}

impl View for Avatar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if let Some(image) = &self.image {
            return self.draw_image(cx, image);
        }
        let theme = *cx.theme();
        let palette = [
            theme.color(Semantic::Accent),
            theme.color(Semantic::Success),
            theme.color(Semantic::Warning),
            theme.color(Semantic::Danger),
            theme.color(Semantic::FocusRing),
        ];
        let hash = self.name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
        let base = palette[(hash as usize) % palette.len()];
        let mut attrs = util::attrs(&theme, TextStyle::Body, Some(Weight::SEMIBOLD));
        attrs.size = (self.size * 0.4).round();
        attrs.line_height = attrs.size * 1.2;
        let text = cx.text(
            &self.initials(),
            &attrs,
            base.lerp(theme.color(Semantic::Ink), AVATAR_TEXT_TOWARD_INK),
            LayoutStyle::leaf(),
        );
        let style = LayoutStyle {
            width: Some(self.size),
            height: Some(self.size),
            align: Align::Center,
            justify: Justify::Center,
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            fill: base.with_alpha(0.28).into(),
            corner_radius: self.size / 2.0,
            border: Some(match self.border {
                Some((width, fill)) => Border { width, color: fill.resolve(&theme) },
                None => Border { width: 1.0, color: base.with_alpha(0.5) },
            }),
            shadow: None,
        };
        let node = cx.container(style, &[text], Paint::Quad(paint));
        cx.semantics(text, Semantics::hidden());
        cx.semantics(node, Semantics::new(Role::Image).name(self.name.clone()));
        node
    }
}

/// A hairline separator. Horizontal dividers stretch across their stack.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Divider {
    vertical: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Divider {
    /// A horizontal line that stretches across its stack.
    pub fn horizontal() -> Self {
        Self { vertical: false }
    }

    /// A vertical line.
    pub fn vertical() -> Self {
        Self { vertical: true }
    }
}

impl View for Divider {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let color = cx.theme().color(Semantic::Border);
        let style = if self.vertical {
            LayoutStyle { width: Some(1.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        } else {
            LayoutStyle { height: Some(1.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() }
        };
        let node = cx.container(style, &[], Paint::Quad(QuadPaint::new(color)));
        cx.semantics(node, Semantics::new(Role::Separator));
        node
    }
}

/// A horizontal progress bar; `value` from 0.0 to 1.0. It can carry a label and a
/// percentage above it and a detail below.
///
/// ```
/// use ravenrs_widgets::{ProgressBar, Ui};
///
/// let bar = ProgressBar::new(0.4).label("Copying").percent().detail("12 of 30 photos").height(4.0);
/// let mut ui = Ui::new(bar, ravenrs_style::Theme::raven_dark());
/// ui.update();
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ProgressBar {
    value: f32,
    tone: Tone,
    height: f32,
    color: Option<Color>,
    label: Option<String>,
    detail: Option<String>,
    percent: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ProgressBar {
    /// A progress bar at `value`, clamped to 0.0–1.0.
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            tone: Tone::Accent,
            height: 6.0,
            color: None,
            label: None,
            detail: None,
            percent: false,
        }
    }

    /// Sets the fill color tone (accent by default).
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Fills with this color instead of the tone's.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets the thickness of the bar in logical pixels (6 by default).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(1.0);
        self
    }

    /// What is progressing, above the bar. It also names it for screen readers.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Shows the percentage above the bar, at its end.
    pub fn percent(mut self) -> Self {
        self.percent = true;
        self
    }

    /// A line in small text below the bar, such as how many are done.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl View for ProgressBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let track = theme.color(Semantic::Control);
        let fill = self.color.unwrap_or_else(|| {
            if self.tone == Tone::Accent { theme.color(Semantic::Accent) } else { self.tone.color(&theme) }
        });
        let h = self.height;
        let value = cx.animated(self.value, theme.motion.snappy).clamp(0.0, 1.0);
        let style = LayoutStyle { height: Some(h), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() };
        let node = cx.container(
            style,
            &[],
            Paint::Custom(std::rc::Rc::new(move |frame: Rect, scene| {
                scene.push_quad(Quad::new(frame, track).radius(h / 2.0));
                if value > 0.0 {
                    let w = (frame.width() * value).max(h);
                    let bar = Rect::new(frame.x(), frame.y(), w, h);
                    scene.push_quad(Quad::new(bar, fill).radius(h / 2.0));
                }
            })),
        );
        let percent = (f64::from(self.value) * 100.0).round();
        let mut semantics = Semantics::new(Role::ProgressIndicator)
            .numeric(percent, 0.0, 100.0, None)
            .value(format!("{percent}%"));
        if let Some(label) = &self.label {
            semantics = semantics.name(label.clone());
        }
        if let Some(detail) = &self.detail {
            semantics = semantics.description(detail.clone());
        }
        if self.label.is_none() && self.detail.is_none() && !self.percent {
            cx.semantics(node, semantics);
            return node;
        }
        let (ink, muted) = (theme.color(Semantic::Ink), theme.color(Semantic::InkMuted));
        let mut rows = Vec::with_capacity(3);
        if self.label.is_some() || self.percent {
            let mut head = Vec::with_capacity(2);
            let name = self.label.as_deref().unwrap_or_default();
            let name = util::label(cx, name, TextStyle::Callout, None, ink);
            cx.semantics(name, Semantics::hidden());
            head.push(cx.container(
                LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::stack(Axis::Horizontal) },
                &[name],
                Paint::None,
            ));
            if self.percent {
                let number = util::label(cx, &format!("{percent}%"), TextStyle::Callout, None, muted);
                cx.semantics(number, Semantics::hidden());
                head.push(number);
            }
            rows.push(cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s2,
                    align_self: Some(Align::Stretch),
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &head,
                Paint::None,
            ));
        }
        rows.push(node);
        if let Some(detail) = &self.detail {
            let line = util::label(cx, detail, TextStyle::Caption, None, muted);
            cx.semantics(line, Semantics::hidden());
            rows.push(line);
        }
        let column = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s1,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &rows,
            Paint::None,
        );
        cx.semantics(column, semantics);
        column
    }
}

/// A placeholder block shown while content loads.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Skeleton {
    width: Option<f32>,
    height: f32,
    radius: Option<f32>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Skeleton {
    /// A block that stretches across its stack.
    pub fn line(height: f32) -> Self {
        Self { width: None, height, radius: None }
    }

    /// A rectangle of fixed size in logical pixels.
    pub fn block(width: f32, height: f32) -> Self {
        Self { width: Some(width), height, radius: None }
    }

    /// A circle `size` logical pixels across.
    pub fn circle(size: f32) -> Self {
        Self { width: Some(size), height: size, radius: Some(size / 2.0) }
    }
}

impl View for Skeleton {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let style = LayoutStyle {
            width: self.width,
            height: Some(self.height),
            align_self: self.width.is_none().then_some(Align::Stretch),
            ..LayoutStyle::leaf()
        };
        let radius = self.radius.unwrap_or(theme.radii.sm);
        let (base, ink) = (theme.color(Semantic::Control), theme.color(Semantic::Ink));
        let node = cx.container(
            style,
            &[],
            Paint::Live(std::rc::Rc::new(move |frame: Rect, time, scene| {
                // A slow breath, which starts at rest under reduced motion.
                let breath = (1.0 - (time * std::f64::consts::TAU / 1.6).cos()) as f32 / 2.0;
                scene.push_quad(Quad::new(frame, base.lerp(ink, 0.08 * breath)).radius(radius));
            })),
        );
        cx.semantics(node, Semantics::hidden());
        node
    }
}

/// A tinted message box with an icon, a title and a description.
#[derive(Clone)]
pub struct Alert<A = ()> {
    title: String,
    message: String,
    tone: Tone,
    compact: bool,
    action: A,
}

/// What [`Alert::action`] adds to an alert. Its own type parameter, so an alert
/// without a button stays `Send`.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AlertAction {
    label: String,
    on_click: Rc<dyn Fn()>,
}

impl std::fmt::Debug for AlertAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlertAction").field("label", &self.label).finish_non_exhaustive()
    }
}

impl<A> std::fmt::Debug for Alert<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Alert").field("title", &self.title).field("tone", &self.tone).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Alert {
    /// An accent-toned alert with a title and a message.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { title: title.into(), message: message.into(), tone: Tone::Accent, compact: false, action: () }
    }

    /// An alert that is only its message, for when a title would just repeat where it is.
    pub fn message(message: impl Into<String>) -> Self {
        Self::new(String::new(), message)
    }

    /// A button beside the message, so that what it asks for is one click away.
    #[must_use]
    pub fn action(self, label: impl Into<String>, on_click: impl Fn() + 'static) -> Alert<AlertAction> {
        Alert {
            title: self.title,
            message: self.message,
            tone: self.tone,
            compact: self.compact,
            action: AlertAction { label: label.into(), on_click: Rc::new(on_click) },
        }
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1", with = "A = (); A = crate::AlertAction"))]
impl<A> Alert<A> {
    /// Sets the color tone and icon.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// A thin strip in small text with a stripe at its edge, for a warning that sits
    /// right next to what it is about, as in a side panel.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// The strip of [`Self::compact`]. The fill stays a tint: a solid color in a dark
    /// panel would shout over the text it carries.
    fn draw_compact(&self, cx: &mut BuildCx<'_>, action: Option<&AlertAction>) -> NodeId {
        let theme = *cx.theme();
        let color = self.tone.color(&theme);
        let ink = color.lerp(theme.color(Semantic::Ink), BADGE_TEXT_TOWARD_INK);
        let stripe = cx.container(
            LayoutStyle { width: Some(2.0), align_self: Some(Align::Stretch), ..LayoutStyle::leaf() },
            &[],
            Paint::Quad(QuadPaint::new(color)),
        );
        let icon = util::icon(cx, self.tone.icon(), 14.0, color);
        let mut lines = Vec::with_capacity(2);
        if !self.title.is_empty() {
            lines.push(util::label(cx, &self.title, TextStyle::Caption, Some(Weight::SEMIBOLD), ink));
        }
        lines.push(util::label(cx, &self.message, TextStyle::Caption, None, ink));
        let text = cx.container(
            LayoutStyle { grow: 1.0, min_width: Some(0.0), ..LayoutStyle::stack(Axis::Vertical) },
            &lines,
            Paint::None,
        );
        let mut items = vec![icon, text];
        if let Some(action) = action {
            let on_click = action.on_click.clone();
            let button = Button::new(action.label.clone())
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Mini)
                .on_click(move || on_click());
            items.push(cx.child(&button));
        }
        let inner = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(theme.spacing.s1, theme.spacing.s2),
                spacing: theme.spacing.s2,
                grow: 1.0,
                min_width: Some(0.0),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &items,
            Paint::None,
        );
        let node = cx.container(
            LayoutStyle {
                align_self: Some(Align::Stretch),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[stripe, inner],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.sm,
                ..QuadPaint::new(color.with_alpha(0.15))
            }),
        );
        cx.clip(node);
        let role = if self.tone == Tone::Danger { Role::Alert } else { Role::Status };
        cx.semantics(node, Semantics::new(role));
        node
    }

    fn draw(&self, cx: &mut BuildCx<'_>, action: Option<&AlertAction>) -> NodeId {
        if self.compact {
            return self.draw_compact(cx, action);
        }
        let theme = *cx.theme();
        let color = self.tone.color(&theme);
        let icon = util::icon(cx, self.tone.icon(), 18.0, color);
        let mut lines = Vec::with_capacity(2);
        if self.title.is_empty() {
            lines.push(util::label(cx, &self.message, TextStyle::Body, None, theme.color(Semantic::Ink)));
        } else {
            lines.push(util::label(
                cx,
                &self.title,
                TextStyle::Body,
                Some(Weight::SEMIBOLD),
                theme.color(Semantic::Ink),
            ));
            lines.push(util::label(
                cx,
                &self.message,
                TextStyle::Callout,
                None,
                theme.color(Semantic::InkMuted),
            ));
        }
        let text = cx.container(
            LayoutStyle { spacing: 2.0, grow: 1.0, ..LayoutStyle::stack(Axis::Vertical) },
            &lines,
            Paint::None,
        );
        let style = LayoutStyle {
            padding: Insets::symmetric(14.0, 16.0),
            spacing: 12.0,
            align_self: Some(Align::Stretch),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            fill: color.with_alpha(0.10).into(),
            corner_radius: theme.radii.lg,
            border: Some(Border { width: 1.0, color: color.with_alpha(0.30) }),
            shadow: None,
        };
        let mut children = vec![icon, text];
        if let Some(action) = action {
            let on_click = action.on_click.clone();
            let button = Button::new(action.label.clone())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .on_click(move || on_click());
            children.push(cx.child(&button));
        }
        let node = cx.container(style, &children, Paint::Quad(paint));
        let role = if self.tone == Tone::Danger { Role::Alert } else { Role::Status };
        cx.semantics(node, Semantics::new(role));
        node
    }
}

impl View for Alert {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.draw(cx, None)
    }
}

impl View for Alert<AlertAction> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.draw(cx, Some(&self.action))
    }
}

/// A headline number with a label and an optional change indicator.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Stat {
    label: String,
    value: String,
    delta: Option<(String, bool, bool)>,
    large: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Stat {
    /// A stat showing `value` under `label`.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into(), delta: None, large: false }
    }

    /// The one number a screen is about, in the `metric` size.
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// A change such as `"+12.4%"`; `up` picks the color and arrow.
    pub fn delta(mut self, text: impl Into<String>, up: bool) -> Self {
        self.delta = Some((text.into(), up, up));
        self
    }

    /// A change where going up is not always good, such as spending: `up` picks
    /// the arrow and `good` the color.
    pub fn delta_toned(mut self, text: impl Into<String>, up: bool, good: bool) -> Self {
        self.delta = Some((text.into(), up, good));
        self
    }
}

impl View for Stat {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let label = util::label(
            cx,
            &self.label.to_uppercase(),
            TextStyle::Caption,
            None,
            theme.color(Semantic::InkMuted),
        );
        let mut value_attrs = util::attrs(&theme, TextStyle::Metric, None);
        if !self.large {
            value_attrs.size = 30.0;
            value_attrs.line_height = 36.0;
        }
        let value = cx.text(&self.value, &value_attrs, theme.color(Semantic::Ink), LayoutStyle::leaf());
        let mut children = vec![label, value];
        if let Some((text, up, good)) = &self.delta {
            let badge = Badge::new(text.clone())
                .tone(if *good { Tone::Success } else { Tone::Danger })
                .icon(if *up { IconName::TrendingUp } else { IconName::TrendingDown });
            children.push(cx.child(&badge));
        }
        cx.container(
            LayoutStyle { spacing: 6.0, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}

/// A titled group of content with an optional description, and actions at the end of
/// its header.
pub struct Section<C> {
    title: String,
    description: Option<String>,
    actions: Vec<crate::AnyView>,
    content: C,
}

impl<C> std::fmt::Debug for Section<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Section").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: ViewSeq> Section<C> {
    /// A section titled `title` around `content`.
    pub fn new(title: impl Into<String>, content: C) -> Self {
        Self { title: title.into(), description: None, actions: Vec::new(), content }
    }

    /// Adds a muted description under the title.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Adds a view, usually a button, at the end of the header. Several line up in order.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.actions.push(Box::new(action));
        self
    }
}

impl<C: ViewSeq> View for Section<C> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let title = util::label(cx, &self.title, TextStyle::Title, None, theme.color(Semantic::Ink));
        cx.semantics(title, Semantics::new(Role::Heading).level(2));
        let mut header = vec![title];
        if let Some(d) = &self.description {
            header.push(util::label(cx, d, TextStyle::Body, None, theme.color(Semantic::InkMuted)));
        }
        let header = cx.container(
            LayoutStyle { spacing: 4.0, ..LayoutStyle::stack(Axis::Vertical) },
            &header,
            Paint::None,
        );
        let header = if self.actions.is_empty() {
            header
        } else {
            let header = cx.container(
                LayoutStyle { grow: 1.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) },
                &[header],
                Paint::None,
            );
            let mut row = vec![header];
            for (i, action) in self.actions.iter().enumerate() {
                row.push(cx.keyed_child(u64::MAX - i as u64, action.as_ref()));
            }
            cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s2,
                    align: Align::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &row,
                Paint::None,
            )
        };
        let mut children = vec![header];
        children.extend(cx.children(&self.content));
        let style =
            LayoutStyle { spacing: 16.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) };
        cx.container(style, &children, Paint::None)
    }
}
