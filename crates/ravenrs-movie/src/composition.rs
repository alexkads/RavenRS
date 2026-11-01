//! One moment of an edited film, drawn: the film, then everything over it.

use std::rc::Rc;

use ravenrs_core::{Border, Color, Effect, Point, Primitive, Quad, Rect, Role, Scene};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    BuildCx, Icon, IconName, NodeId, Paint, QuadPaint, Semantics, Text, VStack, View, ViewExt,
};

use crate::movie::Movie;
use crate::project::{CalloutStyle, ClipKind, Overlay, OverlayKind, Place, Region};

/// How long a zoom takes to travel, in milliseconds.
const ZOOM_TRAVEL_MS: f32 = 500.0;
/// How long the ring of a click lasts, in milliseconds.
const RIPPLE_MS: u64 = 400;
const POINTER_SIZE: f32 = 20.0;
const HALO_RADIUS: f32 = 30.0;
/// How long a soft callout takes to appear, in milliseconds.
const SOFT_MS: f32 = 300.0;

/// What part of the film fills the screen at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Camera {
    origin: Point,
    scale: f32,
}

impl Camera {
    /// A point of the film, on the screen.
    pub(crate) fn to_screen(self, p: Point) -> Point {
        Point::new((p.x - self.origin.x) * self.scale, (p.y - self.origin.y) * self.scale)
    }

    pub(crate) fn scale(self) -> f32 {
        self.scale
    }

    pub(crate) fn region(self, region: Region) -> Rect {
        let at = self.to_screen(Point::new(region.x, region.y));
        Rect::new(at.x, at.y, region.width * self.scale, region.height * self.scale)
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where the camera is at `at_ms`, for a screen `width` wide. With `whole`, zooms
/// do not count, which is how the editor places them.
pub(crate) fn camera_at(movie: &Movie, at_ms: u64, width: f32, whole: bool) -> Camera {
    let logical = movie.logical_size();
    let base = width / logical.width.max(1.0);
    let zoom = (!whole)
        .then(|| {
            movie.project().overlays.iter().find_map(|overlay| match overlay.kind {
                OverlayKind::Zoom { region } if overlay.covers(at_ms) => Some((overlay, region)),
                _ => None,
            })
        })
        .flatten();
    let Some((overlay, region)) = zoom else {
        return Camera { origin: Point::ZERO, scale: base };
    };
    let into = (at_ms - overlay.start_ms) as f32 / ZOOM_TRAVEL_MS;
    let left = (overlay.end_ms() - at_ms) as f32 / ZOOM_TRAVEL_MS;
    let amount = smooth(into.min(left));
    // The whole screen stays filled: the region grows to the film's proportions.
    let fit = (region.width / logical.width).max(region.height / logical.height).clamp(0.05, 1.0);
    let (shown_width, shown_height) = (logical.width * fit, logical.height * fit);
    let x = (region.x + region.width / 2.0 - shown_width / 2.0).clamp(0.0, logical.width - shown_width);
    let y = (region.y + region.height / 2.0 - shown_height / 2.0).clamp(0.0, logical.height - shown_height);
    let shown = 1.0 + (fit - 1.0) * amount;
    Camera { origin: Point::new(x * amount, y * amount), scale: base / shown }
}

/// Places a view at a spot of the screen, out of the flow.
struct At<V> {
    frame: Rect,
    view: V,
}

impl<V: View> View for At<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let child = cx.child(&self.view);
        cx.container(
            LayoutStyle {
                absolute: Some((self.frame.x(), self.frame.y())),
                width: Some(self.frame.width()),
                height: Some(self.frame.height()),
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[child],
            Paint::None,
        )
    }
}

/// A rectangle out of the flow, with what it paints.
fn patch(cx: &mut BuildCx<'_>, frame: Rect, paint: QuadPaint) -> NodeId {
    let node = cx.container(
        LayoutStyle {
            absolute: Some((frame.x(), frame.y())),
            width: Some(frame.width()),
            height: Some(frame.height()),
            ..LayoutStyle::stack(Axis::Vertical)
        },
        &[],
        Paint::Quad(paint),
    );
    cx.semantics(node, Semantics::hidden());
    node
}

/// One moment of an edited film, `width` wide and as tall as its proportions say.
/// The player and the editor both draw with it, so what is edited is what is shown.
#[derive(Debug)]
pub struct Composition {
    movie: Movie,
    at_ms: u64,
    width: f32,
    whole: bool,
}

impl Composition {
    /// The moment `at_ms` of the timeline of `movie`.
    pub fn new(movie: Movie, at_ms: u64) -> Self {
        Self { movie, at_ms, width: 960.0, whole: false }
    }

    /// Its width, in logical pixels (960 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shows the whole screen even during a zoom, to place one.
    pub(crate) fn ignoring_zoom(mut self) -> Self {
        self.whole = true;
        self
    }

    fn height(&self) -> f32 {
        let logical = self.movie.logical_size();
        self.width * logical.height / logical.width.max(1.0)
    }

    fn film(&self, cx: &mut BuildCx<'_>, camera: Camera, film_ms: u64) -> NodeId {
        let (movie, background) = (self.movie.clone(), self.movie.background());
        let list = self.movie.draw_list(film_ms);
        let draw = Rc::new(move |frame: Rect, scene: &mut Scene| {
            // A zoom draws the film larger than the screen, so it has to be cut at the edge.
            scene.push_clip(frame);
            scene.push_quad(Quad::new(frame, background));
            let Some((_, list)) = &list else {
                scene.pop_clip();
                return;
            };
            let start = scene.len();
            let drawn: Vec<Primitive> = list.iter().filter_map(|i| movie.primitive(*i).cloned()).collect();
            scene.extend_from_slice(&drawn);
            let offset = Point::new(
                frame.x() - camera.origin.x * camera.scale,
                frame.y() - camera.origin.y * camera.scale,
            );
            let end = scene.len();
            scene.apply_effect(start..end, Effect { opacity: 1.0, offset, scale: camera.scale }, Point::ZERO);
            scene.pop_clip();
        });
        let node = cx.container(
            LayoutStyle {
                absolute: Some((0.0, 0.0)),
                width: Some(self.width),
                height: Some(self.height()),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[],
            Paint::Custom(draw),
        );
        cx.semantics(node, Semantics::new(Role::Image));
        node
    }

    fn title_card(&self, cx: &mut BuildCx<'_>, clip: usize) -> NodeId {
        let theme = *cx.theme();
        let (title, subtitle) = match self.movie.project().clips.get(clip).map(|c| &c.kind) {
            Some(ClipKind::Title { title, subtitle, .. }) => (title.clone(), subtitle.clone()),
            _ => (String::new(), String::new()),
        };
        let words = VStack::new((
            Text::new(title).style(TextStyle::Title).font_weight(Weight::SEMIBOLD),
            Text::new(subtitle).foreground(Semantic::InkMuted),
        ))
        .spacing(8.0)
        .align(Align::Center);
        let words = cx.child(&words);
        cx.container(
            LayoutStyle {
                absolute: Some((0.0, 0.0)),
                width: Some(self.width),
                height: Some(self.height()),
                align: Align::Center,
                justify: Justify::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[words],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Surface))),
        )
    }

    fn overlay(&self, cx: &mut BuildCx<'_>, overlay: &Overlay, camera: Camera, children: &mut Vec<NodeId>) {
        let theme = *cx.theme();
        let scrim = theme.color(Semantic::Scrim);
        match &overlay.kind {
            OverlayKind::Hide { region } => {
                let paint = QuadPaint {
                    corner_radius: theme.radii.sm,
                    ..QuadPaint::new(theme.color(Semantic::Control))
                };
                children.push(patch(cx, camera.region(*region), paint));
            }
            OverlayKind::Spotlight { region } => {
                let lit = camera.region(*region);
                let (width, height) = (self.width, self.height());
                let right = lit.x() + lit.width();
                let bottom = lit.y() + lit.height();
                for frame in [
                    Rect::new(0.0, 0.0, width, lit.y().max(0.0)),
                    Rect::new(0.0, bottom, width, (height - bottom).max(0.0)),
                    Rect::new(0.0, lit.y(), lit.x().max(0.0), lit.height()),
                    Rect::new(right, lit.y(), (width - right).max(0.0), lit.height()),
                ] {
                    children.push(patch(cx, frame, QuadPaint::new(scrim)));
                }
            }
            OverlayKind::Callout { text, region, style, soft } => {
                let frame = camera.region(*region);
                // Gradually means by its color: the words and the balloon fade together.
                let age = (self.at_ms - overlay.start_ms).min(overlay.end_ms() - self.at_ms) as f32;
                let alpha = if *soft { smooth(age / SOFT_MS) } else { 1.0 };
                let (fill, ink, edge) = match style {
                    CalloutStyle::Balloon => (Semantic::Surface, Semantic::Ink, Semantic::Accent),
                    CalloutStyle::Label => (Semantic::Accent, Semantic::OnAccent, Semantic::Accent),
                };
                let balloon = Text::new(text.clone())
                    .style(TextStyle::Callout)
                    .foreground(theme.color(ink).faded(alpha))
                    .padding((8.0, 12.0))
                    .background(theme.color(fill).faded(alpha))
                    .corner_radius(if *style == CalloutStyle::Label {
                        frame.height().min(999.0)
                    } else {
                        theme.radii.md
                    })
                    .border(2.0, theme.color(edge).faded(alpha));
                children.push(cx.child(&At { frame, view: balloon }));
            }
            OverlayKind::Caption { text } => {
                let height = 44.0;
                let frame = Rect::new(0.0, self.height() - height - 12.0, self.width, height);
                let line = Text::new(text.clone())
                    .style(TextStyle::Callout)
                    .foreground(Color::WHITE)
                    .padding((6.0, 14.0))
                    .background(scrim)
                    .corner_radius(theme.radii.sm);
                let centered = VStack::new((line,)).align(Align::Center);
                children.push(cx.child(&At { frame, view: centered }));
            }
            OverlayKind::FadeIn | OverlayKind::FadeOut => {
                let t = (self.at_ms - overlay.start_ms) as f32 / overlay.duration_ms.max(1) as f32;
                let dark = if matches!(overlay.kind, OverlayKind::FadeIn) { 1.0 - t } else { t };
                let frame = Rect::new(0.0, 0.0, self.width, self.height());
                children.push(patch(
                    cx,
                    frame,
                    QuadPaint::new(Color::rgba(0.0, 0.0, 0.0, dark.clamp(0.0, 1.0))),
                ));
            }
            // These are drawn by others: the camera, the player, the pointer.
            _ => {}
        }
    }

    fn pointer(&self, cx: &mut BuildCx<'_>, camera: Camera, film_ms: u64, children: &mut Vec<NodeId>) {
        let theme = *cx.theme();
        let accent = theme.color(Semantic::Accent);
        let highlighted = self
            .movie
            .project()
            .overlays
            .iter()
            .any(|o| matches!(o.kind, OverlayKind::PointerHighlight) && o.covers(self.at_ms));
        let Some(position) = self.movie.pointer_at(film_ms) else {
            return;
        };
        let at = camera.to_screen(position);
        if highlighted {
            let frame =
                Rect::new(at.x - HALO_RADIUS, at.y - HALO_RADIUS, HALO_RADIUS * 2.0, HALO_RADIUS * 2.0);
            let paint = QuadPaint { corner_radius: HALO_RADIUS, ..QuadPaint::new(accent.faded(0.22)) };
            children.push(patch(cx, frame, paint));
        }
        if let Some((click, age)) = self.movie.click_before(film_ms)
            && age < RIPPLE_MS
        {
            let progress = age as f32 / RIPPLE_MS as f32;
            let radius = (8.0 + 24.0 * progress) * if highlighted { 1.5 } else { 1.0 };
            let center = camera.to_screen(click);
            let frame = Rect::new(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0);
            let paint = QuadPaint {
                fill: Color::TRANSPARENT.into(),
                corner_radius: radius,
                border: Some(Border { width: 3.0, color: accent.faded(1.0 - progress) }),
                shadow: None,
            };
            children.push(patch(cx, frame, paint));
        }
        // The arrow's tip is at the corner of the icon, about where the pointer is.
        for (grow, color) in [(4.0, Color::WHITE), (0.0, theme.color(Semantic::Ink))] {
            let size = POINTER_SIZE + grow;
            let frame = Rect::new(at.x - 3.0 - grow / 2.0, at.y - 3.0 - grow / 2.0, size, size);
            let icon = Icon::new(IconName::MousePointer).size(size).color(color);
            let node = cx.child(&At { frame, view: icon });
            cx.semantics(node, Semantics::hidden());
            children.push(node);
        }
    }
}

impl View for Composition {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let camera = camera_at(&self.movie, self.at_ms, self.width, self.whole);
        let place = self.movie.project().place_at(self.at_ms);
        let mut children = Vec::new();
        let film_ms = match place {
            Some(Place::Title(clip)) => {
                children.push(self.title_card(cx, clip));
                None
            }
            Some(Place::Film(ms)) => {
                children.push(self.film(cx, camera, ms));
                Some(ms)
            }
            None => None,
        };
        let overlays: Vec<Overlay> =
            self.movie.project().overlays.iter().filter(|o| o.covers(self.at_ms)).cloned().collect();
        // What covers the film goes first, the pointer over it, the words over all.
        for overlay in overlays
            .iter()
            .filter(|o| matches!(o.kind, OverlayKind::Hide { .. } | OverlayKind::Spotlight { .. }))
        {
            self.overlay(cx, overlay, camera, &mut children);
        }
        if let Some(ms) = film_ms {
            self.pointer(cx, camera, ms, &mut children);
        }
        for overlay in overlays
            .iter()
            .filter(|o| !matches!(o.kind, OverlayKind::Hide { .. } | OverlayKind::Spotlight { .. }))
        {
            self.overlay(cx, overlay, camera, &mut children);
        }
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(self.height()),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::None,
        );
        cx.clip(node);
        node
    }
}
