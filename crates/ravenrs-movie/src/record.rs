//! Records a film by running the real app from a script, with no window.

use ravenrs_core::{AccessNode, Key, Modifiers, Point, Role, Size};
use ravenrs_e2e::{E2e, Query};
use ravenrs_style::Theme;
use ravenrs_widgets::{Ui, View};

use crate::movie::{Hotspot, HotspotKind, Movie, MovieError};
use crate::narration::Narration;
use crate::session_film::{Pace, SessionFilm};

const FRAME: f64 = 1.0 / 30.0;
/// How fast the pointer travels, in logical pixels a second, and its limits.
const POINTER_SPEED: f32 = 900.0;
const POINTER_TRAVEL: (f64, f64) = (0.25, 0.8);
const SECONDS_PER_LETTER: f64 = 0.06;
/// How long reading or hearing a text takes, at an easy pace.
const READING_PER_LETTER: f64 = 0.075;
const READING_LEAST: f64 = 2.0;
const READING_START: f64 = 0.8;
/// The breath after a recorded box, before the next thing happens.
const AFTER_SPEAKING: f64 = 0.6;

/// What can stop a recording.
#[derive(Debug)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum RecordError {
    /// The script names something that is not on the screen.
    NotOnScreen {
        /// What it looked for.
        name: String,
        /// What there was to find.
        available: Vec<String>,
    },
    /// The film could not be put together.
    Movie(MovieError),
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotOnScreen { name, available } => {
                write!(f, "'{name}' is not on the screen; there is {}", available.join(", "))
            }
            Self::Movie(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for RecordError {}

impl From<MovieError> for RecordError {
    fn from(e: MovieError) -> Self {
        Self::Movie(e)
    }
}

/// How long it takes to read or hear a box: the voice says its title too.
fn reading_seconds(title: &str, text: &str) -> f64 {
    let letters = title.chars().count() + text.chars().count();
    (READING_START + letters as f64 * READING_PER_LETTER).max(READING_LEAST)
}

/// Runs an app from a script and films it: the steps are the ones a person would
/// take, found by the names screen readers see. See the [crate documentation](crate).
#[derive(Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Recorder {
    /// The app, driven as a test drives it: the same session, finding and input as `ravenrs-e2e`.
    e2e: E2e,
    clock: f64,
    pointer: Point,
    logical: Size,
    /// The film, at a person's pace: the same writer a test films with.
    film: SessionFilm,
    narration: Option<Narration>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Recorder {
    /// Films `root` in a window of `size`.
    pub fn new(root: impl View + 'static, theme: Theme, size: Size) -> Self {
        let mut e2e = E2e::new(root, theme).viewport(size, 1.0);
        e2e.frame_at(0.0);
        let film = SessionFilm::start(&mut e2e, Pace::Person);
        let mut recorder = Self {
            e2e,
            clock: 0.0,
            pointer: Point::new(size.width / 2.0, size.height / 2.0),
            logical: size,
            film,
            narration: None,
        };
        recorder.tick();
        recorder
    }

    /// The recordings the player will say the boxes with. Each box then lasts as
    /// long as its recording, and a little more, so that the voice and the box
    /// end together and the film never ends while the voice is still speaking.
    /// A box with no recording lasts as long as reading it takes.
    #[must_use]
    pub fn narration(mut self, narration: Narration) -> Self {
        self.narration = Some(narration);
        self
    }

    /// How long a box stays: as long as its recording, or as reading it takes.
    /// The player says the title and then the text.
    fn speaking_seconds(&self, title: &str, text: &str) -> f64 {
        let said = format!("{title}. {text}");
        match self.narration.as_ref().and_then(|narration| narration.seconds(&said)) {
            Some(seconds) => (seconds + AFTER_SPEAKING).max(READING_LEAST),
            None => reading_seconds(title, text),
        }
    }

    fn milliseconds(&self) -> u64 {
        (self.clock * 1000.0) as u64
    }

    /// Advances the clock by one frame and films what the app draws.
    fn tick(&mut self) {
        self.clock += FRAME;
        self.film.at(self.milliseconds());
        self.e2e.frame_at(self.clock);
    }

    /// What is called `name`, found as a test finds it: `#id` by its test id, a text by the
    /// exact name, and, when that is not one thing, what a person would reach for: the one
    /// that takes input first, then a name that starts with it.
    fn find(&mut self, name: &str) -> Result<AccessNode, RecordError> {
        let not_found = |e2e: &E2e, ids: bool| RecordError::NotOnScreen {
            name: name.to_owned(),
            available: e2e
                .tree()
                .nodes
                .iter()
                .filter_map(|n| {
                    if ids {
                        n.test_id.as_ref().map(|t| format!("#{t}"))
                    } else {
                        n.name.clone().filter(|_| n.focusable)
                    }
                })
                .collect(),
        };
        if let Some(id) = name.strip_prefix('#') {
            return self.e2e.find(&Query::test_id(id.trim())).map_err(|_| not_found(&self.e2e, true));
        }
        let interactive = |n: &AccessNode| n.focusable || n.role == Role::Button || n.role == Role::TextInput;
        let exact = self.e2e.find_all(&Query::text(name)).unwrap_or_default();
        if let Some(found) = exact.iter().find(|n| interactive(n)).or_else(|| exact.first()) {
            return Ok(found.clone());
        }
        let starts = |n: &&AccessNode| n.name.as_deref().is_some_and(|text| text.starts_with(name));
        let nodes = &self.e2e.tree().nodes;
        nodes
            .iter()
            .filter(starts)
            .find(|n| interactive(n))
            .or_else(|| nodes.iter().find(starts))
            .cloned()
            .ok_or_else(|| not_found(&self.e2e, false))
    }

    /// Lets the film run with nothing happening.
    pub fn wait(&mut self, seconds: f64) -> &mut Self {
        let until = self.clock + seconds;
        while self.clock < until {
            self.tick();
        }
        self
    }

    /// Starts a chapter here.
    pub fn chapter(&mut self, title: &str, text: &str) -> &mut Self {
        self.film.chapter(title, text);
        self
    }

    /// Starts a chapter here and explains it in a box in the middle of the film,
    /// for as long as it takes to read or hear.
    pub fn intro(&mut self, title: &str, text: &str) -> &mut Self {
        self.chapter(title, text);
        let seconds = self.speaking_seconds(title, text);
        self.film.hotspot(Hotspot {
            kind: HotspotKind::Note,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            title: title.to_owned(),
            text: text.to_owned(),
            from_ms: self.milliseconds(),
            until_ms: self.milliseconds() + (seconds * 1000.0) as u64,
        });
        self.wait(seconds)
    }

    /// A [`Self::note`] that lasts as long as `text` takes to hear, or to read
    /// when it has no recording.
    pub fn explain(&mut self, name: &str, title: &str, text: &str) -> Result<&mut Self, RecordError> {
        let seconds = self.speaking_seconds(title, text);
        self.note(name, title, text, seconds)
    }

    /// Slides the pointer to the middle of what is called `name`, or of what has the test id
    /// after `#` (`#salvar`), as a test of `ravenrs-e2e` finds it.
    pub fn move_to(&mut self, name: &str) -> Result<&mut Self, RecordError> {
        let target = self.find(name)?.bounds.center();
        let start = self.pointer;
        let distance = (target.x - start.x).hypot(target.y - start.y);
        let travel = f64::from(distance / POINTER_SPEED).clamp(POINTER_TRAVEL.0, POINTER_TRAVEL.1);
        let begin = self.clock;
        self.e2e.pointer_to(start);
        while self.clock - begin < travel {
            let t = ((self.clock - begin) / travel) as f32;
            let progress = t * t * (3.0 - 2.0 * t);
            self.pointer = Point::new(
                start.x + (target.x - start.x) * progress,
                start.y + (target.y - start.y) * progress,
            );
            self.e2e.pointer_to(self.pointer);
            self.tick();
        }
        self.pointer = target;
        self.e2e.pointer_to(target);
        self.tick();
        Ok(self)
    }

    /// Slides the pointer to what is called `name` and clicks it.
    pub fn click(&mut self, name: &str) -> Result<&mut Self, RecordError> {
        self.move_to(name)?;
        let position = self.pointer;
        self.e2e.mouse_down(position);
        self.tick();
        self.e2e.mouse_up(position);
        Ok(self.wait(0.5))
    }

    /// Clicks the field called `name` and types `text` into it, letter by letter.
    pub fn type_into(&mut self, name: &str, text: &str) -> Result<&mut Self, RecordError> {
        self.click(name)?;
        // A click on a field in a dialog can land on its label: the keyboard has to get there.
        let id = self.find(name)?.id;
        self.e2e.focus(id);
        for character in text.chars() {
            self.press(Key::Character(character.to_string()));
            self.wait(SECONDS_PER_LETTER);
        }
        Ok(self.wait(0.3))
    }

    /// Presses a key.
    pub fn press(&mut self, key: Key) -> &mut Self {
        self.e2e.key_down(key, Modifiers::default());
        self.tick();
        self
    }

    /// Rolls the wheel over what is under the pointer, `pixels` in all; positive
    /// goes down the page.
    pub fn scroll(&mut self, pixels: f32) -> &mut Self {
        const STEPS: u16 = 15;
        for _ in 0..STEPS {
            let delta = Point::new(0.0, -pixels / f32::from(STEPS));
            self.e2e.scroll_at(self.pointer, delta);
            self.tick();
        }
        self.wait(0.3)
    }

    /// Marks what is called `name` and says `text` about it for `seconds`, in
    /// which the film goes on. Clicking the mark in the player brings the text back.
    pub fn note(
        &mut self,
        name: &str,
        title: &str,
        text: &str,
        seconds: f64,
    ) -> Result<&mut Self, RecordError> {
        let hotspot = self.hotspot(name, HotspotKind::Note, title, text, seconds)?;
        self.film.hotspot(hotspot);
        Ok(self.wait(seconds))
    }

    /// The film stops here, with what is called `name` marked and `text` saying
    /// what to do, until someone clicks the mark. Follow it with the step itself,
    /// such as [`Self::click`], which is what the film shows next. When the
    /// player is not stopping at steps, the box stays for as long as it takes to
    /// read or hear; when it is, that time is skipped.
    pub fn guide(&mut self, name: &str, title: &str, text: &str) -> Result<&mut Self, RecordError> {
        // The pointer gets there first: the step asks for what it is already on.
        self.move_to(name)?;
        let seconds = self.speaking_seconds(title, text);
        let hotspot = self.hotspot(name, HotspotKind::Stop, title, text, seconds)?;
        self.film.hotspot(hotspot);
        Ok(self.wait(seconds))
    }

    fn hotspot(
        &mut self,
        name: &str,
        kind: HotspotKind,
        title: &str,
        text: &str,
        seconds: f64,
    ) -> Result<Hotspot, RecordError> {
        let bounds = self.find(name)?.bounds;
        // A hair larger than what it marks, so the border does not cover it.
        const MARGIN: f32 = 3.0;
        Ok(Hotspot {
            kind,
            x: (bounds.x() - MARGIN).max(0.0),
            y: (bounds.y() - MARGIN).max(0.0),
            width: (bounds.width() + MARGIN * 2.0).min(self.logical.width),
            height: (bounds.height() + MARGIN * 2.0).min(self.logical.height),
            title: title.to_owned(),
            text: text.to_owned(),
            from_ms: self.milliseconds(),
            until_ms: self.milliseconds() + (seconds * 1000.0) as u64,
        })
    }

    /// The UI being filmed, for what a script cannot say by name.
    pub fn ui_mut(&mut self) -> &mut Ui {
        self.e2e.ui_mut()
    }

    /// The session being filmed, to reach a part as a test does.
    pub fn e2e_mut(&mut self) -> &mut E2e {
        &mut self.e2e
    }

    /// Ends the recording.
    pub fn finish(mut self) -> Result<Movie, RecordError> {
        let duration = self.milliseconds();
        Ok(self.film.finish(&mut self.e2e, duration)?)
    }
}
