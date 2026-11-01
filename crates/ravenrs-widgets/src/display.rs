//! Components that present information: badges, avatars, alerts, stats, progress.

use ravenrs_core::{Border, Color, Insets, Quad, Rect, Role};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{IconName, View, ViewSeq, util};

/// Tone of a badge or alert. Status tones always come with a word or an icon,
/// never color alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
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
pub struct Badge {
    text: String,
    tone: Tone,
    icon: Option<IconName>,
}

impl Badge {
    /// A neutral badge with this text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), tone: Tone::Neutral, icon: None }
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

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

impl View for Badge {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let color = self.tone.color(&theme);
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(util::icon(cx, icon, 12.0, color));
        }
        children.push(util::label(cx, &self.text, TextStyle::Caption, Some(Weight::SEMIBOLD), color));
        let style = LayoutStyle {
            padding: Insets::symmetric(3.0, 9.0),
            spacing: 4.0,
            align: Align::Center,
            align_self: Some(Align::Fit),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            fill: color.with_alpha(0.14).into(),
            corner_radius: 999.0,
            border: Some(Border { width: 1.0, color: color.with_alpha(0.28) }),
            shadow: None,
        };
        cx.container(style, &children, Paint::Quad(paint))
    }
}

/// A round avatar showing initials on a color picked from the name.
#[derive(Clone, Debug)]
pub struct Avatar {
    name: String,
    size: f32,
}

impl Avatar {
    /// An avatar showing the initials of `name`, 36 logical pixels across.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), size: 36.0 }
    }

    /// Sets the diameter in logical pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
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
        let text = cx.text(&self.initials(), &attrs, base.lerp(Color::WHITE, 0.75), LayoutStyle::leaf());
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
            border: Some(Border { width: 1.0, color: base.with_alpha(0.5) }),
            shadow: None,
        };
        let node = cx.container(style, &[text], Paint::Quad(paint));
        cx.semantics(text, Semantics::hidden());
        cx.semantics(node, Semantics::new(Role::Image).name(self.name.clone()));
        node
    }
}

/// A keyboard key, for shortcuts: `Kbd::new("⌘K")`.
#[derive(Clone, Debug)]
pub struct Kbd(String);

impl Kbd {
    /// A key cap showing `keys`.
    pub fn new(keys: impl Into<String>) -> Self {
        Self(keys.into())
    }
}

impl View for Kbd {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let text = util::label(
            cx,
            &self.0,
            TextStyle::Caption,
            Some(Weight::MEDIUM),
            theme.color(Semantic::InkMuted),
        );
        let style = LayoutStyle {
            padding: Insets { top: 1.0, bottom: 2.0, left: 6.0, right: 6.0 },
            align_self: Some(Align::Center),
            ..LayoutStyle::stack(Axis::Horizontal)
        };
        let paint = QuadPaint {
            fill: theme.color(Semantic::Control).into(),
            corner_radius: theme.radii.sm,
            border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
            shadow: None,
        };
        cx.container(style, &[text], Paint::Quad(paint))
    }
}

/// A hairline separator. Horizontal dividers stretch across their stack.
#[derive(Clone, Copy, Debug, Default)]
pub struct Divider {
    vertical: bool,
}

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

/// A horizontal progress bar; `value` from 0.0 to 1.0.
#[derive(Clone, Copy, Debug)]
pub struct ProgressBar {
    value: f32,
    tone: Tone,
    height: f32,
}

impl ProgressBar {
    /// A progress bar at `value`, clamped to 0.0–1.0.
    pub fn new(value: f32) -> Self {
        Self { value: value.clamp(0.0, 1.0), tone: Tone::Accent, height: 6.0 }
    }

    /// Sets the fill color tone (accent by default).
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl View for ProgressBar {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let track = theme.color(Semantic::Control);
        let fill =
            if self.tone == Tone::Accent { theme.color(Semantic::Accent) } else { self.tone.color(&theme) };
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
        cx.semantics(
            node,
            Semantics::new(Role::ProgressIndicator)
                .numeric(percent, 0.0, 100.0, None)
                .value(format!("{percent}%")),
        );
        node
    }
}

/// A placeholder block shown while content loads.
#[derive(Clone, Copy, Debug)]
pub struct Skeleton {
    width: Option<f32>,
    height: f32,
    radius: Option<f32>,
}

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
#[derive(Clone, Debug)]
pub struct Alert {
    title: String,
    message: String,
    tone: Tone,
}

impl Alert {
    /// An accent-toned alert with a title and a message.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { title: title.into(), message: message.into(), tone: Tone::Accent }
    }

    /// Sets the color tone and icon.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl View for Alert {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let color = self.tone.color(&theme);
        let icon = util::icon(cx, self.tone.icon(), 18.0, color);
        let title =
            util::label(cx, &self.title, TextStyle::Body, Some(Weight::SEMIBOLD), theme.color(Semantic::Ink));
        let message =
            util::label(cx, &self.message, TextStyle::Callout, None, theme.color(Semantic::InkMuted));
        let text = cx.container(
            LayoutStyle { spacing: 2.0, grow: 1.0, ..LayoutStyle::stack(Axis::Vertical) },
            &[title, message],
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
        let node = cx.container(style, &[icon, text], Paint::Quad(paint));
        let role = if self.tone == Tone::Danger { Role::Alert } else { Role::Status };
        cx.semantics(node, Semantics::new(role));
        node
    }
}

/// A headline number with a label and an optional change indicator.
#[derive(Clone, Debug)]
pub struct Stat {
    label: String,
    value: String,
    delta: Option<(String, bool, bool)>,
}

impl Stat {
    /// A stat showing `value` under `label`.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into(), delta: None }
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
        value_attrs.size = 30.0;
        value_attrs.line_height = 36.0;
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

/// A titled group of content with an optional description.
pub struct Section<C> {
    title: String,
    description: Option<String>,
    content: C,
}

impl<C> std::fmt::Debug for Section<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Section").field("title", &self.title).finish_non_exhaustive()
    }
}

impl<C: ViewSeq> Section<C> {
    /// A section titled `title` around `content`.
    pub fn new(title: impl Into<String>, content: C) -> Self {
        Self { title: title.into(), description: None, content }
    }

    /// Adds a muted description under the title.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
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
        let mut children = vec![header];
        children.extend(cx.children(&self.content));
        let style =
            LayoutStyle { spacing: 16.0, align: Align::Stretch, ..LayoutStyle::stack(Axis::Vertical) };
        cx.container(style, &children, Paint::None)
    }
}
