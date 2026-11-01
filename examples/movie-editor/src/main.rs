//! Opens a film (`.ravenmovie`) or a bug report (`.ravenbug`) and edits it. What a
//! report says shows in the terminal, and saving keeps the film beside it.

use std::path::PathBuf;

use ravenrs::movie::{BugReport, EditorLabels, EditorState, Movie, MovieEditor, MovieLabels};
use ravenrs::native::{OpenDialog, PlatformError, WindowOptions, run};
use ravenrs::prelude::*;

/// The film in a file, whichever kind of file it is.
fn film_in(bytes: &[u8]) -> Result<Movie, String> {
    if let Ok(report) = BugReport::from_bytes(bytes) {
        println!("{}", report.description);
        for (name, value) in &report.details {
            println!("{name}: {value}");
        }
        return report.film.ok_or_else(|| "This report came without a film.".to_owned());
    }
    Movie::from_bytes(bytes).map_err(|e| e.to_string())
}

fn main() -> Result<(), PlatformError> {
    let chosen = std::env::args().nth(1).map(PathBuf::from).or_else(|| {
        OpenDialog::new().title("Open a film").filter("Film or bug report", &["ravenmovie", "ravenbug"]).ask()
    });
    let Some(path) = chosen else {
        return Ok(());
    };
    let film = match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| film_in(&bytes)) {
        Ok(film) => film,
        Err(why) => {
            eprintln!("{}: {why}", path.display());
            std::process::exit(1);
        }
    };
    let target = path.with_extension("ravenmovie");
    let editor = MovieEditor::new(EditorState::new(film))
        .labels(EditorLabels::pt_br())
        .player_labels(MovieLabels::pt_br())
        .on_save(move |film: &Movie| {
            if let Err(e) = std::fs::write(&target, film.to_bytes()) {
                eprintln!("{}: {e}", target.display());
            }
        });
    run(editor, WindowOptions::new("Editor de filmes").size(1400.0, 900.0).theme(Theme::raven_dark()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_without_a_film_says_so_and_rubbish_is_refused() {
        let report = BugReport { description: "x".to_owned(), film: None, details: Vec::new() };
        assert!(film_in(&report.to_bytes()).is_err());
        assert!(film_in(b"nope").is_err());
    }
}
