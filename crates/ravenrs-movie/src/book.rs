//! The stories of the film components, for RavenComponentBook (feature `book`): see
//! `ravenrs_widgets::book`.

use ravenrs_core::{Color, Primitive, Quad, Rect, Size};
use ravenrs_widgets::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets::{AnyView, Text};

use crate::{FilmBuilder, Movie, MovieAssistant, MovieLibrary, MoviePlayer};

/// A square going across a white page, as the gallery films it; real films come from `Recorder`.
fn film(seconds: u64, chapters: &[(&str, &str, u64)]) -> Option<Movie> {
    // ravenrs-lint: allow(raw_color) the film is a recording, and its white page is part of the picture
    let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
    for second in 0..=seconds {
        let step = 240.0 / seconds.max(1) as f32;
        // ravenrs-lint: allow(raw_color) the square is drawn into the film, not into the interface
        let square =
            Quad::new(Rect::new(20.0 + second as f32 * step, 80.0, 40.0, 40.0), Color::hex(0x4f46e5));
        film.frame(second * 1000, &[Primitive::Quad(square.radius(8.0))]);
    }
    for (title, text, at) in chapters {
        film.chapter(*title, *text, *at);
    }
    film.finish(seconds * 1000).ok()
}

fn player(args: &Args) -> AnyView {
    match film(6, &[("A square", "It goes across the screen.", 0)]) {
        Some(film) => Box::new(MoviePlayer::new(film).width(args.number("width") as f32)),
        None => Box::new(Text::new("No film")),
    }
}

fn assistant(args: &Args) -> AnyView {
    let chapters = [
        ("Move a square", "How it goes across the screen.", 0),
        ("Save your work", "Keep a copy of what you did.", 5000),
    ];
    match film(9, &chapters) {
        Some(film) => Box::new(
            MovieAssistant::new(MovieLibrary::new().film("Manual", film)).width(args.number("width") as f32),
        ),
        None => Box::new(Text::new("No film")),
    }
}

impl ComponentBook for MoviePlayer {
    fn book() -> Meta {
        Meta::new("Application/MoviePlayer", player)
            .summary("Plays a film of the app, drawn as vectors, with chapters, captions and the sound it computes.")
            .arg(ArgType::number("width", 640.0, 320.0, 760.0, 10.0))
            .story("A film", Args::new())
            .story("Narrow", Args::new().with("width", 400.0))
    }
}

impl ComponentBook for MovieAssistant {
    fn book() -> Meta {
        Meta::new("Application/MovieAssistant", assistant)
            .summary("Finds the part of a film that answers a question, by its chapters, and plays it.")
            .arg(ArgType::number("width", 640.0, 320.0, 760.0, 10.0))
            .story("A library", Args::new())
    }
}

/// The stories of this crate, in the order the book lists them.
pub fn stories() -> Vec<Meta> {
    vec![MoviePlayer::book(), MovieAssistant::book()]
}
