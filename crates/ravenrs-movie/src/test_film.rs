//! [`TestFilm`]: a film of an end-to-end test, made from what its `ravenrs-e2e` session draws
//! and does (6.4 of #627).
//!
//! The film goes at a test's pace: every frame the session draws takes a thirtieth of a second,
//! the pointer jumps where it goes, and each step the test marks becomes a chapter. Nothing in
//! it comes from the wall clock, so two runs of the same test give the same bytes, and a film of
//! a failure shows what happened as it happened. A protected field is drawn as the session draws
//! it, hidden, so its value never reaches the film.
//!
//! ```no_run
//! use ravenrs_e2e::E2e;
//! use ravenrs_movie::TestFilm;
//! # fn app() -> ravenrs_widgets::Text { ravenrs_widgets::Text::new("") }
//! let mut e2e = E2e::new(app(), ravenrs_style::Theme::raven_light());
//! let film = TestFilm::start(&mut e2e);
//! e2e.mark_step("Salvar");
//! let _ = e2e.by_test_id("salvar").click();
//! let movie = film.finish(&mut e2e).unwrap();
//! std::fs::write("cenario.ravenmovie", movie.to_bytes()).unwrap();
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{Point, Primitive};
use ravenrs_e2e::{E2e, FrameObserver};

use crate::movie::{FilmBuilder, Movie, MovieError};

/// How long each frame lasts in the film, in milliseconds: thirty frames a second.
const FRAME_MS: u64 = 33;
/// How long the film holds on its last frame, so a player shows how the test ended.
const HOLD_MS: u64 = 1000;

#[derive(Debug)]
struct Take {
    film: FilmBuilder,
    at_ms: u64,
}

impl Take {
    fn tick(&mut self) {
        self.at_ms += FRAME_MS;
    }
}

/// Watches a session and makes a film of it.
struct Watch(Rc<RefCell<Take>>);

impl FrameObserver for Watch {
    fn frame(&mut self, primitives: &[Primitive]) {
        let mut take = self.0.borrow_mut();
        let at = take.at_ms;
        take.film.frame(at, primitives);
        take.tick();
    }

    fn pointer(&mut self, at: Point) {
        let mut take = self.0.borrow_mut();
        let ms = take.at_ms;
        take.film.pointer(ms, at);
    }

    fn click(&mut self, at: Point) {
        let mut take = self.0.borrow_mut();
        let ms = take.at_ms;
        take.film.click(ms, at);
    }

    fn key(&mut self) {
        let mut take = self.0.borrow_mut();
        let ms = take.at_ms;
        take.film.key(ms);
    }

    fn step(&mut self, title: &str) {
        let mut take = self.0.borrow_mut();
        let ms = take.at_ms;
        take.film.chapter(title, "", ms);
    }
}

/// A film of an [`E2e`] session being made. See the [module](self).
#[derive(Debug)]
pub struct TestFilm {
    take: Rc<RefCell<Take>>,
}

impl TestFilm {
    /// Starts filming `e2e`, from the frame on its screen.
    pub fn start(e2e: &mut E2e) -> Self {
        let take =
            Rc::new(RefCell::new(Take { film: FilmBuilder::new(e2e.size(), e2e.background()), at_ms: 0 }));
        e2e.observe(Box::new(Watch(take.clone())));
        Self { take }
    }

    /// Stops filming `e2e` and gives the film, which holds a second on how the test ended.
    pub fn finish(self, e2e: &mut E2e) -> Result<Movie, MovieError> {
        drop(e2e.stop_observing());
        let take = Rc::try_unwrap(self.take)
            .map_or_else(|shared| shared.replace(Take::empty()), RefCell::into_inner);
        let end = take.at_ms + HOLD_MS;
        take.film.finish(end)
    }
}

impl Take {
    fn empty() -> Self {
        Self { film: FilmBuilder::new(ravenrs_core::Size::ZERO, ravenrs_core::Color::TRANSPARENT), at_ms: 0 }
    }
}

/// Films an app's own session over the test channel, for [`ravenrs_e2e_wire::serve_filmed`]:
/// `Film { path }` starts a [`TestFilm`], and `FilmStop` writes it to that path.
#[derive(Debug, Default)]
pub struct ChannelFilm {
    taking: Option<(TestFilm, std::path::PathBuf)>,
}

impl ravenrs_e2e_wire::Filming for ChannelFilm {
    fn start(&mut self, e2e: &mut E2e, path: &std::path::Path) {
        if let Some((film, _)) = self.taking.take() {
            drop(film.finish(e2e));
        }
        self.taking = Some((TestFilm::start(e2e), path.to_path_buf()));
    }

    fn stop(&mut self, e2e: &mut E2e) -> Result<std::path::PathBuf, String> {
        let (film, path) = self.taking.take().ok_or("nada está sendo filmado")?;
        let movie = film.finish(e2e).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        std::fs::write(&path, movie.to_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }
}
