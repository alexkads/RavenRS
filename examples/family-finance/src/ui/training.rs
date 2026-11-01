//! The training films, shipped inside the app, and reporting a problem.

use ravenrs::movie::{BugLabels, BugReporter, FlightRecorder, Movie, MovieLabels, MoviePlayer};
use ravenrs::prelude::*;

use super::store::{Store, heading};
use crate::update;

/// The films, recorded by `tests/training.rs` from the real app.
const FILM: &[u8] = include_bytes!("../../assets/treinamentos.ravenmovie");
/// The room around the player: the sidebar and the page's margins.
const AROUND_THE_PLAYER: f32 = 340.0;

/// The film, read once: it is small, but views build often.
pub(crate) struct TrainingState {
    film: Option<Movie>,
}

impl TrainingState {
    pub(crate) fn new() -> Self {
        Self { film: Movie::from_bytes(FILM).ok() }
    }
}

/// The page with the films.
pub(crate) fn training(state: &TrainingState) -> impl View + use<> {
    let film = state.film.clone();
    ScrollView::new((VStack::new((
        heading(
            "Treinamentos",
            "Assista no seu ritmo. Em cada passo o filme para e espera você clicar no ponto marcado.",
        ),
        responsive(move |size| -> AnyView {
            match &film {
                Some(film) => Box::new(
                    MoviePlayer::new(film.clone())
                        .labels(MovieLabels::pt_br())
                        .width((size.width - AROUND_THE_PLAYER).max(480.0)),
                ),
                None => Box::new(
                    Alert::new("Treinamentos indisponíveis", "O arquivo dos filmes não pôde ser lido.")
                        .tone(Tone::Danger),
                ),
            }
        }),
    ))
    .spacing(16.0)
    .align(Align::Stretch),))
    .padding(24.0)
}

/// Saves the report to a file the person sends to the team: nothing here goes
/// anywhere by itself, and the film shows their data.
pub(crate) fn reporter(store: &Store, recorder: &FlightRecorder) -> BugReporter {
    let (store, today) = (store.clone(), store.today);
    BugReporter::new(recorder.clone(), move |report| {
        let name = format!("problema-raven-financas-{today}.ravenbug");
        store.save_file(&name, ("Relato de problema", &["ravenbug"]), &report.to_bytes());
    })
    .labels(BugLabels::pt_br())
    .details(|| {
        vec![
            ("versão".to_owned(), update::CURRENT.to_owned()),
            ("sistema".to_owned(), std::env::consts::OS.to_owned()),
        ]
    })
}
