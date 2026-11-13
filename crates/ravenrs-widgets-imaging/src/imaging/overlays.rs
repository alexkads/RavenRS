//! What is drawn over a picture: composition guides, the rectangle of a drag, the outline of a
//! selection, the eyedropper. They share [`ContentSpace`], which says where the content is.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::{
    Color, CursorIcon, Key, Modifiers, PathElement, PathFill, Point, Quad, Rect, Role, Scene, Size,
};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, Observer, State};
use ravenrs_style::{Semantic, Theme};

use super::{ZoomView, stroke_outline};
use crate::context::{BuildCx, DragPhase, NodeId, Paint, Semantics};
use crate::{AnyView, ButtonSize, IconButton, IconName, Tooltip, View};

// ---------------------------------------------------------------------------------------------
// ContentSpace
// ---------------------------------------------------------------------------------------------

/// How the coordinates of a widget drawn over content relate to the screen, so that a selection
/// or a crop is told in the units of the content whatever the zoom.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum ContentSpace {
    /// The widget's own box is the content, one unit per logical pixel from its top left corner.
    #[default]
    Frame,
    /// Content of this size, shown whole and centered in the box, as an image that fits.
    Fit(Size),
    /// Content of this size where a [`ZoomPanView`](super::ZoomPanView) with the same
    /// [`ZoomView`] shows it. Put the widget exactly over that view.
    View(Binding<ZoomView>, Size),
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ContentSpace {
    /// Where the whole content lands on screen, for a widget at `frame`.
    pub fn placed(&self, frame: Rect) -> Rect {
        match self {
            Self::Frame => frame,
            Self::Fit(content) => fitted(frame, *content),
            Self::View(view, content) => {
                let mut v = view.get();
                v.viewport = frame.size;
                if v.fit {
                    v.scale = v.fit_scale(*content);
                    v.center = Point::new(0.5, 0.5);
                }
                let placed = v.placed(*content);
                placed.translate(frame.origin)
            }
        }
    }

    /// The size of the content in its own units.
    pub fn content_size(&self, frame: Rect) -> Size {
        match self {
            Self::Frame => frame.size,
            Self::Fit(content) | Self::View(_, content) => *content,
        }
    }

    /// Logical pixels on screen per unit of content.
    pub fn scale(&self, frame: Rect) -> f32 {
        self.map(frame).scale
    }

    /// A point on screen, in the units of the content.
    pub fn to_content(&self, frame: Rect, at: Point) -> Point {
        self.map(frame).content(at)
    }

    /// A point of the content, on screen.
    pub fn to_screen(&self, frame: Rect, at: Point) -> Point {
        self.map(frame).screen(at)
    }

    /// A rectangle of the content, on screen.
    pub fn rect_to_screen(&self, frame: Rect, rect: Rect) -> Rect {
        self.map(frame).screen_rect(rect)
    }

    pub(super) fn map(&self, frame: Rect) -> ContentMap {
        let placed = self.placed(frame);
        let content = self.content_size(frame);
        ContentMap { placed, scale: placed.width() / content.width.max(f32::EPSILON), content }
    }

    /// Makes the build that calls it follow the view.
    pub(super) fn watch(&self) {
        if let Self::View(view, _) = self {
            view.get();
        }
    }
}

/// The largest rectangle of the proportions of `content` in `frame`, centered.
pub(super) fn fitted(frame: Rect, content: Size) -> Rect {
    let scale = (frame.width() / content.width.max(f32::EPSILON))
        .min(frame.height() / content.height.max(f32::EPSILON))
        .max(0.0);
    let (w, h) = (content.width * scale, content.height * scale);
    Rect::new(frame.x() + (frame.width() - w) / 2.0, frame.y() + (frame.height() - h) / 2.0, w, h)
}

/// One frame's worth of [`ContentSpace`].
#[derive(Clone, Copy, Debug)]
pub(super) struct ContentMap {
    pub placed: Rect,
    pub scale: f32,
    pub content: Size,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ContentMap {
    pub fn screen(&self, at: Point) -> Point {
        Point::new(self.placed.x() + at.x * self.scale, self.placed.y() + at.y * self.scale)
    }

    pub fn content(&self, at: Point) -> Point {
        let scale = self.scale.max(f32::EPSILON);
        Point::new((at.x - self.placed.x()) / scale, (at.y - self.placed.y()) / scale)
    }

    pub fn screen_rect(&self, rect: Rect) -> Rect {
        let at = self.screen(rect.origin);
        Rect::new(at.x, at.y, rect.width() * self.scale, rect.height() * self.scale)
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, self.content.width, self.content.height)
    }
}

/// A rectangle from two opposite corners.
fn spanned(a: Point, b: Point) -> Rect {
    Rect::new(a.x.min(b.x), a.y.min(b.y), (b.x - a.x).abs(), (b.y - a.y).abs())
}

/// Two inks that read over any picture together, the light one and the dark one. They come from
/// the theme but do not swap with it: a picture stays the same in a light theme.
pub(super) fn picture_inks(theme: &Theme) -> (Color, Color) {
    let brightness = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
    let (a, b) = (theme.color(Semantic::Canvas), theme.color(Semantic::Ink));
    if brightness(a) > brightness(b) { (a, b) } else { (b, a) }
}

// ---------------------------------------------------------------------------------------------
// The box of an overlay
// ---------------------------------------------------------------------------------------------

/// The box of a widget drawn over something: the size it was given, or that of the view it is
/// laid over.
#[derive(Default)]
pub(super) struct OverlayBox {
    width: Option<f32>,
    height: Option<f32>,
    grow: Option<f32>,
    stage: Option<AnyView>,
    pub space: ContentSpace,
}

impl fmt::Debug for OverlayBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OverlayBox").field("space", &self.space).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl OverlayBox {
    pub fn size(&mut self, width: f32, height: f32) {
        self.width = Some(width);
        self.height = Some(height);
        self.grow = Some(0.0);
    }

    pub fn grow(&mut self, grow: f32) {
        self.grow = Some(grow);
    }

    pub fn stage(&mut self, stage: AnyView) {
        self.stage = Some(stage);
    }

    /// The size of the box as far as a build can know it, which the first build may not.
    pub fn known_size(&self, cx: &BuildCx<'_>) -> Option<Size> {
        match (self.width, self.height) {
            (Some(width), Some(height)) => Some(Size::new(width, height)),
            _ => cx.scroll_viewport(),
        }
    }

    /// Builds the box, and returns it with the node that paints and takes the pointer.
    pub fn build(&self, cx: &mut BuildCx<'_>, paint: Paint) -> (NodeId, NodeId) {
        let sized = self.width.is_some();
        let Some(stage) = &self.stage else {
            let grow = self.grow.unwrap_or(1.0);
            let root = cx.container(
                LayoutStyle {
                    width: self.width,
                    height: self.height,
                    grow,
                    align_self: Some(if sized { Align::Start } else { Align::Stretch }),
                    ..LayoutStyle::leaf()
                },
                &[],
                paint,
            );
            cx.report_frame(root);
            return (root, root);
        };
        let stage = cx.child(stage.as_ref());
        // Laid over: the runtime has no box that takes the size of a sibling, so it is read back
        // from the last layout. Nothing else moves when the first build has none yet.
        let measured = cx.scroll_viewport().unwrap_or(Size::ZERO);
        let surface = cx.container(
            LayoutStyle {
                absolute: Some((0.0, 0.0)),
                width: Some(measured.width),
                height: Some(measured.height),
                ..LayoutStyle::leaf()
            },
            &[],
            paint,
        );
        let grow = self.grow.unwrap_or(0.0);
        let root = cx.container(
            LayoutStyle {
                width: self.width,
                height: self.height,
                grow,
                align: Align::Stretch,
                align_self: Some(if grow > 0.0 { Align::Stretch } else { Align::Start }),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[stage, surface],
            Paint::None,
        );
        cx.report_frame(root);
        (root, surface)
    }
}

/// The builders every overlay has, over the field that holds its box.
macro_rules! overlay_builders {
    ($ty:ident, $($field:ident).+) => {
        #[cfg_attr(feature = "live", ravenrs_live::expose(part = "builders"))]
        impl $ty {
            /// Fixed size in logical pixels, instead of taking the free space.
            pub fn size(mut self, width: f32, height: f32) -> Self {
                self.$($field).+.size(width, height);
                self
            }

            /// How much of the free space along the parent stack it takes: 1 by default, or 0 over a
            /// [stage](Self::stage), which then decides the size.
            pub fn grow(mut self, grow: f32) -> Self {
                self.$($field).+.grow(grow);
                self
            }

            /// The view it goes over, such as an [`Image`](crate::Image) or a
            /// [`ZoomPanView`](crate::ZoomPanView), which gives it its size. Without one, put it in
            /// a [`ZStack`](crate::ZStack) with a fixed [size](Self::size).
            pub fn stage(mut self, stage: impl View + 'static) -> Self {
                self.$($field).+.stage(Box::new(stage));
                self
            }

            /// Where the content is, and so the units of its coordinates: by default, its own box.
            pub fn space(mut self, space: ContentSpace) -> Self {
                self.$($field).+.space = space;
                self
            }
        }
    };
}
pub(super) use overlay_builders;

/// A painter that keeps asking for frames during a gesture, which then costs no build, and that
/// costs nothing afterwards.
pub(super) fn gesture_paint(live: bool, draw: impl Fn(Rect, &mut Scene) + 'static) -> Paint {
    if live {
        Paint::Live(Rc::new(move |frame, _, scene| draw(frame, scene)))
    } else {
        Paint::Custom(Rc::new(draw))
    }
}

// ---------------------------------------------------------------------------------------------
// RuleOfThirdsGrid
// ---------------------------------------------------------------------------------------------

/// The lines a [`RuleOfThirdsGrid`] draws.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum CompositionGuide {
    /// Two lines each way, at a third and two thirds.
    #[default]
    Thirds,
    /// This many equal parts each way, such as a fine grid to level a horizon.
    Divisions(u32),
    /// Square cells, this many along the shorter side.
    Squares(u32),
    /// Two lines each way, at the golden ratio.
    GoldenRatio,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl CompositionGuide {
    /// Where the lines cross each axis, as shares of a box of this size.
    fn lines(self, width: f32, height: f32) -> (Vec<f32>, Vec<f32>) {
        let equal = |parts: u32| -> Vec<f32> { (1..parts).map(|i| i as f32 / parts as f32).collect() };
        match self {
            Self::Thirds => (equal(3), equal(3)),
            Self::Divisions(parts) => (equal(parts), equal(parts)),
            Self::GoldenRatio => {
                let golden = 1.0 / 1.618_034;
                (vec![1.0 - golden, golden], vec![1.0 - golden, golden])
            }
            Self::Squares(cells) => {
                let side = width.min(height) / cells.max(1) as f32;
                let along = |length: f32| -> Vec<f32> {
                    if side <= 0.0 {
                        return Vec::new();
                    }
                    (1..)
                        .map(|i| i as f32 * side)
                        .take_while(|at| *at < length - 0.5)
                        .map(|at| at / length)
                        .collect()
                };
                (along(width), along(height))
            }
        }
    }
}

/// Draws a guide inside a box given by its corners on screen, clockwise from the top left.
pub(super) fn paint_guide(
    scene: &mut Scene,
    corners: [Point; 4],
    guide: CompositionGuide,
    light: Color,
    dark: Color,
    opacity: f32,
) {
    let [tl, tr, br, bl] = corners;
    let (width, height) = ((tr.x - tl.x).hypot(tr.y - tl.y), (bl.x - tl.x).hypot(bl.y - tl.y));
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let (across, down) = guide.lines(width, height);
    let mix = |a: Point, b: Point, t: f32| Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    let (light, dark) = (light.with_alpha(light.a * opacity), dark.with_alpha(dark.a * opacity * 0.5));
    let lines = across
        .iter()
        .map(|t| (mix(tl, tr, *t), mix(bl, br, *t)))
        .chain(down.iter().map(|t| (mix(tl, bl, *t), mix(tr, br, *t))));
    if (tl.y - tr.y).abs() < 0.01 {
        // Upright, a line lands on whole pixels and stays sharp. The dark line beside it is what
        // shows over a bright sky.
        for (a, b) in lines {
            let (x, y) = (a.x.round(), a.y.round());
            let (w, h) = ((b.x - a.x).abs().max(1.0), (b.y - a.y).abs().max(1.0));
            let (dx, dy) = if w <= 1.0 { (1.0, 0.0) } else { (0.0, 1.0) };
            scene.push_quad(Quad::new(Rect::new(x + dx, y + dy, w, h), dark));
            scene.push_quad(Quad::new(Rect::new(x, y, w, h), light));
        }
    } else {
        let mut outline = Vec::new();
        for (a, b) in lines {
            outline.extend(stroke_outline(&[a, b], 1.0));
        }
        if !outline.is_empty() {
            scene.push_path(PathFill::solid(outline, light));
        }
    }
}

/// The corners of an upright rectangle, clockwise from the top left.
pub(super) fn corners_of(rect: Rect) -> [Point; 4] {
    [
        rect.origin,
        Point::new(rect.max_x(), rect.y()),
        Point::new(rect.max_x(), rect.max_y()),
        Point::new(rect.x(), rect.max_y()),
    ]
}

/// A composition grid over a picture: thirds by default, or the other [`CompositionGuide`]s. It
/// takes no input, so what is underneath still gets the pointer.
///
/// ```
/// use ravenrs_core::Size;
/// use ravenrs_widgets::{CompositionGuide, ContentSpace, RuleOfThirdsGrid};
///
/// let grid = RuleOfThirdsGrid::new()
///     .guide(CompositionGuide::GoldenRatio)
///     .space(ContentSpace::Fit(Size::new(6000.0, 4000.0)))
///     .size(300.0, 200.0);
/// ```
#[derive(Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RuleOfThirdsGrid {
    frame: OverlayBox,
    guide: CompositionGuide,
    area: Option<Rect>,
    color: Option<Color>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RuleOfThirdsGrid {
    /// Thirds over the whole content.
    pub fn new() -> Self {
        Self::default()
    }

    /// Which lines it draws.
    pub fn guide(mut self, guide: CompositionGuide) -> Self {
        self.guide = guide;
        self
    }

    /// Draws only inside this rectangle of the content, such as a crop.
    pub fn area(mut self, area: Rect) -> Self {
        self.area = Some(area);
        self
    }

    /// The color of the lines, instead of a light line with a dark one beside it.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

overlay_builders!(RuleOfThirdsGrid, frame);

impl View for RuleOfThirdsGrid {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (mut light, mut dark) = picture_inks(cx.theme());
        if let Some(color) = self.color {
            light = color;
            dark = Color::TRANSPARENT;
        }
        self.frame.space.watch();
        let (space, area, guide) = (self.frame.space.clone(), self.area, self.guide);
        let paint = Paint::Custom(Rc::new(move |frame: Rect, scene: &mut Scene| {
            let map = space.map(frame);
            let shown = area.map_or(map.placed, |area| map.screen_rect(area));
            scene.push_clip(frame);
            paint_guide(scene, corners_of(shown), guide, light, dark, 0.45);
            scene.pop_clip();
        }));
        let (root, surface) = self.frame.build(cx, paint);
        // Only lines to look by: there is nothing in them to tell a screen reader.
        cx.semantics(surface, Semantics::hidden());
        root
    }
}

// ---------------------------------------------------------------------------------------------
// RubberBand
// ---------------------------------------------------------------------------------------------

/// Below this many pixels, a press and release is a click and not a selection.
const BAND_MIN_DRAG: f32 = 4.0;

#[derive(Debug, Default)]
struct BandMemory {
    frame: Cell<Rect>,
    /// Where the drag began, in units of the content.
    pressed: Cell<Option<Point>>,
}

/// A rectangle drawn with a drag, in the units of the content. The selection follows the drag
/// through its binding, and [`on_select`](Self::on_select) gets it on release; a press and
/// release without moving clears it. Escape cancels a drag.
///
/// Keyboard: the arrows move it, with Alt they resize it (and with Shift, further), Enter
/// selects it, Delete or Escape clears it. With nothing selected, an arrow starts from the
/// middle quarter of the content.
///
/// While dragging it paints without a build, so it costs no rebuilds if nothing else reads the
/// selection.
///
/// ```
/// use ravenrs_core::Rect;
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::RubberBand;
///
/// let selection = State::new(None::<Rect>);
/// let band = RubberBand::new(selection.binding())
///     .keep(false)
///     .on_select(|rect| println!("zoom to {rect:?}"))
///     .size(320.0, 200.0);
/// ```
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RubberBand {
    selection: Binding<Option<Rect>>,
    frame: OverlayBox,
    on_select: Option<Rc<dyn Fn(Rect)>>,
    keep: bool,
    disabled: bool,
    label: Option<String>,
}

impl fmt::Debug for RubberBand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RubberBand").field("frame", &self.frame).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RubberBand {
    /// A band bound to `selection`, in the units of its [space](Self::space).
    pub fn new(selection: Binding<Option<Rect>>) -> Self {
        Self {
            selection,
            frame: OverlayBox::default(),
            on_select: None,
            keep: true,
            disabled: false,
            label: None,
        }
    }

    /// Runs `handler` with the rectangle when a drag ends, or on Enter.
    pub fn on_select(mut self, handler: impl Fn(Rect) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Whether the rectangle stays after the release (it does by default). A box for zooming,
    /// say, goes away as soon as it is used.
    pub fn keep(mut self, keep: bool) -> Self {
        self.keep = keep;
        self
    }

    /// Takes no input while disabled, which leaves the pointer to what is underneath: for a
    /// band that only works in one mode or with a key held.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

overlay_builders!(RubberBand, frame);

/// `rect` moved to be inside `bounds`, and no larger than them.
fn kept_inside(rect: Rect, bounds: Rect) -> Rect {
    let (w, h) = (rect.width().min(bounds.width()), rect.height().min(bounds.height()));
    Rect::new(
        rect.x().clamp(bounds.x(), (bounds.max_x() - w).max(bounds.x())),
        rect.y().clamp(bounds.y(), (bounds.max_y() - h).max(bounds.y())),
        w,
        h,
    )
}

fn clamped(at: Point, bounds: Rect) -> Point {
    Point::new(at.x.clamp(bounds.x(), bounds.max_x()), at.y.clamp(bounds.y(), bounds.max_y()))
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl RubberBand {
    fn on_key(
        selection: &Binding<Option<Rect>>,
        memory: &BandMemory,
        space: &ContentSpace,
        on_select: Option<&Rc<dyn Fn(Rect)>>,
        key: &Key,
        modifiers: Modifiers,
    ) -> bool {
        let current = selection.get();
        match key {
            Key::Escape if memory.pressed.get().is_some() || current.is_some() => {
                memory.pressed.set(None);
                selection.set(None);
                return true;
            }
            Key::Delete | Key::Backspace if current.is_some() => {
                selection.set(None);
                return true;
            }
            Key::Enter => {
                let (Some(rect), Some(handler)) = (current, on_select) else { return false };
                handler(rect);
                return true;
            }
            _ => {}
        }
        let (dx, dy) = match key {
            Key::ArrowLeft => (-1.0, 0.0),
            Key::ArrowRight => (1.0, 0.0),
            Key::ArrowUp => (0.0, -1.0),
            Key::ArrowDown => (0.0, 1.0),
            _ => return false,
        };
        let map = space.map(memory.frame.get());
        let bounds = map.bounds();
        let Some(rect) = current else {
            let (w, h) = (bounds.width() / 2.0, bounds.height() / 2.0);
            selection.set(Some(Rect::new(w / 2.0, h / 2.0, w, h)));
            return true;
        };
        let step = (if modifiers.shift { 10.0 } else { 1.0 }) / map.scale.max(f32::EPSILON);
        let next = if modifiers.alt {
            let least = BAND_MIN_DRAG / map.scale.max(f32::EPSILON);
            Rect::new(
                rect.x(),
                rect.y(),
                (rect.width() + dx * step).max(least),
                (rect.height() + dy * step).max(least),
            )
        } else {
            Rect::new(rect.x() + dx * step, rect.y() + dy * step, rect.width(), rect.height())
        };
        selection.set(Some(kept_inside(next, bounds)));
        true
    }
}

impl View for RubberBand {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let labels = cx.labels();
        let live = cx.use_state(|| false);
        let is_live = live.get();
        // During a drag it paints from the binding on its own: reading it here too would make
        // every step of the drag a rebuild.
        let current = if is_live {
            Observer::untracked(|| self.selection.get())
        } else {
            self.frame.space.watch();
            self.selection.get()
        };
        let memory = cx.use_state(|| Rc::new(BandMemory::default())).get();
        let (light, dark) = picture_inks(&theme);
        let (space, selection, remembered) =
            (self.frame.space.clone(), self.selection.clone(), memory.clone());
        let paint = gesture_paint(is_live, move |frame, scene| {
            remembered.frame.set(frame);
            let Some(rect) = selection.get() else { return };
            let shown = space.rect_to_screen(frame, rect);
            scene.push_clip(frame);
            scene.push_quad(Quad::new(shown, light.with_alpha(0.12)).border(1.0, light));
            scene.push_quad(
                Quad::new(shown.inflate(1.0), Color::TRANSPARENT).border(1.0, dark.with_alpha(0.55)),
            );
            scene.pop_clip();
        });
        let (root, surface) = self.frame.build(cx, paint);
        let name = self.label.clone().unwrap_or_else(|| labels.rubber_band.clone());
        let mut semantics = Semantics::new(Role::Group).name(name).disabled(self.disabled);
        if let Some(rect) = current {
            semantics = semantics.value(format!(
                "{:.0}, {:.0}, {:.0} × {:.0}",
                rect.x(),
                rect.y(),
                rect.width(),
                rect.height()
            ));
        }
        cx.semantics(surface, semantics);
        if self.disabled {
            return root;
        }
        cx.focusable(surface, 0.0);
        cx.cursor(surface, CursorIcon::Default);
        let (selection, space, on_select, keep, grabbed) = (
            self.selection.clone(),
            self.frame.space.clone(),
            self.on_select.clone(),
            self.keep,
            memory.clone(),
        );
        cx.on_drag(
            surface,
            Rc::new(move |d| {
                let map = space.map(d.frame);
                let at = clamped(map.content(d.position), map.bounds());
                match d.phase {
                    DragPhase::Start => {
                        grabbed.pressed.set(Some(at));
                        live.set_if_changed(true);
                    }
                    DragPhase::Move => {
                        let Some(pressed) = grabbed.pressed.get() else { return };
                        let rect = spanned(pressed, at);
                        let far = BAND_MIN_DRAG / map.scale.max(f32::EPSILON);
                        if rect.width().max(rect.height()) >= far || selection.get().is_some() {
                            selection.set(Some(rect));
                        }
                    }
                    DragPhase::End => {
                        live.set_if_changed(false);
                        let Some(pressed) = grabbed.pressed.take() else { return };
                        let rect = spanned(pressed, at);
                        let far = BAND_MIN_DRAG / map.scale.max(f32::EPSILON);
                        if rect.width().max(rect.height()) < far {
                            if selection.get().is_some() {
                                selection.set(None);
                            }
                            return;
                        }
                        if let Some(handler) = &on_select {
                            handler(rect);
                        }
                        if !keep {
                            selection.set(None);
                        }
                    }
                    // The base may add more; they act as nothing here.
                    _ => {}
                }
            }),
        );
        let (selection, space, on_select) =
            (self.selection.clone(), self.frame.space.clone(), self.on_select.clone());
        cx.on_key(
            surface,
            Rc::new(move |key, modifiers| {
                Self::on_key(&selection, &memory, &space, on_select.as_ref(), key, modifiers)
            }),
        );
        root
    }
}

// ---------------------------------------------------------------------------------------------
// Marquee
// ---------------------------------------------------------------------------------------------

/// The length of a dash of a marquee and of the gap after it.
const MARQUEE_DASH: f32 = 4.0;
/// How fast the dashes walk, in pixels per second.
const MARQUEE_SPEED: f32 = 16.0;

/// Draws straight lines, each as a sharp quad when it can be and in one path otherwise.
struct Strokes<'a> {
    scene: &'a mut Scene,
    frame: Rect,
    width: f32,
    color: Color,
    path: Vec<PathElement>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Strokes<'_> {
    fn line(&mut self, a: Point, b: Point) {
        let (x, y) = (a.x.min(b.x), a.y.min(b.y));
        let (w, h) = ((b.x - a.x).abs(), (b.y - a.y).abs());
        // A straight line on whole pixels stays sharp: half a pixel each side of one would read
        // as a gray blur, and the dashes would vanish into the dark line under them.
        let bounds = if h < 0.01 {
            Rect::new(x, y.round(), w.max(1.0), 1.0)
        } else if w < 0.01 {
            Rect::new(x.round(), y, 1.0, h.max(1.0))
        } else {
            let half = self.width / 2.0;
            if Rect::new(x, y, w, h).inflate(half).intersect(&self.frame).is_empty() {
                return;
            }
            self.path.extend(stroke_outline(&[a, b], self.width));
            return;
        };
        if !bounds.intersect(&self.frame).is_empty() {
            self.scene.push_quad(Quad::new(bounds, self.color));
        }
    }

    fn finish(self) {
        if !self.path.is_empty() {
            self.scene.push_path(PathFill::solid(self.path, self.color));
        }
    }
}

/// Calls `dash` for each dash along a line of connected points, with the pattern shifted by
/// `phase` pixels and continuing around its corners.
fn march(points: &[Point], phase: f32, mut dash: impl FnMut(Point, Point)) {
    let period = MARQUEE_DASH * 2.0;
    let mut walked = 0.0;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = (b.x - a.x).hypot(b.y - a.y);
        if length <= 0.0 {
            continue;
        }
        let at = |t: f32| Point::new(a.x + (b.x - a.x) * t / length, a.y + (b.y - a.y) * t / length);
        let mut t = 0.0;
        while t < length {
            let place = (walked + t - phase).rem_euclid(period);
            if place < MARQUEE_DASH {
                let end = (t + MARQUEE_DASH - place).min(length);
                dash(at(t), at(end));
                t = end.max(t + 0.001);
            } else {
                t += (period - place).max(0.001);
            }
        }
        walked += length;
    }
}

/// The animated outline of a selection, the "marching ants": a dark line with light dashes
/// walking over it, which reads over any picture. Give it a rectangle, a closed path or loose
/// segments, such as the edges of a mask, in the units of the [space](Self::space).
///
/// The dashes stop with reduced motion, and then it asks for no frames at all. It takes no
/// input, so what is underneath still gets the pointer.
///
/// ```
/// use ravenrs_core::Rect;
/// use ravenrs_widgets::Marquee;
///
/// let marquee = Marquee::rect(Rect::new(20.0, 20.0, 120.0, 80.0)).size(320.0, 200.0);
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Marquee {
    lines: Rc<[Vec<Point>]>,
    frame: OverlayBox,
    animated: bool,
    label: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Marquee {
    /// Around a rectangle.
    pub fn rect(rect: Rect) -> Self {
        let [a, b, c, d] = corners_of(rect);
        Self::from_lines(vec![vec![a, b, c, d, a]])
    }

    /// Along a closed path of these points.
    pub fn path(points: impl IntoIterator<Item = Point>) -> Self {
        let mut points: Vec<Point> = points.into_iter().collect();
        if let (Some(first), true) = (points.first().copied(), points.len() > 2) {
            points.push(first);
        }
        Self::from_lines(vec![points])
    }

    /// Along separate segments, in any order: the outline of a mask, say.
    pub fn segments(segments: impl IntoIterator<Item = (Point, Point)>) -> Self {
        Self::from_lines(segments.into_iter().map(|(a, b)| vec![a, b]).collect())
    }

    fn from_lines(lines: Vec<Vec<Point>>) -> Self {
        Self { lines: lines.into(), frame: OverlayBox::default(), animated: true, label: None }
    }

    /// Whether the dashes walk (they do by default, unless motion is reduced).
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

overlay_builders!(Marquee, frame);

impl View for Marquee {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (light, dark) = picture_inks(cx.theme());
        let name = self.label.clone().unwrap_or_else(|| cx.labels().marquee.clone());
        self.frame.space.watch();
        let (lines, space) = (self.lines.clone(), self.frame.space.clone());
        let draw = move |frame: Rect, phase: f32, scene: &mut Scene| {
            let map = space.map(frame);
            let shown: Vec<Vec<Point>> =
                lines.iter().map(|line| line.iter().map(|p| map.screen(*p)).collect()).collect();
            scene.push_clip(frame);
            let mut base = Strokes { scene, frame, width: 1.5, color: dark, path: Vec::new() };
            for line in &shown {
                for pair in line.windows(2) {
                    base.line(pair[0], pair[1]);
                }
            }
            base.finish();
            let mut dashes = Strokes { scene, frame, width: 1.0, color: light, path: Vec::new() };
            for line in &shown {
                march(line, phase, |a, b| dashes.line(a, b));
            }
            dashes.finish();
            scene.pop_clip();
        };
        let paint = if self.animated && !cx.reduced_motion() {
            Paint::Live(Rc::new(move |frame, time, scene| draw(frame, time as f32 * MARQUEE_SPEED, scene)))
        } else {
            Paint::Custom(Rc::new(move |frame, scene| draw(frame, 0.0, scene)))
        };
        let (root, surface) = self.frame.build(cx, paint);
        cx.semantics(surface, Semantics::new(Role::Image).name(name));
        root
    }
}

// ---------------------------------------------------------------------------------------------
// Eyedropper
// ---------------------------------------------------------------------------------------------

/// What an [`Eyedropper`] and an [`EyedropperArea`] share, and how a color reaches them: from
/// the area, or from whoever knows the color under the pointer through [`pick`](Self::pick).
/// Nothing here can read the screen, so the color always comes from the caller.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EyedropperState {
    armed: State<bool>,
    picked: State<Option<Color>>,
    on_pick: Option<Rc<dyn Fn(Color)>>,
    stay_armed: bool,
}

impl fmt::Debug for EyedropperState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EyedropperState").field("armed", &self.armed.get()).finish_non_exhaustive()
    }
}

impl Default for EyedropperState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl EyedropperState {
    /// Not armed, with no color yet.
    pub fn new() -> Self {
        Self { armed: State::new(false), picked: State::new(None), on_pick: None, stay_armed: false }
    }

    /// Runs `handler` with each color picked.
    pub fn on_pick(mut self, handler: impl Fn(Color) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }

    /// Stays armed after a pick, to take several in a row (by default it disarms).
    pub fn stay_armed(mut self, stay: bool) -> Self {
        self.stay_armed = stay;
        self
    }

    /// Whether the next click on the picture picks a color.
    pub fn is_armed(&self) -> bool {
        self.armed.get()
    }

    /// Arms it or disarms it.
    pub fn set_armed(&self, armed: bool) {
        self.armed.set_if_changed(armed);
    }

    /// The last color picked.
    pub fn picked(&self) -> Option<Color> {
        self.picked.get()
    }

    /// Tells it the color of the point clicked. It counts only while armed, and returns whether
    /// it did.
    pub fn pick(&self, color: Color) -> bool {
        if !self.armed.get() {
            return false;
        }
        self.picked.set(Some(color));
        if !self.stay_armed {
            self.armed.set(false);
        }
        if let Some(handler) = &self.on_pick {
            handler(color);
        }
        true
    }

    /// While armed, Escape disarms it wherever the focus is.
    fn listen_for_escape(&self, cx: &mut BuildCx<'_>) {
        if self.armed.get() {
            let armed = self.armed.clone();
            cx.shortcut(Key::Escape, false, Rc::new(move || armed.set(false)));
        }
    }
}

/// The button that arms an eyedropper, which looks pressed until a color is picked or Escape
/// disarms it. The color comes from an [`EyedropperArea`] or from
/// [`EyedropperState::pick`]: nothing here can read the screen.
///
/// ```
/// use ravenrs_core::{Color, Size};
/// use ravenrs_widgets::{ContentSpace, Eyedropper, EyedropperArea, EyedropperState};
///
/// let dropper = EyedropperState::new().on_pick(|color| println!("{color:?}"));
/// let button = Eyedropper::new(&dropper);
/// let area = EyedropperArea::new(&dropper, |at| Some(Color::rgb(at.x / 300.0, at.y / 200.0, 0.5)))
///     .space(ContentSpace::Fit(Size::new(300.0, 200.0)))
///     .size(300.0, 200.0);
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Eyedropper {
    state: EyedropperState,
    icon: IconName,
    size: ButtonSize,
    disabled: bool,
    label: Option<String>,
    hint: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Eyedropper {
    /// The button of this eyedropper.
    pub fn new(state: &EyedropperState) -> Self {
        Self {
            state: state.clone(),
            icon: IconName::Crosshair,
            size: ButtonSize::Small,
            disabled: false,
            label: None,
            hint: None,
        }
    }

    /// Its icon, instead of a crosshair.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// The size of the button.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Whether it can be armed.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The name screen readers announce, instead of the one in [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// What its tooltip says, instead of the one in [`Labels`](crate::Labels).
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl View for Eyedropper {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        self.state.listen_for_escape(cx);
        let armed = self.state.armed.clone();
        let is_armed = armed.get() && !self.disabled;
        let button = IconButton::new(self.icon)
            .label(self.label.clone().unwrap_or_else(|| labels.eyedropper.clone()))
            .size(self.size)
            .disabled(self.disabled)
            .selected(is_armed)
            .on_click(move || armed.update(|a| *a = !*a));
        let hint = self.hint.clone().unwrap_or_else(|| labels.eyedropper_hint.clone());
        cx.child(&Tooltip::new(button, hint))
    }
}

/// Where an eyedropper picks from, over a picture. While armed, a click asks `sampler` for the
/// color at that point of the content, in the units of the [space](Self::space); a point out of
/// the content or a `None` picks nothing and leaves it armed. While not armed it takes no input.
///
/// `CursorIcon` has no eyedropper, so while armed the cursor is the pointing hand.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EyedropperArea {
    state: EyedropperState,
    sampler: Rc<dyn Fn(Point) -> Option<Color>>,
    frame: OverlayBox,
    label: Option<String>,
}

impl fmt::Debug for EyedropperArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EyedropperArea").field("state", &self.state).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl EyedropperArea {
    /// An area of this eyedropper that reads colors with `sampler`.
    pub fn new(state: &EyedropperState, sampler: impl Fn(Point) -> Option<Color> + 'static) -> Self {
        Self { state: state.clone(), sampler: Rc::new(sampler), frame: OverlayBox::default(), label: None }
    }

    /// The name screen readers announce while armed, instead of the one in
    /// [`Labels`](crate::Labels).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

overlay_builders!(EyedropperArea, frame);

impl View for EyedropperArea {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let labels = cx.labels();
        self.state.listen_for_escape(cx);
        let (root, surface) = self.frame.build(cx, Paint::None);
        if !self.state.armed.get() {
            return root;
        }
        let name = self.label.clone().unwrap_or_else(|| labels.eyedropper_area.clone());
        cx.semantics(surface, Semantics::new(Role::Group).name(name));
        cx.cursor(surface, CursorIcon::Pointer);
        let (state, sampler, space) = (self.state.clone(), self.sampler.clone(), self.frame.space.clone());
        cx.on_drag(
            surface,
            Rc::new(move |d| {
                if d.phase != DragPhase::Start {
                    return;
                }
                let map = space.map(d.frame);
                if !map.placed.contains(d.position) {
                    return;
                }
                if let Some(color) = sampler(map.content(d.position)) {
                    state.pick(color);
                }
            }),
        );
        root
    }
}
