//! Pieces that show a state at a glance: dots, ratings, tiles, empty areas and banners.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, GlyphRun, Insets, Key, PathFill, Point, Primitive, Rect, Role, Scene, Size,
};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_reactive::Binding;
use ravenrs_style::{Semantic, TextStyle, Theme, Weight};
use ravenrs_text::TextAttrs;

use crate::context::{BuildCx, NodeId, Paint, QuadPaint, Semantics};
use crate::{AnyView, Date, FloatingCorner, IconName, ProgressBar, Tone, View, util};

/// How much of a color remains in an unlit star.
const UNLIT_STAR: f32 = 0.35;
/// One pass of the light over a [`ShimmerText`], in seconds.
const SHIMMER_PERIOD: f64 = 1.8;
/// How far the light of a [`ShimmerText`] reaches to each side, in logical pixels.
const SHIMMER_REACH: f32 = 36.0;
/// Below this the light on a glyph would not show, so the glyph is not drawn twice.
const SHIMMER_FLOOR: f32 = 0.04;

/// A small round mark of state, such as "not saved".
///
/// A color alone tells a screen reader nothing, so give a dot that means something a
/// [`label`](Self::label). Without one it is left out of the accessibility tree.
///
/// ```
/// use ravenrs_widgets::{Dot, Tone};
///
/// let unsaved = Dot::new().tone(Tone::Warning).label("Not saved");
/// ```
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Dot {
    tone: Tone,
    color: Option<Color>,
    size: Option<f32>,
    label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Dot {
    /// A neutral dot, 8 logical pixels across.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the color tone.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Uses this color instead of the tone's.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets the diameter in logical pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    /// What the dot means, for screen readers.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl View for Dot {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let color = self.color.unwrap_or_else(|| self.tone.color(&theme));
        let size = self.size.unwrap_or(theme.spacing.s2);
        let style = LayoutStyle {
            width: Some(size),
            height: Some(size),
            align_self: Some(Align::Center),
            ..LayoutStyle::leaf()
        };
        let paint = QuadPaint { corner_radius: size / 2.0, ..QuadPaint::new(color) };
        let node = cx.container(style, &[], Paint::Quad(paint));
        cx.semantics(
            node,
            match &self.label {
                Some(label) => Semantics::new(Role::Image).name(label.clone()),
                None => Semantics::hidden(),
            },
        );
        node
    }
}

/// A small view on a corner of another, such as a channel icon on an avatar or a
/// count on a button. It sits centered on the corner, half outside.
///
/// ```
/// use ravenrs_widgets::{Avatar, Dot, FloatingCorner, OverlayBadge, Tone};
///
/// let online = OverlayBadge::new(Avatar::new("Ana Lima"), Dot::new().tone(Tone::Success).label("Online"))
///     .corner(FloatingCorner::BottomRight);
/// ```
#[derive(Debug)]
pub struct OverlayBadge<C, B> {
    content: C,
    badge: B,
    corner: FloatingCorner,
    inset: f32,
    visible: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl<C: View, B: View> OverlayBadge<C, B> {
    /// `badge` on the top right corner of `content`.
    pub fn new(content: C, badge: B) -> Self {
        Self { content, badge, corner: FloatingCorner::TopRight, inset: 0.0, visible: true }
    }

    /// Which corner it sits on.
    pub fn corner(mut self, corner: FloatingCorner) -> Self {
        self.corner = corner;
        self
    }

    /// Moves it toward the middle of the content, in logical pixels, as for a round
    /// content whose corners are empty.
    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }

    /// Hides the badge and keeps the content, as when a count reaches zero.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

impl<C: View, B: View> View for OverlayBadge<C, B> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = cx.child(&self.content);
        let mut children = vec![content];
        if self.visible {
            let badge = cx.child(&self.badge);
            // The corner depends on the content's size, which the last layout reports.
            let size = cx.scroll_viewport().unwrap_or(Size::ZERO);
            let right = matches!(self.corner, FloatingCorner::TopRight | FloatingCorner::BottomRight);
            let bottom = matches!(self.corner, FloatingCorner::BottomLeft | FloatingCorner::BottomRight);
            let x = if right { size.width - self.inset } else { self.inset };
            let y = if bottom { size.height - self.inset } else { self.inset };
            // A box of no size: what is inside centers on the corner without knowing its own size.
            children.push(cx.container(
                LayoutStyle {
                    absolute: Some((x, y)),
                    width: Some(0.0),
                    height: Some(0.0),
                    align: Align::Center,
                    justify: Justify::Center,
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &[badge],
                Paint::None,
            ));
        }
        let root = cx.container(
            LayoutStyle { align_self: Some(Align::Fit), ..LayoutStyle::stack(Axis::Horizontal) },
            &children,
            Paint::None,
        );
        cx.report_frame(root);
        root
    }
}

/// Stars for a score, to read or to give.
///
/// Giving one works with the pointer, the arrow keys, Home and End, and the digits.
///
/// ```
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::Rating;
///
/// let score = State::new(3u32);
/// let editable = Rating::new(score.binding()).label("Photo rating").clearable();
/// let shown = Rating::read_only(4);
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Rating {
    value: Option<Binding<u32>>,
    fixed: u32,
    max: u32,
    size: f32,
    tone: Tone,
    color: Option<Color>,
    label: Option<String>,
    clearable: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Rating {
    fn with(value: Option<Binding<u32>>, fixed: u32) -> Self {
        Self {
            value,
            fixed,
            max: 5,
            size: 18.0,
            tone: Tone::Warning,
            color: None,
            label: None,
            clearable: false,
        }
    }

    /// Stars bound to `value`, from none up to the maximum.
    pub fn new(value: Binding<u32>) -> Self {
        Self::with(Some(value), 0)
    }

    /// Stars that only show `value`. They cost one element, however many are drawn, so
    /// they can go in every cell of a long list.
    pub fn read_only(value: u32) -> Self {
        Self::with(None, value)
    }

    /// Sets how many stars there are (5 by default).
    pub fn max(mut self, max: u32) -> Self {
        self.max = max;
        self
    }

    /// Sets the size of a star in logical pixels (18 by default).
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Sets the tone of the lit stars.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Uses this color for the lit stars, as when what is behind them changes.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// What is being rated, for screen readers.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Clicking the current score takes it away.
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }

    fn colors(&self, theme: &Theme) -> (Color, Color) {
        let lit = self.color.unwrap_or_else(|| self.tone.color(theme));
        (lit, theme.color(Semantic::InkMuted).with_alpha(UNLIT_STAR))
    }

    fn read_only_view(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let (lit, unlit) = self.colors(&theme);
        let (max, size) = (self.max, self.size);
        let value = self.fixed.min(max);
        let node = cx.container(
            LayoutStyle {
                width: Some(max as f32 * size),
                height: Some(size),
                align_self: Some(Align::Center),
                ..LayoutStyle::leaf()
            },
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                for star in 0..max {
                    let at = Point::new(frame.x() + (star as f32 + 0.5) * size, frame.center().y);
                    let color = if star < value { lit } else { unlit };
                    scene.push_path(PathFill::solid(util::star_path(at, size * 0.48), color));
                }
            })),
        );
        let labels = cx.labels();
        let name = self.label.clone().unwrap_or_else(|| labels.rating.clone());
        cx.semantics(node, Semantics::new(Role::Image).name(name).value(labels.rating_value(value, max)));
        node
    }

    fn editable_view(&self, cx: &mut BuildCx<'_>, binding: &Binding<u32>) -> NodeId {
        let theme = *cx.theme();
        let (lit, unlit) = self.colors(&theme);
        let (max, clearable) = (self.max, self.clearable);
        let value = binding.get().min(max);
        let preview = cx.use_state(|| None::<u32>);
        // Where the pointer is shows what a click would give, until it leaves.
        let shown = if cx.is_hover_within() { preview.get().unwrap_or(value) } else { value };
        let stars: Vec<NodeId> = (0..max)
            .map(|star| {
                let score = star + 1;
                cx.child(&Star {
                    size: self.size,
                    color: if star < shown { lit } else { unlit },
                    preview: {
                        let preview = preview.clone();
                        Rc::new(move || {
                            if preview.get() != Some(score) {
                                preview.set(Some(score));
                            }
                        })
                    },
                    pick: {
                        let binding = binding.clone();
                        Rc::new(move || {
                            let again = clearable && binding.get() == score;
                            binding.set(if again { 0 } else { score });
                        })
                    },
                })
            })
            .collect();
        let node = cx.container(
            LayoutStyle {
                align: Align::Center,
                align_self: Some(Align::Fit),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &stars,
            Paint::None,
        );
        let labels = cx.labels();
        let name = self.label.clone().unwrap_or_else(|| labels.rating.clone());
        cx.semantics(
            node,
            Semantics::new(Role::Slider)
                .name(name)
                .numeric(f64::from(value), 0.0, f64::from(max), Some(1.0))
                .value(labels.rating_value(value, max)),
        );
        let keys = binding.clone();
        cx.on_key(
            node,
            Rc::new(move |key, _| {
                let current = keys.get().min(max);
                let next = match key {
                    Key::ArrowRight | Key::ArrowUp => (current + 1).min(max),
                    Key::ArrowLeft | Key::ArrowDown => current.saturating_sub(1),
                    Key::Home => 0,
                    Key::End => max,
                    Key::Character(c) => match c.parse::<u32>() {
                        Ok(digit) if digit <= max => digit,
                        _ => return false,
                    },
                    _ => return false,
                };
                keys.set(next);
                true
            }),
        );
        cx.focusable(node, theme.radii.sm);
        node
    }
}

impl View for Rating {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        match &self.value {
            Some(binding) => self.editable_view(cx, binding),
            None => self.read_only_view(cx),
        }
    }
}

/// One star that can be clicked. The rating as a whole is what screen readers and the
/// keyboard see, as a slider.
struct Star {
    size: f32,
    color: Color,
    preview: Rc<dyn Fn()>,
    pick: Rc<dyn Fn()>,
}

impl View for Star {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (color, size) = (self.color, self.size);
        let node = cx.container(
            LayoutStyle { width: Some(size), height: Some(size), ..LayoutStyle::leaf() },
            &[],
            Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
                scene.push_path(PathFill::solid(util::star_path(frame.center(), size * 0.48), color));
            })),
        );
        cx.semantics(node, Semantics::hidden());
        let preview = self.preview.clone();
        cx.on_pointer_move(node, Rc::new(move |_, _| preview()));
        cx.on_click(node, self.pick.clone());
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// An icon in a square tinted with its color, beside a title or at the top of a notice.
///
/// ```
/// use ravenrs_widgets::{IconName, IconTile, Tone};
///
/// let tile = IconTile::new(IconName::TriangleAlert).tone(Tone::Warning);
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct IconTile {
    icon: IconName,
    tone: Tone,
    color: Option<Color>,
    size: f32,
    label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl IconTile {
    /// A neutral tile of 48 logical pixels showing `icon`.
    pub fn new(icon: IconName) -> Self {
        Self { icon, tone: Tone::Neutral, color: None, size: 48.0, label: None }
    }

    /// Sets the color tone.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Uses this color instead of the tone's.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets the side of the square in logical pixels. The icon follows it.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// What the icon means, for screen readers. Without it, the tile is decoration.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl View for IconTile {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let color = self.color.unwrap_or_else(|| self.tone.color(&theme));
        let icon = util::icon(cx, self.icon, (self.size * 0.58).round(), color);
        cx.semantics(icon, Semantics::hidden());
        let node = cx.container(
            LayoutStyle {
                width: Some(self.size),
                height: Some(self.size),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[icon],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                ..QuadPaint::new(color.with_alpha(0.14))
            }),
        );
        cx.semantics(
            node,
            match &self.label {
                Some(label) => Semantics::new(Role::Image).name(label.clone()),
                None => Semantics::hidden(),
            },
        );
        node
    }
}

/// What an area shows when it has nothing: an icon, a title, a sentence and what to do.
///
/// ```
/// use ravenrs_widgets::{Button, EmptyState, IconName};
///
/// let empty = EmptyState::new("No photos yet")
///     .icon(IconName::Image)
///     .description("Import a folder to start.")
///     .action(Button::new("Import"));
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EmptyState {
    title: String,
    description: Option<String>,
    icon: Option<IconName>,
    actions: Vec<AnyView>,
}

impl fmt::Debug for EmptyState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmptyState").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl EmptyState {
    /// An empty state with this title.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), description: None, icon: None, actions: Vec::new() }
    }

    /// The sentence under the title, saying why it is empty or what would fill it.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// The icon above the title.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Adds a view under the text, usually a button. Call again for more, side by side.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.actions.push(Box::new(action));
        self
    }
}

impl View for EmptyState {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            children.push(cx.child(&IconTile::new(icon)));
        }
        let title = util::label(cx, &self.title, TextStyle::Headline, None, theme.color(Semantic::Ink));
        cx.semantics(title, Semantics::new(Role::Heading).level(3));
        children.push(title);
        if let Some(text) = &self.description {
            let attrs = util::attrs(&theme, TextStyle::Body, None);
            // A long sentence reads better in a short line than across a wide area.
            let style = LayoutStyle { max_width: Some(420.0), ..LayoutStyle::leaf() };
            children.push(cx.text(text, &attrs, theme.color(Semantic::InkMuted), style));
        }
        if !self.actions.is_empty() {
            let actions: Vec<NodeId> = self.actions.iter().map(|a| cx.child(a.as_ref())).collect();
            children.push(cx.container(
                LayoutStyle {
                    spacing: theme.spacing.s2,
                    padding: Insets { top: theme.spacing.s2, ..Insets::ZERO },
                    ..LayoutStyle::stack(Axis::Horizontal)
                },
                &actions,
                Paint::None,
            ));
        }
        cx.container(
            LayoutStyle {
                padding: Insets::uniform(theme.spacing.s8),
                spacing: theme.spacing.s2,
                align: Align::Center,
                justify: Justify::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        )
    }
}

/// A wide band in a strong color, readable from across the room: "You can remove the card".
/// In the accent tone its text is the theme's `on-accent`; on any other tone or color it is black
/// or white, whichever has more contrast.
///
/// ```
/// use ravenrs_widgets::{HeroBanner, IconName, Tone};
///
/// let banner = HeroBanner::new("You can remove the card").icon(IconName::CircleCheck).tone(Tone::Success);
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct HeroBanner {
    title: String,
    description: Option<String>,
    icon: Option<IconName>,
    tone: Tone,
    color: Option<Color>,
    actions: Vec<AnyView>,
}

impl fmt::Debug for HeroBanner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeroBanner").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl HeroBanner {
    /// A banner saying `title`, in the accent tone.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            icon: None,
            tone: Tone::Accent,
            color: None,
            actions: Vec::new(),
        }
    }

    /// A line under the title.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// The icon before the title.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Sets the tone. Warning and danger are announced as alerts.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Uses this color instead of the tone's.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Adds a view after the text, usually a button.
    pub fn action(mut self, action: impl View + 'static) -> Self {
        self.actions.push(Box::new(action));
        self
    }
}

/// Black or white, whichever has more contrast on `fill`: one of them always reaches 4.5:1.
fn ink_on(fill: Color) -> Color {
    let fill = fill.with_alpha(1.0);
    if fill.contrast_ratio(Color::BLACK) >= fill.contrast_ratio(Color::WHITE) {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

impl View for HeroBanner {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let fill = self.color.unwrap_or_else(|| self.tone.color(&theme));
        // On the theme's own accent, the theme says what reads on it.
        let ink = match (self.color, self.tone) {
            (None, Tone::Accent) => theme.palette.on_accent,
            _ => ink_on(fill),
        };
        let mut text = vec![util::label(cx, &self.title, TextStyle::Title, Some(Weight::BOLD), ink)];
        if let Some(description) = &self.description {
            text.push(util::label(cx, description, TextStyle::Body, None, ink));
        }
        let text = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s1,
                min_width: Some(0.0),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &text,
            Paint::None,
        );
        let mut children = Vec::new();
        if let Some(icon) = self.icon {
            let icon = util::icon(cx, icon, 32.0, ink);
            cx.semantics(icon, Semantics::hidden());
            children.push(icon);
        }
        children.push(text);
        for action in &self.actions {
            children.push(cx.child(action.as_ref()));
        }
        let node = cx.container(
            LayoutStyle {
                padding: Insets::symmetric(theme.spacing.s4 + theme.spacing.s1 / 2.0, theme.spacing.s4),
                spacing: theme.spacing.s3,
                align: Align::Center,
                justify: Justify::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &children,
            Paint::Quad(QuadPaint { corner_radius: theme.radii.lg, ..QuadPaint::new(fill) }),
        );
        let role = if matches!(self.tone, Tone::Warning | Tone::Danger) { Role::Alert } else { Role::Status };
        cx.semantics(node, Semantics::new(role).name(self.title.clone()));
        node
    }
}

/// Several progress bars stacked, for work that has more than one part.
///
/// ```
/// use ravenrs_widgets::{MultiProgress, Tone};
///
/// let copying = MultiProgress::new()
///     .meter("Local copy", 40, 120)
///     .meter("Previews", 12, 40);
/// ```
#[derive(Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MultiProgress {
    bars: Vec<ProgressBar>,
    label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl MultiProgress {
    /// No bars yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a bar for `done` of `total`, with both numbers below it. Nothing to do counts as
    /// nothing done, and more than the total counts as all of it.
    pub fn meter(self, label: impl Into<String>, done: u64, total: u64) -> Self {
        let fraction = if total == 0 { 0.0 } else { (done as f64 / total as f64) as f32 };
        self.bar(ProgressBar::new(fraction).label(label).percent().detail(format!("{done} / {total}")))
    }

    /// Adds a bar set up by hand, with its own color or height.
    pub fn bar(mut self, bar: ProgressBar) -> Self {
        self.bars.push(bar);
        self
    }

    /// What the bars are, as a whole, for screen readers.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl View for MultiProgress {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let spacing = cx.theme().spacing.s3;
        let bars: Vec<NodeId> = self.bars.iter().map(|bar| cx.child(bar)).collect();
        let node = cx.container(
            LayoutStyle {
                spacing,
                align: Align::Stretch,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &bars,
            Paint::None,
        );
        if let Some(label) = &self.label {
            cx.semantics(node, Semantics::new(Role::Group).name(label.clone()));
        }
        node
    }
}

/// One line of text with a light passing over it, to say that something is under way.
///
/// When motion is reduced, or while [`active`](Self::active) is off, it is plain muted text
/// that asks for no frames.
///
/// ```
/// use ravenrs_widgets::ShimmerText;
///
/// let waiting = ShimmerText::new("Looking for updates");
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ShimmerText {
    text: String,
    style: TextStyle,
    active: bool,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ShimmerText {
    /// `text` with the light passing over it.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), style: TextStyle::Callout, active: true }
    }

    /// Sets the text style.
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Turns the light on or off, as the work starts and ends, without moving the text.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// The shaped text, measured from its own corner, which a paint-only frame cannot shape again.
#[derive(Default)]
struct ShimmerGlyphs {
    width: f32,
    runs: Vec<GlyphRun>,
}

fn shape_for_shimmer(
    text: &str,
    attrs: &TextAttrs,
    frame: Rect,
    cx: &mut crate::PaintCx<'_>,
) -> ShimmerGlyphs {
    let mut layout = cx.text.layout(text, attrs);
    layout.measure(Some(frame.width() + 0.5));
    let mut scene = Scene::new();
    layout.paint(Point::ZERO, Color::WHITE, &mut scene);
    let runs = scene
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Glyphs(run) = p { Some(run.clone()) } else { None })
        .collect();
    ShimmerGlyphs { width: frame.width(), runs }
}

fn paint_shimmer(glyphs: &ShimmerGlyphs, origin: Point, time: f64, light: Color, scene: &mut Scene) {
    let progress = (time / SHIMMER_PERIOD).fract() as f32;
    let center = -SHIMMER_REACH + (glyphs.width + 2.0 * SHIMMER_REACH) * progress;
    for run in &glyphs.runs {
        for glyph in &run.glyphs {
            let strength = 1.0 - (glyph.x - center).abs() / SHIMMER_REACH;
            if strength < SHIMMER_FLOOR {
                continue;
            }
            let mut lit = *glyph;
            lit.x += origin.x;
            lit.y += origin.y;
            scene.push_glyphs(GlyphRun {
                font: run.font.clone(),
                size: run.size,
                color: light.with_alpha(light.a * strength),
                glyphs: vec![lit],
            });
        }
    }
}

impl View for ShimmerText {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let attrs = util::attrs(&theme, self.style, None);
        let (base, light) = (theme.color(Semantic::InkMuted), theme.color(Semantic::Ink));
        let text = cx.text(&self.text, &attrs, base, LayoutStyle::leaf());
        if !self.active || cx.reduced_motion() {
            return text;
        }
        let glyphs = Rc::new(RefCell::new(ShimmerGlyphs::default()));
        let drawn = glyphs.clone();
        // Live drawing has the clock but cannot shape text, so a node with rich drawing shapes
        // it first. Only the glyphs under the light are drawn again, on top of the plain text.
        let light_node = cx.container(
            LayoutStyle { absolute: Some((0.0, 0.0)), ..LayoutStyle::leaf() },
            &[],
            Paint::Live(Rc::new(move |frame: Rect, time, scene: &mut Scene| {
                paint_shimmer(&drawn.borrow(), frame.origin, time, light, scene);
            })),
        );
        cx.semantics(light_node, Semantics::hidden());
        let source = self.text.clone();
        cx.container(
            LayoutStyle { align_self: Some(Align::Fit), ..LayoutStyle::stack(Axis::Horizontal) },
            &[text, light_node],
            Paint::Rich(Rc::new(move |frame: Rect, cx: &mut crate::PaintCx<'_>| {
                if glyphs.borrow().width != frame.width() {
                    *glyphs.borrow_mut() = shape_for_shimmer(&source, &attrs, frame, cx);
                }
            })),
        )
    }
}

/// A line with a word in the middle that divides a list by day: "Today", "Yesterday".
///
/// ```
/// use ravenrs_widgets::{Date, DaySeparator};
///
/// let today = Date::new(2026, 11, 4).unwrap_or(Date::from_days(0));
/// let heading = DaySeparator::of(today.add_days(-1), today);
/// let free = DaySeparator::new("Last week");
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct DaySeparator {
    text: DayText,
}

#[derive(Clone, Debug)]
enum DayText {
    Free(String),
    Day { day: Date, today: Date },
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl DaySeparator {
    /// A separator with this text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: DayText::Free(text.into()) }
    }

    /// A separator for `day`: the words for today and yesterday from
    /// [`Labels`](crate::Labels), and otherwise the short date.
    pub fn of(day: Date, today: Date) -> Self {
        Self { text: DayText::Day { day, today } }
    }
}

impl View for DaySeparator {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let text = match &self.text {
            DayText::Free(text) => text.clone(),
            DayText::Day { day, today } => {
                let labels = cx.labels();
                match today.to_days() - day.to_days() {
                    0 => labels.today.clone(),
                    1 => labels.yesterday.clone(),
                    _ => labels.short_date(*day),
                }
            }
        };
        let border = theme.color(Semantic::Border);
        let line = |cx: &mut BuildCx<'_>| {
            cx.container(
                LayoutStyle { height: Some(1.0), grow: 1.0, ..LayoutStyle::leaf() },
                &[],
                Paint::Quad(QuadPaint::new(border)),
            )
        };
        let before = line(cx);
        let word =
            util::label(cx, &text, TextStyle::Caption, Some(Weight::MEDIUM), theme.color(Semantic::InkMuted));
        cx.semantics(word, Semantics::hidden());
        let after = line(cx);
        let node = cx.container(
            LayoutStyle {
                spacing: theme.spacing.s3,
                align: Align::Center,
                align_self: Some(Align::Stretch),
                ..LayoutStyle::stack(Axis::Horizontal)
            },
            &[before, word, after],
            Paint::None,
        );
        cx.semantics(node, Semantics::new(Role::Separator).name(text));
        node
    }
}
