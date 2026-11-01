//! Plays a film, stops where it teaches, and lets the watcher jump around.

use std::cell::Cell;
use std::rc::Rc;

use ravenrs_core::{Border, CursorIcon, Role};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    BuildCx, Button, ButtonSize, ButtonStyle, Card, HStack, Icon, IconName, List, ListItem, NodeId, Paint,
    QuadPaint, ScrollView, Semantics, Slider, Spacer, Text, Toggle, VStack, View, ViewExt,
};

use crate::composition::{Composition, camera_at};
use crate::movie::{Hotspot, HotspotKind, Movie};
use crate::project::Region;

/// How often it draws while playing, in seconds. The pointer and the zooms have
/// no frames of their own, so they move at this pace.
const FRAME: f64 = 1.0 / 30.0;
const CHAPTERS_WIDTH: f32 = 260.0;
const COLUMN_GAP: f32 = 16.0;
/// Below this total width, the chapters go under the film.
const SIDE_BY_SIDE_FROM: f32 = 720.0;

/// The words a [`MoviePlayer`] shows. English by default.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MovieLabels {
    /// The play button.
    pub play: String,
    /// The pause button.
    pub pause: String,
    /// Goes back to the beginning.
    pub restart: String,
    /// Stops the film at each step that asks for a click.
    pub guided: String,
    /// Goes past a step without clicking on the film.
    pub next_step: String,
    /// The heading of the chapters.
    pub chapters: String,
    /// The film's position, for screen readers.
    pub position: String,
    /// The film itself, for screen readers.
    pub movie: String,
}

impl Default for MovieLabels {
    fn default() -> Self {
        Self {
            play: "Play".into(),
            pause: "Pause".into(),
            restart: "From the start".into(),
            guided: "Stop at each step".into(),
            next_step: "Continue".into(),
            chapters: "Chapters".into(),
            position: "Position".into(),
            movie: "Film".into(),
        }
    }
}

impl MovieLabels {
    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            play: "Reproduzir".into(),
            pause: "Pausar".into(),
            restart: "Do início".into(),
            guided: "Parar em cada passo".into(),
            next_step: "Continuar".into(),
            chapters: "Capítulos".into(),
            position: "Posição".into(),
            movie: "Filme".into(),
        }
    }
}

/// `2:05`.
fn clock_label(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Where a film is, read and written while building. Clicks raise `changes`, so
/// the player rebuilds; while playing, it asks to be woken for the next picture.
#[derive(Debug, Default)]
struct Playhead {
    position_ms: Cell<u64>,
    /// Playing since this clock reading, from this position.
    running: Cell<Option<(f64, u64)>>,
    /// The stop it is waiting at.
    waiting: Cell<Option<usize>>,
    /// The note someone clicked, shown until the film moves on.
    opened: Cell<Option<usize>>,
}

impl Playhead {
    fn is_running(&self) -> bool {
        self.running.get().is_some()
    }

    fn pause(&self) {
        self.running.set(None);
    }

    fn seek(&self, ms: u64) {
        self.position_ms.set(ms);
        self.waiting.set(None);
        self.opened.set(None);
        if self.is_running() {
            self.running.set(Some((f64::NAN, ms)));
        }
    }

    fn play(&self, movie: &Movie) {
        // At the end, playing means playing again.
        if self.position_ms.get() >= movie.duration_ms() {
            self.position_ms.set(0);
        }
        self.waiting.set(None);
        self.opened.set(None);
        self.running.set(Some((f64::NAN, self.position_ms.get())));
    }

    /// Moves to where the clock says it should be. The first stop crossed on the
    /// way holds it there, and the end of the film finishes it.
    fn advance(&self, movie: &Movie, guided: bool, now: f64) {
        let Some((since, from)) = self.running.get() else {
            return;
        };
        // A click has no clock, so playing starts counting from the next build.
        if since.is_nan() {
            self.running.set(Some((now, from)));
            return;
        }
        let target = from + ((now - since).max(0.0) * 1000.0) as u64;
        let before = self.position_ms.get();
        let stop = guided
            .then(|| {
                movie.hotspots().iter().position(|spot| {
                    spot.kind == HotspotKind::Stop && spot.from_ms > before && spot.from_ms <= target
                })
            })
            .flatten();
        if let Some(stop) = stop {
            self.position_ms.set(movie.hotspots()[stop].from_ms);
            self.waiting.set(Some(stop));
            self.running.set(None);
        } else if target >= movie.duration_ms() {
            self.position_ms.set(movie.duration_ms());
            self.running.set(None);
        } else {
            self.position_ms.set(target);
        }
    }
}

/// The mark over a spot on the film.
struct HotspotMark {
    hotspot: Hotspot,
    frame: ravenrs_core::Rect,
    waiting: bool,
    on_click: Rc<dyn Fn()>,
}

impl View for HotspotMark {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let color = theme.color(Semantic::Accent);
        let hovered = cx.is_hovered();
        let node = cx.container(
            LayoutStyle {
                absolute: Some((self.frame.x(), self.frame.y())),
                width: Some(self.frame.width()),
                height: Some(self.frame.height()),
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &[],
            Paint::Quad(QuadPaint {
                fill: color.faded(if self.waiting || hovered { 0.28 } else { 0.14 }).into(),
                corner_radius: theme.radii.md,
                border: Some(Border { width: 2.0, color }),
                shadow: None,
            }),
        );
        cx.semantics(
            node,
            Semantics::new(Role::Button)
                .name(self.hotspot.title.clone())
                .description(self.hotspot.text.clone()),
        );
        cx.on_click(node, self.on_click.clone());
        cx.focusable(node, theme.radii.md);
        cx.cursor(node, CursorIcon::Pointer);
        node
    }
}

/// The film and the marks over it.
struct Stage<'a> {
    movie: &'a Movie,
    playhead: Rc<Playhead>,
    width: f32,
    marks: Vec<Hotspot>,
    changes: State<u64>,
    labels: &'a MovieLabels,
}

impl View for Stage<'_> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let mut at = self.playhead.position_ms.get();
        // Nothing has played yet: the first thing seen should not be a black screen.
        if at == 0 && !self.playhead.is_running() {
            at = self.movie.poster_ms();
        }
        let logical = self.movie.logical_size();
        let height = self.width * logical.height / logical.width.max(1.0);
        let camera = camera_at(self.movie, at, self.width, false);
        let film = Composition::new(self.movie.clone(), at).width(self.width);
        let film = cx.child(&film);
        cx.semantics(film, Semantics::new(Role::Image).name(self.labels.movie.clone()));
        let mut children = vec![film];
        for hotspot in &self.marks {
            let waiting = hotspot.kind == HotspotKind::Stop;
            let index = self.movie.hotspots().iter().position(|spot| spot == hotspot);
            let (playhead, changes, duration) =
                (self.playhead.clone(), self.changes.clone(), self.movie.duration_ms());
            let on_click: Rc<dyn Fn()> = Rc::new(move || {
                if waiting {
                    let at = playhead.position_ms.get();
                    playhead.waiting.set(None);
                    playhead.running.set(Some((f64::NAN, at.min(duration))));
                } else {
                    playhead.opened.set(index);
                }
                changes.update(|n| *n += 1);
            });
            let region = Region::new(hotspot.x, hotspot.y, hotspot.width, hotspot.height);
            let mark =
                HotspotMark { hotspot: hotspot.clone(), frame: camera.region(region), waiting, on_click };
            children.push(cx.child(&mark));
        }
        let node = cx.container(
            LayoutStyle {
                width: Some(self.width),
                height: Some(height),
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

/// Plays a [`Movie`] as an interactive manual.
///
/// Whoever watches can pause, drag the position, jump to a chapter and click the
/// marks on the film. With "stop at each step" on, which is how it starts, the
/// film holds at each [`Recorder::guide`](crate::Recorder::guide) until the marked spot is clicked,
/// so people follow it at their own pace. It sleeps while paused.
///
/// Cloning a film is cheap, since the film itself is shared:
///
/// ```no_run
/// use ravenrs_movie::{Movie, MoviePlayer};
///
/// let movie = Movie::from_bytes(&std::fs::read("manual.ravenmovie").unwrap_or_default())?;
/// let player = MoviePlayer::new(movie).width(960.0);
/// # Ok::<(), ravenrs_movie::MovieError>(())
/// ```
#[derive(Debug)]
pub struct MoviePlayer {
    movie: Movie,
    width: f32,
    labels: MovieLabels,
}

impl MoviePlayer {
    /// A player for `movie`, stopped at its first picture.
    pub fn new(movie: Movie) -> Self {
        Self { movie, width: 960.0, labels: MovieLabels::default() }
    }

    /// The width of the whole player, in logical pixels (960 by default). The
    /// film gets what its chapters leave.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The words it shows.
    pub fn labels(mut self, labels: MovieLabels) -> Self {
        self.labels = labels;
        self
    }

    fn chapters(&self, playhead: &Rc<Playhead>, changes: &State<u64>) -> Option<impl View + use<>> {
        let movie = &self.movie;
        if movie.chapters().is_empty() {
            return None;
        }
        let current = movie.chapter_at(playhead.position_ms.get());
        let rows: Vec<ListItem> = movie
            .chapters()
            .iter()
            .enumerate()
            .map(|(i, chapter)| {
                let (playhead, changes, at) = (playhead.clone(), changes.clone(), chapter.at_ms);
                ListItem::new(chapter.title.clone())
                    .subtitle(clock_label(chapter.at_ms))
                    .selected(current == Some(i))
                    .on_click(move || {
                        playhead.seek(at);
                        changes.update(|n| *n += 1);
                    })
            })
            .collect();
        Some(
            VStack::new((
                Text::new(self.labels.chapters.clone())
                    .style(TextStyle::Callout)
                    .font_weight(Weight::SEMIBOLD),
                ScrollView::new((List::new(rows),)),
            ))
            .spacing(8.0)
            .align(Align::Stretch),
        )
    }

    /// What the film is saying right now: the waiting step, an opened or running
    /// note, or what the chapter teaches.
    fn caption(&self, playhead: &Rc<Playhead>, changes: &State<u64>) -> Option<impl View + use<>> {
        let movie = self.movie.clone();
        let at = playhead.position_ms.get();
        let spots = movie.hotspots();
        let waiting = playhead.waiting.get().and_then(|i| spots.get(i));
        let opened = playhead.opened.get().and_then(|i| spots.get(i));
        let running = spots
            .iter()
            .find(|spot| spot.kind == HotspotKind::Note && spot.from_ms <= at && at < spot.until_ms);
        let said =
            waiting.or(opened).or(running).map(|spot| (spot.title.clone(), spot.text.clone())).or_else(
                || {
                    movie
                        .chapter_at(at)
                        .and_then(|i| movie.chapters().get(i))
                        .map(|c| (c.title.clone(), c.text.clone()))
                },
            )?;
        let go_on = waiting.map(|_| {
            let (playhead, changes, player) = (playhead.clone(), changes.clone(), movie.clone());
            Button::new(self.labels.next_step.clone())
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Small)
                .icon(IconName::ArrowRight)
                .on_click(move || {
                    playhead.play(&player);
                    changes.update(|n| *n += 1);
                })
        });
        Some(Card::new(
            HStack::new((
                Icon::new(if waiting.is_some() { IconName::MousePointer } else { IconName::Info })
                    .color(Semantic::Accent),
                VStack::new((
                    Text::new(said.0).font_weight(Weight::SEMIBOLD),
                    Text::new(said.1).style(TextStyle::Callout).foreground(Semantic::InkMuted),
                ))
                .spacing(2.0)
                .grow(1.0),
                go_on,
            ))
            .spacing(12.0)
            .align(Align::Center),
        ))
    }

    fn controls(
        &self,
        playhead: &Rc<Playhead>,
        changes: &State<u64>,
        guided: &State<bool>,
        film_width: f32,
    ) -> impl View + use<> {
        let duration = self.movie.duration_ms();
        let running = playhead.is_running();
        let (toggle_head, toggle_changes, toggle_movie) =
            (playhead.clone(), changes.clone(), self.movie.clone());
        let (back_head, back_changes) = (playhead.clone(), changes.clone());
        let (read, write, moved) = (playhead.clone(), playhead.clone(), changes.clone());
        let position = Binding::new(
            move || read.position_ms.get() as f32,
            move |ms: f32| {
                write.seek(ms.max(0.0) as u64);
                moved.update(|n| *n += 1);
            },
        );
        HStack::new((
            Button::new(if running { self.labels.pause.clone() } else { self.labels.play.clone() })
                .icon(if running { IconName::Pause } else { IconName::Play })
                .size(ButtonSize::Small)
                .on_click(move || {
                    if toggle_head.is_running() {
                        toggle_head.pause();
                    } else {
                        toggle_head.play(&toggle_movie);
                    }
                    toggle_changes.update(|n| *n += 1);
                }),
            Button::new(self.labels.restart.clone())
                .icon(IconName::RotateCcw)
                .style(ButtonStyle::Ghost)
                .size(ButtonSize::Small)
                .on_click(move || {
                    back_head.seek(0);
                    back_changes.update(|n| *n += 1);
                }),
            Slider::new(position)
                .range(0.0, duration.max(1) as f32)
                .step(100.0)
                .label(self.labels.position.clone())
                .grow(1.0),
            Text::new(format!("{} / {}", clock_label(playhead.position_ms.get()), clock_label(duration)))
                .style(TextStyle::Caption)
                .foreground(Semantic::InkMuted),
            Spacer::new().width(4.0),
            Toggle::new(guided.binding()).label(self.labels.guided.clone()),
        ))
        .spacing(8.0)
        .align(Align::Center)
        .width(film_width)
    }
}

impl View for MoviePlayer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let changes: State<u64> = cx.use_state(|| 0);
        let guided: State<bool> = cx.use_state(|| true);
        let playhead: State<Rc<Playhead>> = cx.use_state(|| Rc::new(Playhead::default()));
        // Read, so that a click rebuilds this view.
        let _ = changes.get();
        let playhead = playhead.get();
        let movie = &self.movie;
        let now = cx.now();
        playhead.advance(movie, guided.get(), now);

        if playhead.is_running() {
            cx.wake_at(now + FRAME);
        }

        let side_by_side = self.width >= SIDE_BY_SIDE_FROM && !movie.chapters().is_empty();
        let film_width = if side_by_side { self.width - CHAPTERS_WIDTH - COLUMN_GAP } else { self.width };
        let at = playhead.position_ms.get();
        let waiting = playhead.waiting.get();
        let marks: Vec<Hotspot> = movie
            .hotspots()
            .iter()
            .enumerate()
            .filter(|(i, spot)| match spot.kind {
                HotspotKind::Stop => waiting == Some(*i),
                HotspotKind::Note => spot.from_ms <= at && at < spot.until_ms,
            })
            .map(|(_, spot)| spot.clone())
            .collect();

        let stage = Stage {
            movie,
            playhead: playhead.clone(),
            width: film_width,
            marks,
            changes: changes.clone(),
            labels: &self.labels,
        };
        let stage = cx.child(&stage);
        let controls = cx.child(&self.controls(&playhead, &changes, &guided, film_width));
        let mut column = vec![stage, controls];
        if let Some(caption) = self.caption(&playhead, &changes) {
            let caption = cx.child(&caption.width(film_width));
            column.push(caption);
        }
        let column = cx.container(
            LayoutStyle { spacing: 12.0, ..LayoutStyle::stack(Axis::Vertical) },
            &column,
            Paint::None,
        );
        let chapters = self.chapters(&playhead, &changes).map(|chapters| {
            let view = chapters.width(if side_by_side { CHAPTERS_WIDTH } else { self.width });
            cx.child(&view)
        });
        let children: Vec<NodeId> = [Some(column), chapters].into_iter().flatten().collect();
        cx.container(
            LayoutStyle {
                spacing: COLUMN_GAP,
                ..LayoutStyle::stack(if side_by_side { Axis::Horizontal } else { Axis::Vertical })
            },
            &children,
            Paint::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::clock_label;

    #[test]
    fn the_clock_reads_like_one() {
        assert_eq!(clock_label(0), "0:00");
        assert_eq!(clock_label(61_500), "1:01");
        assert_eq!(clock_label(600_000), "10:00");
    }
}
