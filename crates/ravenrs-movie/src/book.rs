//! The stories of the film components, for RavenComponentBook (feature `book`): see
//! `ravenrs_widgets::book`.

use ravenrs_core::{Color, Primitive, Quad, Rect, Size};
use ravenrs_widgets::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets::{AnyView, IconName, StatusBar, Text, VStack};

use crate::{
    BugLabels, BugReporter, FilmBuilder, FlightRecorder, Movie, MovieAssistant, MovieLibrary, MoviePlayer,
};

/// The languages a report can be written in, as `BugLabels` has them.
const LANGUAGES: [&str; 2] = ["pt-BR", "en"];
/// The kinds of report, in the order of `BugLabels::kinds`.
const KINDS: [&str; 3] = ["Something is wrong", "It is confusing", "A suggestion"];

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

/// The report dialog as an app has it: a status bar whose action opens it, as in Raven
/// Finanças, and the dialog itself, open or not, written as far as the story says.
fn reporter(args: &Args) -> AnyView {
    let english = args.choice("language") == "en";
    let labels = if english { BugLabels::default() } else { BugLabels::pt_br() };
    let with_errors = args.bool("errors");
    let errors = move || match (with_errors, english) {
        (false, _) => Vec::new(),
        (true, true) => vec![
            "WARN sync: the server took 8.2 s to answer".to_owned(),
            "ERROR preview: could not read IMG_0412.CR3".to_owned(),
        ],
        (true, false) => vec![
            "WARN sincronia: o servidor levou 8,2 s para responder".to_owned(),
            "ERROR prévia: não deu para ler IMG_0412.CR3".to_owned(),
        ],
    };
    let details = move || {
        let (version, system) = if english { ("version", "system") } else { ("versão", "sistema") };
        vec![(version.to_owned(), "0.1.32".to_owned()), (system.to_owned(), "macos".to_owned())]
    };
    let reporter =
        BugReporter::new(FlightRecorder::new(240.0), |_| {}).labels(labels).details(details).errors(errors);

    let kind = KINDS.iter().position(|k| *k == args.choice("kind")).unwrap_or(0);
    let include = args.bool("include_film");
    let marked = args.bool("marked").then(|| {
        let names = if english { ["Save button", "Amount field"] } else { ["Botão Salvar", "Campo Valor"] };
        (Rect::new(40.0, 60.0, 180.0, 64.0), names.iter().map(|n| (*n).to_owned()).collect())
    });
    let show = {
        let reporter = reporter.clone();
        move || reporter.open_as_story(film(6, &[]), kind, include, marked.clone())
    };
    if args.bool("open") {
        show();
    }
    let action = if english { "Report a problem" } else { "Relatar problema" };
    let bar = StatusBar::new().right((StatusBar::action(action).icon(IconName::CircleAlert).on_click(show),));
    Box::new(VStack::new((bar, reporter)).spacing(0.0))
}

impl ComponentBook for BugReporter {
    fn book() -> Meta {
        Meta::new("Application/BugReporter", reporter)
            .summary(
                "The dialog that turns the last minutes on screen into a bug report: its kind, \
                 what happened, a box over the problem, the film to watch and cover, and all that goes along.",
            )
            .arg(ArgType::choice("language", &LANGUAGES, "pt-BR").description("The words, from `BugLabels`."))
            .arg(ArgType::choice("kind", &KINDS, "Something is wrong").description("What kind of report it is."))
            .arg(ArgType::bool("open", true).description("Open, or only the status bar action that opens it."))
            .arg(ArgType::bool("include_film", true).description("The film of the last minutes goes along."))
            .arg(ArgType::bool("marked", false).description("A box already drawn around the problem."))
            .arg(ArgType::bool("errors", false).description("The app's last errors, from `RecentErrors`."))
            .story("Describing", Args::new())
            .story("It is confusing", Args::new().with("kind", "It is confusing"))
            .story("A suggestion", Args::new().with("kind", "A suggestion"))
            .story("Box marked", Args::new().with("marked", true))
            .story("Without the film", Args::new().with("include_film", false))
            .story("With errors", Args::new().with("errors", true))
            .story("English", Args::new().with("language", "en"))
            .story("Closed", Args::new().with("open", false))
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
    vec![MoviePlayer::book(), MovieAssistant::book(), BugReporter::book()]
}
