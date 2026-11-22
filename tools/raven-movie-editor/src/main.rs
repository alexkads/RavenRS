//! Opens a film (`.ravenmovie`) or a bug report (`.ravenbug`) and edits it. What a
//! report says shows in the terminal, and saving keeps the film beside it.

use std::path::PathBuf;

use ravenrs::movie::{BugReport, EditorLabels, EditorState, Movie, MovieEditor, MovieLabels};
use ravenrs::native::{OpenDialog, PlatformError, WindowOptions};
use ravenrs::prelude::*;

/// The film in a file, whichever kind of file it is.
fn film_in(bytes: &[u8]) -> Result<Movie, String> {
    if let Ok(report) = BugReport::from_bytes(bytes) {
        println!("{}: {}", report.kind, report.description);
        for (name, value) in &report.details {
            println!("{name}: {value}");
        }
        if let Some(area) = report.area {
            println!("marked: {}x{} at {}, {}", area.width, area.height, area.x, area.y);
        }
        for element in &report.elements {
            println!("under the box: {element}");
        }
        for error in &report.errors {
            println!("error: {error}");
        }
        return report.film.ok_or_else(|| "This report came without a film.".to_owned());
    }
    Movie::from_bytes(bytes).map_err(|e| e.to_string())
}

mod versao;

/// A film of one empty frame, then a click, over a second: enough for the editor's timeline and
/// player, for the self-test and for the tests of Raven Test Studio.
fn made_up_film() -> Result<Movie, String> {
    let mut builder = ravenrs::movie::FilmBuilder::new(Size::new(640.0, 400.0), Color::rgb(0.1, 0.1, 0.12));
    builder.frame(0, &[]);
    builder.click(500, Point::new(320.0, 200.0));
    builder.finish(1000).map_err(|e| e.to_string())
}

/// The scenarios of Raven Test Studio the tool brings, one per file of `tests/e2e/`: the Studio
/// gets them when it opens the tool with `--e2e-serve`.
const SCENARIOS: [&str; 1] = [include_str!("../tests/e2e/abrir.raventest")];

/// Answers Raven Test Studio, with no window, on the made-up film; saving writes the film into
/// a folder of its own under the system's temporary folder, never next to the person's files.
/// `RAVEN_MOVIE_EDITOR_E2E_DIR` names that folder, for a test that reopens what was saved.
fn serve() -> Result<(), String> {
    let folder = std::env::var_os("RAVEN_MOVIE_EDITOR_E2E_DIR").map_or_else(
        || std::env::temp_dir().join(format!("raven-movie-editor-e2e-{}", std::process::id())),
        PathBuf::from,
    );
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let target = folder.join("filme.ravenmovie");
    let editor = MovieEditor::new(EditorState::new(made_up_film()?))
        .labels(EditorLabels::pt_br())
        .player_labels(MovieLabels::pt_br())
        .on_save(move |film: &Movie| {
            if let Err(e) = std::fs::write(&target, film.to_bytes()) {
                eprintln!("{}: {e}", target.display());
            }
        });
    let session = ravenrs_e2e::E2e::new(editor, Theme::raven_dark()).viewport(Size::new(1400.0, 900.0), 1.0);
    let hello = ravenrs_e2e_wire::Hello::of("Raven Movie Editor").scenarios(SCENARIOS);
    ravenrs_e2e_wire::serve_filmed(hello, session, ravenrs::movie::ChannelFilm::default())
        .map_err(|e| e.to_string())
}

/// Opens the editor on a made-up film with no window. Returns what it checked.
fn self_test() -> Result<String, String> {
    let film = made_up_film()?;
    let editor = MovieEditor::new(EditorState::new(film))
        .labels(EditorLabels::pt_br())
        .player_labels(MovieLabels::pt_br());
    let mut ui = Ui::new(editor, Theme::raven_dark());
    ui.set_viewport(Size::new(1400.0, 900.0), 1.0);
    for _ in 0..4 {
        ui.update();
    }
    let nodes = ui.access_tree().nodes.len();
    if nodes < 3 {
        return Err(format!("o editor abriu com {nodes} nós acessíveis"));
    }
    Ok("Autoteste: o editor abre um filme sem janela".to_owned())
}

fn main() -> Result<(), PlatformError> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // The installer proves a new binary opens with this, before any window or folder.
    if arguments.iter().any(|a| a == "--versao") {
        println!("{}", versao::VERSION);
        return Ok(());
    }
    // A test tool such as Raven Test Studio drives the editor with no window (`--e2e-serve`).
    if arguments.iter().any(|arg| arg == ravenrs_e2e_wire::SERVE_FLAG) {
        if let Err(why) = serve() {
            eprintln!("o canal de teste parou: {why}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if arguments.iter().any(|a| a == "--self-test") {
        match self_test() {
            Ok(said) => println!("{said}"),
            Err(why) => {
                eprintln!("o autoteste falhou: {why}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let chosen = arguments.first().map(PathBuf::from).or_else(|| {
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
    // Closing the window keeps the editor beside the clock, as the other tools do.
    let tray = ravenrs::native::SystemTray::new("Raven Movie Editor")
        .open("Abrir o Raven Movie Editor")
        .quit("Sair");
    let identity = ravenrs::native::AppIdentity::new(versao::APP.package)
        .labels(ravenrs::prelude::Labels::pt_br())
        .new_issue_url("https://github.com/alexkads/RavenRS/issues/new")
        .close_to_tray(tray);
    let options = WindowOptions::new("Raven Movie Editor").size(1400.0, 900.0).theme(Theme::raven_dark());
    ravenrs::native::run_as(editor, options, identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_without_a_film_says_so_and_rubbish_is_refused() {
        let report = BugReport { description: "x".to_owned(), ..BugReport::default() };
        assert!(film_in(&report.to_bytes()).is_err());
        assert!(film_in(b"nope").is_err());
    }
}
