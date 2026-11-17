//! [`SessionFilm`]: the one way a film is made from what an app draws and does (6.4 of #627),
//! whatever the film is for:
//!
//! - a **test**, watching its `ravenrs-e2e` session at a test's pace ([`SessionFilm::start`]),
//!   and over the test channel ([`ChannelFilm`]);
//! - a **training**, which the [`Recorder`](crate::Recorder) scripts on the same session at a
//!   person's pace, with its boxes and voice;
//! - a **bug report**, where the [`FlightRecorder`](crate::FlightRecorder) hands it what the
//!   running app paints, at the app's own clock ([`SessionFilm::new`]).
//!
//! - [`Pace::Test`]: every frame the session draws takes a thirtieth of a second, the pointer
//!   jumps, and each step the test marks becomes a chapter.
//! - [`Pace::Person`]: whoever scripts the session says the moment with [`SessionFilm::at`],
//!   and the frames, the gliding pointer, the clicks and the keys land there.
//!
//! Nothing comes from the wall clock, so two runs give the same bytes. A protected field is
//! drawn as the session draws it, hidden, so its value never reaches the film.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_movie::{Pace, SessionFilm};
//! # fn app() -> ravenrs_widgets::Text { ravenrs_widgets::Text::new("") }
//! let mut e2e = E2e::new(app(), ravenrs_style::Theme::raven_light());
//! let film = SessionFilm::start(&mut e2e, Pace::Test);
//! e2e.mark_step("Salvar");
//! let _ = e2e.by_test_id("salvar").click();
//! let movie = film.finish(&mut e2e, 0).unwrap();
//! std::fs::write("cenario.ravenmovie", movie.to_bytes()).unwrap();
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{Color, Point, Primitive, Size};
use ravenrs_e2e::{E2e, FrameObserver};

use crate::movie::{FilmBuilder, Hotspot, Movie, MovieError};

/// How long each frame lasts at a test's pace, in milliseconds: thirty frames a second.
const FRAME_MS: u64 = 33;
/// How long a test's film holds on its last frame, so a player shows how it ended.
const HOLD_MS: u64 = 1000;

/// How the film keeps time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    /// A test's: each frame drawn moves the film a thirtieth of a second, and the pointer jumps.
    Test,
    /// A person's: whoever scripts the session says the moment with [`SessionFilm::at`].
    Person,
}

#[derive(Debug)]
struct Take {
    film: FilmBuilder,
    at_ms: u64,
    pace: Pace,
}

/// Watches a session and puts what it sees in the film.
struct Watch(SessionFilm);

impl FrameObserver for Watch {
    fn frame(&mut self, primitives: &[Primitive]) {
        self.0.frame(primitives);
    }

    fn pointer(&mut self, at: Point) {
        self.0.pointer(at);
    }

    fn click(&mut self, at: Point) {
        self.0.click(at);
    }

    fn key(&mut self) {
        self.0.key();
    }

    fn step(&mut self, title: &str) {
        self.0.chapter(title, "");
    }
}

/// A film of an [`E2e`] session being made. See the [module](self).
#[derive(Debug)]
pub struct SessionFilm {
    take: Rc<RefCell<Take>>,
}

impl SessionFilm {
    /// A film of something `size` large over `background`, fed by hand with [`Self::frame`],
    /// [`Self::pointer`] and the rest: what a recorder of the running app does.
    pub fn new(size: Size, background: Color, pace: Pace) -> Self {
        Self {
            take: Rc::new(RefCell::new(Take { film: FilmBuilder::new(size, background), at_ms: 0, pace })),
        }
    }

    /// Starts filming `e2e` at `pace`, from the frame on its screen.
    pub fn start(e2e: &mut E2e, pace: Pace) -> Self {
        let film = Self::new(e2e.size(), e2e.background(), pace);
        e2e.observe(Box::new(Watch(Self { take: film.take.clone() })));
        film
    }

    /// Puts a frame of `primitives` here; at a test's pace, the film then moves a frame on.
    /// A frame like the last one costs nothing.
    pub fn frame(&self, primitives: &[Primitive]) {
        let mut take = self.take.borrow_mut();
        let at = take.at_ms;
        take.film.frame(at, primitives);
        if take.pace == Pace::Test {
            take.at_ms += FRAME_MS;
        }
    }

    /// Notes the pointer at `at`, here.
    pub fn pointer(&self, at: Point) {
        let mut take = self.take.borrow_mut();
        let ms = take.at_ms;
        take.film.pointer(ms, at);
    }

    /// Notes a click at `at`, here.
    pub fn click(&self, at: Point) {
        let mut take = self.take.borrow_mut();
        let ms = take.at_ms;
        take.film.click(ms, at);
    }

    /// Notes a key, here, for its sound.
    pub fn key(&self) {
        let mut take = self.take.borrow_mut();
        let ms = take.at_ms;
        take.film.key(ms);
    }

    /// The film so far, lasting at least `duration_ms`, without stopping it: a bug report
    /// takes it while the app goes on.
    pub fn movie(&self, duration_ms: u64) -> Result<Movie, MovieError> {
        let take = self.take.borrow();
        take.film.clone().finish(duration_ms.max(take.at_ms))
    }

    /// Where the film is, in milliseconds.
    pub fn now(&self) -> u64 {
        self.take.borrow().at_ms
    }

    /// Moves the film to `ms`, at a person's pace: what the session draws or does next lands
    /// there. The film never goes back.
    pub fn at(&self, ms: u64) {
        let mut take = self.take.borrow_mut();
        take.at_ms = take.at_ms.max(ms);
    }

    /// Starts a chapter here.
    pub fn chapter(&self, title: &str, text: &str) {
        let mut take = self.take.borrow_mut();
        let ms = take.at_ms;
        take.film.chapter(title, text, ms);
    }

    /// Marks a spot on the screen: a note, or a stop the player waits at.
    pub fn hotspot(&self, hotspot: Hotspot) {
        self.take.borrow_mut().film.hotspot(hotspot);
    }

    /// Stops filming `e2e` and gives the film, lasting at least `duration_ms`. A test's film
    /// holds a second on how the test ended.
    pub fn finish(self, e2e: &mut E2e, duration_ms: u64) -> Result<Movie, MovieError> {
        drop(e2e.stop_observing());
        let take = match Rc::try_unwrap(self.take) {
            Ok(take) => take.into_inner(),
            Err(shared) => shared.replace(Take {
                film: FilmBuilder::new(Size::ZERO, Color::TRANSPARENT),
                at_ms: 0,
                pace: Pace::Test,
            }),
        };
        let end = match take.pace {
            Pace::Test => take.at_ms + HOLD_MS,
            Pace::Person => take.at_ms,
        };
        take.film.finish(end.max(duration_ms))
    }
}

/// Films an app's own session over the test channel, at a test's pace, for
/// [`ravenrs_e2e_wire::serve_filmed`]: `Film { path }` starts a [`SessionFilm`], and `FilmStop`
/// writes it to that path.
#[derive(Debug, Default)]
pub struct ChannelFilm {
    taking: Option<(SessionFilm, std::path::PathBuf)>,
}

impl ravenrs_e2e_wire::Filming for ChannelFilm {
    fn frame(&mut self, e2e: &E2e) -> Result<Vec<u8>, String> {
        // Reuse the film writer without attaching an observer: an ongoing take keeps filming.
        let frame = SessionFilm::new(e2e.size(), e2e.background(), Pace::Person);
        frame.frame(e2e.primitives());
        frame.movie(1).map(|movie| movie.to_bytes()).map_err(|error| error.to_string())
    }

    fn start(&mut self, e2e: &mut E2e, path: &std::path::Path) {
        if let Some((film, _)) = self.taking.take() {
            drop(film.finish(e2e, 0));
        }
        self.taking = Some((SessionFilm::start(e2e, Pace::Test), path.to_path_buf()));
    }

    fn stop(&mut self, e2e: &mut E2e) -> Result<std::path::PathBuf, String> {
        let (film, path) = self.taking.take().ok_or("nada está sendo filmado")?;
        let movie = film.finish(e2e, 0).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        std::fs::write(&path, movie.to_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }
}
