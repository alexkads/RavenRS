//! The last minutes on screen, always at hand, and a bug report made from them.
//!
//! Nothing here leaves the computer by itself. What is recorded stays in memory
//! until someone opens a report, sees the film, covers what should not show and
//! chooses to send it, through whatever the app does with [`BugReport`].

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{Point, Primitive, Size};
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    BuildCx, Button, ButtonStyle, Dialog, IconName, NodeId, Text, TextField, Toggle, UiActivity, VStack,
    View, ViewExt,
};
use serde::{Deserialize, Serialize};

use crate::editor::{EditorLabels, EditorState, MovieEditor};
use crate::movie::{FilmBuilder, Movie, MovieError};
use crate::player::{MovieLabels, MoviePlayer};

const REPORT_MAGIC: &[u8] = b"RAVENBUG1\n";
/// The fewest seconds between two frames it keeps: a report does not need more,
/// and the app it watches should not pay for more.
const SHORTEST_GAP: f64 = 1.0 / 15.0;
const PREVIEW_WIDTH: f32 = 640.0;

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
            })),
        }
    }

    /// The film so far, to look at, edit or send. Recording goes on.
    pub fn film(&self) -> Option<Movie> {
        self.flight.borrow().film()
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
                _ => {}
            }
        }));
        cx.empty()
    }
}

/// What someone says about a bug, for the development team.
#[derive(Clone, Debug)]
pub struct BugReport {
    /// What happened, in their words.
    pub description: String,
    /// The film of what led to it, if they chose to include it.
    pub film: Option<Movie>,
    /// What the app adds, such as its version. Shown before it is sent.
    pub details: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize)]
struct ReportHeader {
    description: String,
    details: Vec<(String, String)>,
}

impl BugReport {
    /// The report as the bytes of one file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let header = ReportHeader { description: self.description.clone(), details: self.details.clone() };
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
        Ok(Self { description: header.description, film, details: header.details })
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
    /// The film player's words.
    pub player: MovieLabels,
    /// The film editor's words.
    pub editor: EditorLabels,
}

impl Default for BugLabels {
    fn default() -> Self {
        Self {
            title: "Report a problem".into(),
            intro: "Tell the team what happened. You see everything before it goes.".into(),
            description: "What happened?".into(),
            include_film: "Include a film of the last minutes".into(),
            film_warning: "The film shows what was on your screen. Watch it, and cover what should not go."
                .into(),
            edit_film: "Cover or cut parts of the film".into(),
            done_editing: "Done".into(),
            send: "Send".into(),
            cancel: "Cancel".into(),
            player: MovieLabels::default(),
            editor: EditorLabels::default(),
        }
    }
}

impl BugLabels {
    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            title: "Relatar um problema".into(),
            intro: "Conte à equipe o que aconteceu. Você vê tudo antes de sair daqui.".into(),
            description: "O que aconteceu?".into(),
            include_film: "Incluir um filme dos últimos minutos".into(),
            film_warning: "O filme mostra o que estava na sua tela. Assista e cubra o que não deve ir."
                .into(),
            edit_film: "Cobrir ou cortar partes do filme".into(),
            done_editing: "Concluir".into(),
            send: "Enviar".into(),
            cancel: "Cancelar".into(),
            player: MovieLabels::pt_br(),
            editor: EditorLabels::pt_br(),
        }
    }
}

/// A report being written.
#[derive(Clone)]
struct Draft {
    editor: Option<EditorState>,
}

/// The dialog that turns what the recorder has into a [`BugReport`].
///
/// The film is taken when the dialog opens, so it does not show the report being
/// written. The person watches it, can leave it out, and can cover or cut parts
/// of it in the editor before sending. Where the report goes is up to the app.
#[derive(Clone)]
pub struct BugReporter {
    recorder: FlightRecorder,
    open: State<bool>,
    editing: State<bool>,
    include: State<bool>,
    description: State<String>,
    draft: Rc<RefCell<Draft>>,
    labels: Rc<BugLabels>,
    details: Rc<dyn Fn() -> Vec<(String, String)>>,
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
            draft: Rc::new(RefCell::new(Draft { editor: None })),
            labels: Rc::new(BugLabels::default()),
            details: Rc::new(Vec::new),
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

    /// Opens the dialog, with the film up to now.
    pub fn open(&self) {
        self.draft.borrow_mut().editor = self.recorder.film().map(EditorState::new);
        self.recorder.set_paused(true);
        self.description.set(String::new());
        self.include.set(true);
        self.editing.set(false);
        self.open.set(true);
    }

    fn close(&self) {
        self.open.set(false);
        self.editing.set(false);
        self.draft.borrow_mut().editor = None;
        self.recorder.set_paused(false);
    }

    fn send(&self) {
        let film = if self.include.get() {
            self.draft.borrow().editor.as_ref().map(EditorState::movie)
        } else {
            None
        };
        (self.on_send)(BugReport { description: self.description.get(), film, details: (self.details)() });
        self.close();
    }
}

impl View for BugReporter {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        if !self.open.get() {
            return cx.empty();
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
        let dialog = Dialog::new(open, labels.title.clone())
            .description(labels.intro.clone())
            .width(PREVIEW_WIDTH + 64.0)
            .content(
                ravenrs_widgets::ScrollView::new((VStack::new((
                    Text::new(labels.description.clone()).style(TextStyle::Callout),
                    TextField::new(self.description.binding()),
                    has_film.then(|| Toggle::new(self.include.binding()).label(labels.include_film.clone())),
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
