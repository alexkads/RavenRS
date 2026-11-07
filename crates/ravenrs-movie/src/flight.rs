//! The last minutes on screen, always at hand, and a bug report made from them.
//!
//! Nothing here leaves the computer by itself. What is recorded stays in memory
//! until someone opens a report, sees the film, covers what should not show and
//! chooses to send it, through whatever the app does with [`BugReport`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_app_i18n::{Language, Texts};
use ravenrs_core::{AccessTree, Border, Color, CursorIcon, Point, Primitive, Rect, Role, Size};
use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    BuildCx, Button, ButtonStyle, Dialog, Drag, DragPhase, IconName, NodeId, Paint, Placement, QuadPaint,
    SegmentedControl, Semantics, Text, TextField, Toggle, UiActivity, VStack, View, ViewExt,
};
use serde::{Deserialize, Serialize};

use crate::editor::{EditorLabels, EditorState, MovieEditor};
use crate::movie::{FilmBuilder, Movie, MovieError};
use crate::player::{MovieLabels, MoviePlayer};
use crate::project::Region;

const REPORT_MAGIC: &[u8] = b"RAVENBUG1\n";
/// The fewest seconds between two frames it keeps: a report does not need more,
/// and the app it watches should not pay for more.
const SHORTEST_GAP: f64 = 1.0 / 15.0;
const PREVIEW_WIDTH: f32 = 640.0;
/// A smaller box is a click that slipped, not a choice.
const SMALLEST_MARK: f32 = 12.0;
/// How many elements under the mark a report names, the first in reading order.
const MARKED_ELEMENTS: usize = 12;
/// How much the page dims while marking: enough to tell the mode, not to hide the page.
const MARKING_DIM: f32 = 0.25;

#[derive(Clone, Debug)]
struct Stretch {
    film: FilmBuilder,
    started: f64,
}

#[derive(Debug)]
struct Flight {
    keep_seconds: f64,
    size: Size,
    newer: Option<Stretch>,
    older: Option<Stretch>,
    last_frame: Vec<Primitive>,
    last_at: f64,
    now: f64,
    paused: bool,
    /// What the screen had when last asked, for naming what was marked.
    described: Option<AccessTree>,
}

impl Flight {
    /// The stretch being written, starting another when this one is full. Two
    /// stretches of half the time each always hold at least that half, and old
    /// memory goes all at once instead of frame by frame.
    fn stretch(&mut self, now: f64, background: ravenrs_core::Color) -> &mut Stretch {
        let full = self.newer.as_ref().is_none_or(|s| now - s.started >= self.keep_seconds / 2.0);
        if full {
            self.older = self.newer.take();
            self.last_frame.clear();
            self.newer = Some(Stretch { film: FilmBuilder::new(self.size, background), started: now });
        }
        self.newer
            .get_or_insert_with(|| Stretch { film: FilmBuilder::new(self.size, background), started: now })
    }

    fn painted(&mut self, scene: &ravenrs_core::Scene, now: f64) {
        self.now = now;
        let primitives = scene.primitives();
        if self.paused || (now - self.last_at < SHORTEST_GAP && !self.last_frame.is_empty()) {
            return;
        }
        if self.last_frame == primitives
            && self.newer.as_ref().is_some_and(|s| now - s.started < self.keep_seconds / 2.0)
        {
            return;
        }
        let stretch = self.stretch(now, scene.clear_color());
        let at = ((now - stretch.started).max(0.0) * 1000.0) as u64;
        stretch.film.frame(at, primitives);
        self.last_frame = primitives.to_vec();
        self.last_at = now;
    }

    fn pointer(&mut self, position: Point, clicked: bool, now: f64) {
        self.now = now;
        if self.paused {
            return;
        }
        if let Some(stretch) = &mut self.newer {
            let at = ((now - stretch.started).max(0.0) * 1000.0) as u64;
            stretch.film.pointer(at, position);
            if clicked {
                stretch.film.click(at, position);
            }
        }
    }

    /// The stretches as one film, without disturbing the recording.
    fn film(&self) -> Option<Movie> {
        let newer = self.newer.as_ref()?;
        let stretches: Vec<&Stretch> = self.older.iter().chain([newer]).collect();
        let origin = stretches[0].started;
        let mut whole: Option<FilmBuilder> = None;
        let mut last_ms = 0;
        for stretch in stretches {
            let ms = ((self.now - stretch.started).max(0.0) * 1000.0) as u64;
            let Ok(part) = stretch.film.clone().finish(ms) else {
                continue;
            };
            let shift = ((stretch.started - origin).max(0.0) * 1000.0) as u64;
            let logical = part.logical_size();
            let background = part.background();
            let film = whole.get_or_insert_with(|| FilmBuilder::new(logical, background));
            for (at, list) in part.frames() {
                let drawn: Vec<Primitive> = list.iter().filter_map(|i| part.primitive(*i).cloned()).collect();
                film.frame(at + shift, &drawn);
            }
            for key in part.pointer_keys() {
                film.pointer(key.at_ms + shift, Point::new(key.x, key.y));
            }
            for key in part.click_keys() {
                film.click(key.at_ms + shift, Point::new(key.x, key.y));
            }
            last_ms = last_ms.max(ms + shift);
        }
        whole?.finish(last_ms).ok()
    }
}

/// Keeps the last minutes on screen in memory, the way a flight recorder does, so
/// that a bug can show what led to it. Put it anywhere in the app's views: it
/// draws nothing. A film costs little because each thing drawn is stored once and
/// a screen that does not change costs nothing, and it runs only when the app paints.
///
/// It sees all that the app shows, so it writes nothing anywhere: only
/// [`Self::film`] gets anything out of it.
///
/// ```
/// use ravenrs_movie::FlightRecorder;
///
/// // The last three minutes.
/// let recorder = FlightRecorder::new(180.0);
/// assert!(recorder.film().is_none(), "nothing on screen yet");
/// ```
#[derive(Clone, Debug)]
pub struct FlightRecorder {
    flight: Rc<RefCell<Flight>>,
}

impl FlightRecorder {
    /// A recorder that keeps at least half of `seconds`, and up to all of them.
    pub fn new(seconds: f64) -> Self {
        Self {
            flight: Rc::new(RefCell::new(Flight {
                keep_seconds: seconds.max(1.0),
                size: Size::new(1.0, 1.0),
                newer: None,
                older: None,
                last_frame: Vec::new(),
                last_at: f64::NEG_INFINITY,
                now: 0.0,
                paused: false,
                described: None,
            })),
        }
    }

    /// The film so far, to look at, edit or send. Recording goes on.
    pub fn film(&self) -> Option<Movie> {
        self.flight.borrow().film()
    }

    fn take_described(&self) -> Option<AccessTree> {
        self.flight.borrow_mut().described.take()
    }

    /// Stops or resumes noting anything, for screens that should stay out of it.
    pub fn set_paused(&self, paused: bool) {
        self.flight.borrow_mut().paused = paused;
    }
}

impl View for FlightRecorder {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let size = cx.viewport();
        {
            let mut flight = self.flight.borrow_mut();
            if flight.size != size {
                // A film has one size, so a resized window starts over.
                flight.size = size;
                flight.newer = None;
                flight.older = None;
                flight.last_frame.clear();
            }
        }
        let flight = self.flight.clone();
        cx.observe_activity(Rc::new(move |activity: UiActivity<'_>, now: f64| {
            // Its own dialog paints inside a borrow only if it reads the film while
            // painting, which it does not; if that changes, a frame is skipped, no more.
            let Ok(mut flight) = flight.try_borrow_mut() else {
                return;
            };
            match activity {
                UiActivity::Painted(scene) => flight.painted(scene, now),
                UiActivity::PointerMoved(position) => flight.pointer(position, false, now),
                UiActivity::Clicked(position) => flight.pointer(position, true, now),
                UiActivity::Described(tree) => flight.described = Some(tree.clone()),
                _ => {}
            }
        }));
        cx.empty()
    }
}

/// What someone says about a bug, for the development team.
#[derive(Clone, Debug, Default)]
pub struct BugReport {
    /// What kind of report it is, in the words of [`BugLabels::kinds`].
    pub kind: String,
    /// What happened, in their words.
    pub description: String,
    /// The box they drew around the problem, in the logical pixels of the window.
    pub area: Option<Region>,
    /// What was on screen under that box, by the names screen readers use.
    pub elements: Vec<String>,
    /// The app's last errors, from [`BugReporter::errors`].
    pub errors: Vec<String>,
    /// The film of what led to it, if they chose to include it.
    pub film: Option<Movie>,
    /// What the app adds, such as its version. Shown before it is sent.
    pub details: Vec<(String, String)>,
}

/// What is not in the older files is simply empty there.
#[derive(Serialize, Deserialize)]
struct ReportHeader {
    #[serde(default)]
    kind: String,
    description: String,
    #[serde(default)]
    area: Option<Region>,
    #[serde(default)]
    elements: Vec<String>,
    #[serde(default)]
    errors: Vec<String>,
    details: Vec<(String, String)>,
}

impl BugReport {
    /// The report as the bytes of one file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let header = ReportHeader {
            kind: self.kind.clone(),
            description: self.description.clone(),
            area: self.area,
            elements: self.elements.clone(),
            errors: self.errors.clone(),
            details: self.details.clone(),
        };
        let header = serde_json::to_vec(&header).unwrap_or_default();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(REPORT_MAGIC);
        bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&header);
        if let Some(film) = &self.film {
            bytes.extend_from_slice(&film.to_bytes());
        }
        bytes
    }

    /// Reads a report back from the bytes of a file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MovieError> {
        let damaged = |why: &str| MovieError::Damaged(why.to_owned());
        let rest =
            bytes.strip_prefix(REPORT_MAGIC).ok_or_else(|| damaged("it does not start like a report"))?;
        let (length, rest) = rest.split_first_chunk::<8>().ok_or_else(|| damaged("no header"))?;
        let length = usize::try_from(u64::from_le_bytes(*length)).map_err(|_| damaged("header too long"))?;
        let header = rest.get(..length).ok_or_else(|| damaged("the header was cut"))?;
        let header: ReportHeader =
            serde_json::from_slice(header).map_err(|e| MovieError::Damaged(e.to_string()))?;
        let rest = &rest[length..];
        let film = if rest.is_empty() { None } else { Some(Movie::from_bytes(rest)?) };
        Ok(Self {
            kind: header.kind,
            description: header.description,
            area: header.area,
            elements: header.elements,
            errors: header.errors,
            film,
            details: header.details,
        })
    }
}

/// The words a [`BugReporter`] shows. English by default.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct BugLabels {
    /// The dialog's title.
    pub title: String,
    /// What the dialog is about.
    pub intro: String,
    /// The field for what happened.
    pub description: String,
    /// Includes the film.
    pub include_film: String,
    /// Warns that the film shows what was on screen.
    pub film_warning: String,
    /// Opens the editor to cover what should not show.
    pub edit_film: String,
    /// Goes back from the editor.
    pub done_editing: String,
    /// Sends it.
    pub send: String,
    /// Gives up.
    pub cancel: String,
    /// Names the choice of [`Self::kinds`].
    pub kind: String,
    /// The kinds of report to choose from. The first starts chosen.
    pub kinds: Vec<String>,
    /// The button that starts marking the problem on screen.
    pub mark: String,
    /// The same button, when a box was already drawn.
    pub mark_again: String,
    /// What to do while marking.
    pub mark_hint: String,
    /// Heads what goes along with the report.
    pub goes_along: String,
    /// Before the elements under the box.
    pub elements: String,
    /// Before the app's last errors.
    pub errors: String,
    /// The film player's words.
    pub player: MovieLabels,
    /// The film editor's words.
    pub editor: EditorLabels,
}

impl Default for BugLabels {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

impl BugLabels {
    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            title: texts.get("movie.bug.title"),
            intro: texts.get("movie.bug.intro"),
            description: texts.get("movie.bug.description"),
            include_film: texts.get("movie.bug.include_film"),
            film_warning: texts.get("movie.bug.film_warning"),
            edit_film: texts.get("movie.bug.edit_film"),
            done_editing: texts.get("movie.bug.done_editing"),
            send: texts.get("movie.bug.send"),
            cancel: texts.get("movie.bug.cancel"),
            kind: texts.get("movie.bug.kind"),
            kinds: texts.list("movie.bug.kinds"),
            mark: texts.get("movie.bug.mark"),
            mark_again: texts.get("movie.bug.mark_again"),
            mark_hint: texts.get("movie.bug.mark_hint"),
            goes_along: texts.get("movie.bug.goes_along"),
            elements: texts.get("movie.bug.elements"),
            errors: texts.get("movie.bug.errors"),
            player: MovieLabels::from_texts(texts),
            editor: EditorLabels::from_texts(texts),
        }
    }

    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }
}

/// A report being written.
#[derive(Clone)]
struct Draft {
    editor: Option<EditorState>,
    /// What lights the marked box in the film, to replace when it is marked again.
    light: Option<u64>,
    described: Option<AccessTree>,
    elements: Vec<String>,
}

/// The dialog that turns what the recorder has into a [`BugReport`].
///
/// The film is taken when the dialog opens, so it does not show the report being
/// written. The person watches it, can leave it out, and can cover or cut parts
/// of it in the editor before sending. They can also draw a box around the problem,
/// which names what is under it and lights it in the film, and everything else that
/// goes along is listed before they send. Where the report goes is up to the app.
#[derive(Clone)]
pub struct BugReporter {
    recorder: FlightRecorder,
    open: State<bool>,
    editing: State<bool>,
    include: State<bool>,
    description: State<String>,
    kind: State<usize>,
    marking: State<bool>,
    marked: State<Option<Rect>>,
    drawing: State<Option<Rect>>,
    asked: Rc<Cell<bool>>,
    draft: Rc<RefCell<Draft>>,
    labels: Rc<BugLabels>,
    details: Rc<dyn Fn() -> Vec<(String, String)>>,
    errors: Rc<dyn Fn() -> Vec<String>>,
    on_send: Rc<dyn Fn(BugReport)>,
}

impl std::fmt::Debug for BugReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BugReporter").field("recorder", &self.recorder).finish_non_exhaustive()
    }
}

impl BugReporter {
    /// A reporter over what `recorder` keeps; `on_send` gets the finished report.
    pub fn new(recorder: FlightRecorder, on_send: impl Fn(BugReport) + 'static) -> Self {
        Self {
            recorder,
            open: State::new(false),
            editing: State::new(false),
            include: State::new(true),
            description: State::new(String::new()),
            kind: State::new(0),
            marking: State::new(false),
            marked: State::new(None),
            drawing: State::new(None),
            asked: Rc::default(),
            draft: Rc::new(RefCell::new(Draft {
                editor: None,
                light: None,
                described: None,
                elements: Vec::new(),
            })),
            labels: Rc::new(BugLabels::default()),
            details: Rc::new(Vec::new),
            errors: Rc::new(Vec::new),
            on_send: Rc::new(on_send),
        }
    }

    /// The words it shows.
    pub fn labels(mut self, labels: BugLabels) -> Self {
        self.labels = Rc::new(labels);
        self
    }

    /// What the app adds to every report, such as its version.
    pub fn details(mut self, details: impl Fn() -> Vec<(String, String)> + 'static) -> Self {
        self.details = Rc::new(details);
        self
    }

    /// The app's last errors, such as [`RecentErrors::lines`](crate::RecentErrors::lines).
    /// The person sees them before anything goes.
    pub fn errors(mut self, errors: impl Fn() -> Vec<String> + 'static) -> Self {
        self.errors = Rc::new(errors);
        self
    }

    /// Opens the dialog, with the film up to now.
    pub fn open(&self) {
        *self.draft.borrow_mut() = Draft {
            editor: self.recorder.film().map(EditorState::new),
            light: None,
            described: None,
            elements: Vec::new(),
        };
        self.recorder.set_paused(true);
        self.description.set(String::new());
        self.kind.set(0);
        self.marking.set(false);
        self.marked.set(None);
        self.drawing.set(None);
        self.include.set(true);
        self.editing.set(false);
        self.open.set(true);
    }

    fn close(&self) {
        self.open.set(false);
        self.editing.set(false);
        self.marking.set(false);
        self.draft.borrow_mut().editor = None;
        self.recorder.set_paused(false);
    }

    /// Hides the dialog to mark the page behind it.
    fn start_marking(&self) {
        self.draft.borrow_mut().described = None;
        self.asked.set(false);
        self.drawing.set(None);
        self.marking.set(true);
    }

    /// Keeps the box drawn, names what is under it and lights it in the film.
    fn finish_marking(&self, area: Rect) {
        let region = Region::new(area.x(), area.y(), area.width(), area.height());
        let mut draft = self.draft.borrow_mut();
        draft.elements = draft.described.as_ref().map(|tree| named_under(tree, area)).unwrap_or_default();
        if let Some(editor) = draft.editor.clone() {
            draft.light = Some(editor.light_at_end(region, draft.light));
        }
        self.marked.set(Some(area));
        self.marking.set(false);
    }

    fn send(&self) {
        let film = if self.include.get() {
            self.draft.borrow().editor.as_ref().map(EditorState::movie)
        } else {
            None
        };
        let kind = self.labels.kinds.get(self.kind.get()).cloned().unwrap_or_default();
        let area = self.marked.get().map(|a| Region::new(a.x(), a.y(), a.width(), a.height()));
        (self.on_send)(BugReport {
            kind,
            description: self.description.get(),
            area,
            elements: self.draft.borrow().elements.clone(),
            errors: (self.errors)(),
            film,
            details: (self.details)(),
        });
        self.close();
    }
}

impl View for BugReporter {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if !self.open.get() {
            return cx.empty();
        }
        if self.marking.get() {
            if let Some(tree) = self.recorder.take_described() {
                self.draft.borrow_mut().described = Some(tree);
            }
            // The layer below hides the page from this description, so it comes a frame
            // earlier. An app without a recorder never gets one, and marks all the same.
            if self.draft.borrow().described.is_none() && !self.asked.replace(true) {
                cx.ask_for_access_tree();
                cx.wake_at(cx.now());
                return cx.empty();
            }
            let anchor = cx.empty();
            let leave = self.marking.clone();
            cx.overlay(
                anchor,
                Placement::Fill,
                &MarkingLayer(self.clone()),
                Some(Rc::new(move || leave.set(false))),
            );
            return anchor;
        }
        let labels = &self.labels;
        let viewport = cx.viewport();
        let (closing, read_open, cancel) = (self.clone(), self.open.clone(), self.clone());
        let open = Binding::new(
            move || read_open.get(),
            move |open: bool| {
                if !open {
                    closing.close();
                }
            },
        );
        let editor = self.draft.borrow().editor.clone();
        if self.editing.get()
            && let Some(state) = editor
        {
            let area = Size::new(viewport.width - 120.0, viewport.height - 200.0);
            let back = self.editing.clone();
            let dialog = Dialog::new(open, labels.edit_film.clone())
                .width(area.width + 48.0)
                .content(
                    MovieEditor::new(state)
                        .labels(labels.editor.clone())
                        .area(area)
                        .frame(Some(area.width), Some(area.height)),
                )
                .action(Button::new(labels.done_editing.clone()).on_click(move || back.set(false)));
            return cx.child(&dialog);
        }
        let include = self.include.get();
        let has_film = editor.is_some();
        let film = editor.filter(|_| include).map(|state| {
            let edit = self.editing.clone();
            VStack::new((
                Text::new(labels.film_warning.clone())
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                MoviePlayer::new(state.movie())
                    .labels(labels.player.clone())
                    .width(PREVIEW_WIDTH.min(viewport.width - 120.0)),
                Button::new(labels.edit_film.clone())
                    .icon(IconName::EyeOff)
                    .style(ButtonStyle::Secondary)
                    .on_click(move || edit.set(true)),
            ))
            .spacing(10.0)
            .align(Align::Start)
        });
        let send = self.clone();
        let written = !self.description.get().trim().is_empty();
        let mark = self.clone();
        let marked = self.marked.get().is_some();
        let (elements, errors) = (self.draft.borrow().elements.clone(), (self.errors)());
        let details = (self.details)();
        let along = (!elements.is_empty() || !errors.is_empty() || !details.is_empty()).then(|| {
            let mut lines: Vec<ravenrs_widgets::AnyView> =
                vec![Box::new(Text::new(labels.goes_along.clone()).style(TextStyle::Callout))];
            let mut line = |text: String| {
                lines
                    .push(Box::new(Text::new(text).style(TextStyle::Caption).foreground(Semantic::InkMuted)));
            };
            details.iter().for_each(|(name, value)| line(format!("{name}: {value}")));
            if !elements.is_empty() {
                line(format!("{}: {}", labels.elements, elements.join(", ")));
            }
            errors.iter().for_each(|error| line(format!("{}: {error}", labels.errors)));
            VStack::new(lines).spacing(4.0).align(Align::Stretch)
        });
        let dialog = Dialog::new(open, labels.title.clone())
            .description(labels.intro.clone())
            .width(PREVIEW_WIDTH + 64.0)
            .content(
                ravenrs_widgets::ScrollView::new((VStack::new((
                    Text::new(labels.kind.clone()).style(TextStyle::Callout),
                    SegmentedControl::new(self.kind.binding(), labels.kinds.iter().map(String::as_str))
                        .label(labels.kind.clone()),
                    Text::new(labels.description.clone()).style(TextStyle::Callout),
                    TextField::new(self.description.binding()),
                    Button::new(if marked { labels.mark_again.clone() } else { labels.mark.clone() })
                        .icon(IconName::Crosshair)
                        .style(ButtonStyle::Secondary)
                        .on_click(move || mark.start_marking()),
                    has_film.then(|| Toggle::new(self.include.binding()).label(labels.include_film.clone())),
                    along,
                    film,
                ))
                .spacing(10.0)
                .align(Align::Stretch),))
                .frame(None, Some((viewport.height - 260.0).max(200.0))),
            )
            .action(
                Button::new(labels.cancel.clone())
                    .style(ButtonStyle::Secondary)
                    .on_click(move || cancel.close()),
            )
            .action(Button::new(labels.send.clone()).disabled(!written).on_click(move || send.send()));
        cx.child(&dialog)
    }
}

/// The names on screen under `area`, in reading order and once each. What is more
/// than half outside it is left out, or the panels around it would always count.
fn named_under(tree: &AccessTree, area: Rect) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for node in &tree.nodes {
        let Some(name) = node.name.as_deref().map(str::trim).filter(|name| !name.is_empty()) else {
            continue;
        };
        let bounds = node.bounds;
        let inside = bounds.intersect(&area);
        let is_under = inside.width() * inside.height() * 2.0 >= bounds.width() * bounds.height()
            && bounds.width() * bounds.height() > 0.0;
        if !is_under || node.role == Role::Window {
            continue;
        }
        if !names.iter().any(|known| known == name) {
            names.push(name.to_owned());
        }
        if names.len() == MARKED_ELEMENTS {
            break;
        }
    }
    names
}

/// Covers the window while someone draws a box around the problem.
struct MarkingLayer(BugReporter);

impl View for MarkingLayer {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let theme = *cx.theme();
        let viewport = cx.viewport();
        let reporter = &self.0;
        let leave = reporter.marking.clone();
        let hint = cx.child(
            &VStack::new((
                Text::new(reporter.labels.mark_hint.clone()),
                Button::new(reporter.labels.cancel.clone())
                    .style(ButtonStyle::Secondary)
                    .on_click(move || leave.set(false)),
            ))
            .spacing(theme.spacing.s2)
            .align(Align::Center)
            .padding(theme.spacing.s3)
            .background(Semantic::Surface)
            .corner_radius(theme.radii.md),
        );
        let mut children = vec![hint];
        if let Some(area) = reporter.drawing.get() {
            let color = theme.color(Semantic::FocusRing);
            let box_node = cx.container(
                LayoutStyle {
                    absolute: Some((area.x(), area.y())),
                    width: Some(area.width()),
                    height: Some(area.height()),
                    ..LayoutStyle::stack(Axis::Vertical)
                },
                &[],
                Paint::Quad(QuadPaint {
                    fill: Color::TRANSPARENT.into(),
                    corner_radius: 0.0,
                    border: Some(Border { width: 2.0, color }),
                    shadow: None,
                }),
            );
            cx.semantics(box_node, Semantics::hidden());
            children.push(box_node);
        }
        let mut dim = theme.color(Semantic::Scrim);
        dim.a = MARKING_DIM;
        let node = cx.container(
            LayoutStyle {
                width: Some(viewport.width),
                height: Some(viewport.height),
                padding: theme.spacing.s4.into(),
                align: Align::Center,
                ..LayoutStyle::stack(Axis::Vertical)
            },
            &children,
            Paint::Quad(QuadPaint::new(dim)),
        );
        cx.cursor(node, CursorIcon::Default);
        let drawing = reporter.clone();
        let from = Rc::new(Cell::new(Point::ZERO));
        let from = cx.use_state(move || from).get();
        cx.on_drag(
            node,
            Rc::new(move |drag: Drag| {
                if drag.phase == DragPhase::Start {
                    from.set(drag.position);
                }
                let start = from.get();
                let (left, top) = (start.x.min(drag.position.x), start.y.min(drag.position.y));
                let area = Rect::from_origin_size(
                    Point::new(left, top),
                    Size::new((start.x - drag.position.x).abs(), (start.y - drag.position.y).abs()),
                );
                if drag.phase != DragPhase::End {
                    drawing.drawing.set(Some(area));
                } else if area.width() >= SMALLEST_MARK && area.height() >= SMALLEST_MARK {
                    drawing.finish_marking(area);
                } else {
                    drawing.drawing.set(None);
                }
            }),
        );
        node
    }
}
