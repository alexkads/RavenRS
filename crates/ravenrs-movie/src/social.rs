//! A film told for a phone screen: a headline, the film and, under it, what is
//! being explained, in words big enough to read without sound.

use ravenrs_core::{Border, Role, Size};
use ravenrs_layout::{Align, Axis, Justify, LayoutStyle};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{BuildCx, NodeId, Paint, QuadPaint, Semantics, Text, VStack, View, ViewExt};

use crate::movie::{HotspotKind, Movie};
use crate::player::{MovieFrame, shown_at};
use crate::project::{OverlayKind, Place, Region};

const GAP: f32 = 24.0;

/// One moment of a film laid out for a tall or square screen, such as the videos of
/// social networks: the chapter on top, the film in the middle and, under it, the
/// title and words of what it explains, so it reads with the sound off.
///
/// The film takes the height left and is cut to a window that follows the spot it
/// explains, or the pointer, so it stays big enough to read on a phone.
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SocialFrame {
    movie: Movie,
    at_ms: u64,
    size: Size,
    brand: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SocialFrame {
    /// The moment `at_ms` of `movie` on a screen of `size` logical pixels.
    pub fn new(movie: Movie, at_ms: u64, size: Size) -> Self {
        Self { movie, at_ms, size, brand: String::new() }
    }

    /// A name shown at the foot of every frame, such as the product's.
    pub fn brand(mut self, brand: impl Into<String>) -> Self {
        self.brand = brand.into();
        self
    }
}

/// What the window over the film follows at `at`: the spot it explains, or else the
/// pointer, or else the middle, in logical pixels of the film.
fn focus(movie: &Movie, at: u64) -> (f32, f32) {
    let logical = movie.logical_size();
    if let Some(spot) = shown_at(movie, at).and_then(|i| movie.hotspots().get(i))
        && !spot.is_general()
    {
        return (spot.x + spot.width / 2.0, spot.y + spot.height / 2.0);
    }
    let pointer = match movie.project().place_at(at) {
        Some(Place::Film(ms)) => movie.pointer_at(ms),
        _ => None,
    };
    pointer.map_or((logical.width / 2.0, logical.height / 2.0), |p| (p.x, p.y))
}

/// The words under the film take about this much, and the headline and the name at
/// the foot about this.
const WORDS_HEIGHT: f32 = 190.0;
const HEADLINE_HEIGHT: f32 = 44.0;
const BRAND_HEIGHT: f32 = 24.0;

impl View for SocialFrame {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let at = self.at_ms;
        let padding = (self.size.width * 0.06).round();
        let width = self.size.width - padding * 2.0;
        let logical = self.movie.logical_size();
        let chapter = self.movie.chapter_at(at).and_then(|i| self.movie.chapters().get(i));
        let spot = shown_at(&self.movie, at).and_then(|i| self.movie.hotspots().get(i));
        let headline = chapter.map(|chapter| {
            Text::new(chapter.title.clone()).style(TextStyle::Title).font_weight(Weight::BOLD)
        });
        // The film fills the height that is left, cut to a window that follows what
        // it explains: a whole wide screen would be too small to read on a phone.
        let natural = width * logical.height / logical.width.max(1.0);
        let room =
            self.size.height - padding * 2.0 - HEADLINE_HEIGHT - WORDS_HEIGHT - BRAND_HEIGHT - GAP * 4.0;
        let window = room.clamp(natural, width * 1.25);
        let film_width = window * logical.width / logical.height.max(1.0);
        let scale = film_width / logical.width.max(1.0);
        let (fx, fy) = focus(&self.movie, at);
        let x = (fx * scale - width / 2.0).clamp(0.0, (film_width - width).max(0.0));
        let y = (fy * scale - window / 2.0)
            .clamp(0.0, (film_width * logical.height / logical.width.max(1.0) - window).max(0.0));
        let film = MovieFrame::new(self.movie.clone(), at).width(film_width).without_boxes();
        let film = cx.child(&film);
        let shifted = cx.container(
            LayoutStyle { absolute: Some((-x, -y)), ..LayoutStyle::stack(Axis::Vertical) },
            &[film],
            Paint::None,
        );
        let windowed = cx.container(
            LayoutStyle {
                width: Some(width),
                height: Some(window),
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[shifted],
            Paint::Quad(QuadPaint {
                corner_radius: theme.radii.lg,
                border: Some(Border { width: 1.0, color: theme.color(Semantic::Border) }),
                ..QuadPaint::new(self.movie.background())
            }),
        );
        cx.clip(windowed);
        // A step the film would wait at says so, since nobody clicks it here.
        let words = spot.map(|spot| {
            let eyebrow = (spot.kind == HotspotKind::Stop).then(|| {
                Text::new(chapter.map_or_else(String::new, |c| c.title.clone()))
                    .style(TextStyle::Caption)
                    .foreground(Semantic::Accent)
            });
            VStack::new((
                eyebrow,
                Text::new(spot.title.clone()).style(TextStyle::Headline).font_weight(Weight::SEMIBOLD),
                Text::new(spot.text.clone()).style(TextStyle::Body).foreground(Semantic::InkMuted),
            ))
            .spacing(8.0)
            .align(Align::Start)
            .padding(20.0)
            .background(theme.color(Semantic::SurfaceElevated))
            .corner_radius(theme.radii.lg)
            .border(1.0, theme.color(Semantic::Border))
        });
        let brand = (!self.brand.is_empty())
            .then(|| Text::new(self.brand.clone()).style(TextStyle::Callout).foreground(Semantic::Accent));
        // What is left over goes above the headline and above the name, so the
        // middle stays in the middle and the name at the foot.
        let grow_to_end = |cx: &mut BuildCx<'_>, child: Option<NodeId>| {
            cx.container(
                LayoutStyle {
                    grow: 1.0,
                    justify: Justify::End,
                    align: Align::Start,
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &child.into_iter().collect::<Vec<_>>(),
                Paint::None,
            )
        };
        let headline = headline.map(|headline| cx.child(&headline));
        let top = grow_to_end(cx, headline);
        let words = words.map(|words| cx.child(&words));
        let brand = brand.map(|brand| cx.child(&brand));
        let bottom = grow_to_end(cx, brand);
        let column = cx.container(
            LayoutStyle {
                spacing: GAP,
                grow: 1.0,
                align: Align::Stretch,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[Some(top), Some(windowed), words, Some(bottom)].into_iter().flatten().collect::<Vec<_>>(),
            Paint::None,
        );
        let node = cx.container(
            LayoutStyle {
                width: Some(self.size.width),
                height: Some(self.size.height),
                padding: padding.into(),
                align: Align::Stretch,
                overflow_hidden: true,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[column],
            Paint::Quad(QuadPaint::new(theme.color(Semantic::Canvas))),
        );
        cx.semantics(node, Semantics::new(Role::Image));
        cx.clip(node);
        node
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Movie {
    /// The same film with the camera brought `zoom` times closer to each spot it
    /// explains, for as long as it explains it. Spots about the whole screen, and
    /// moments the edits already zoom, stay as they are.
    pub fn following(&self, zoom: f32) -> Movie {
        let logical = self.logical_size();
        let zoom = zoom.max(1.0);
        let (shown_width, shown_height) = (logical.width / zoom, logical.height / zoom);
        let mut project = self.project().clone();
        let zoomed = |project: &crate::project::Project, from: u64, until: u64| {
            project.overlays.iter().any(|overlay| {
                matches!(overlay.kind, OverlayKind::Zoom { .. })
                    && overlay.start_ms < until
                    && from < overlay.end_ms()
            })
        };
        for spot in self.hotspots().iter().filter(|spot| !spot.is_general()) {
            if spot.until_ms <= spot.from_ms || zoomed(self.project(), spot.from_ms, spot.until_ms) {
                continue;
            }
            let (width, height) = (shown_width.max(spot.width), shown_height.max(spot.height));
            let x = (spot.x + spot.width / 2.0 - width / 2.0).clamp(0.0, (logical.width - width).max(0.0));
            let y =
                (spot.y + spot.height / 2.0 - height / 2.0).clamp(0.0, (logical.height - height).max(0.0));
            project.add_overlay(
                OverlayKind::Zoom { region: Region::new(x, y, width, height) },
                spot.from_ms,
                spot.until_ms - spot.from_ms,
            );
        }
        self.with_project(project)
    }
}
