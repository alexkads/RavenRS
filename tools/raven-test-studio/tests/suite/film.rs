//! The film of a run (#674): the Studio films the app it drives, here another Studio opened
//! apart, and keeps the film of a failure, or every film when asked.
#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use std::path::{Path, PathBuf};

use raven_test_studio::runner::FilmMode;
use raven_test_studio::{RunOptions, run_named_with};
use ravenrs::movie::Movie;

/// A folder of its own for one test, empty, with the scenario `text` in it.
fn scenario(name: &str, text: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("rts-filme-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("cenario.raventest");
    std::fs::write(&file, text).unwrap();
    (dir, file)
}

fn on_a_studio_apart(file: &Path, film: FilmMode, out: &Path) -> raven_test_studio::report::Report {
    raven_test_studio::apps::learn(raven_test_studio::apps::App {
        name: "Raven Test Studio".to_owned(),
        binary_path: Some(env!("CARGO_BIN_EXE_raven-test-studio").into()),
        ..raven_test_studio::apps::App::default()
    });
    let options = RunOptions { film, out: Some(out.to_path_buf()) };
    run_named_with(file.to_str().unwrap(), None, &options).unwrap()
}

const FAILS: &str =
    "scenario: Falha filmada\nopen Raven Test Studio\nclick #run\ncheck #summary = 4 de 5 passos passaram\n";
const PASSES: &str =
    "scenario: Passa filmado\nopen Raven Test Studio\nclick #run\ncheck #summary = 5 de 5 passos passaram\n";

#[test]
fn a_failure_keeps_the_film_of_the_app_it_failed_on() {
    let (dir, file) = scenario("falha", FAILS);
    let report = on_a_studio_apart(&file, FilmMode::OnFailure, &dir.join("out"));
    assert!(!report.passed());
    assert_eq!(report.films.len(), 1, "{}", report.text());
    let film = &report.films[0];
    assert!(film.ends_with("Falha-filmada/filme-1.ravenmovie"), "{film}");
    let movie = Movie::from_bytes(&std::fs::read(film).unwrap()).unwrap();
    assert!(movie.click_before(movie.duration_ms()).is_some(), "the click on Rodar is in the film");
    assert!(report.text().contains("filme: "), "{}", report.text());
    assert_eq!(report.json()["films"][0], film.as_str());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pass_keeps_no_film_unless_asked_for_every_one() {
    let (dir, file) = scenario("passa", PASSES);
    let out = dir.join("out");
    let report = on_a_studio_apart(&file, FilmMode::OnFailure, &out);
    assert!(report.passed(), "{}", report.text());
    assert!(report.films.is_empty());
    assert!(!out.join("Passa-filmado").join("filme-1.ravenmovie").exists(), "the film of a pass is dropped");

    let always = on_a_studio_apart(&file, FilmMode::Always, &out);
    assert_eq!(always.films.len(), 1);
    assert!(Path::new(&always.films[0]).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_app_that_does_not_film_does_not_fail_the_run() {
    // The sample form runs inside the Studio, not over the channel, so it is not filmed.
    let (dir, file) = scenario(
        "sem-filme",
        "scenario: Sem filme\nabrir Novo lançamento\ndigitar #descricao = Feira | Descrição\n",
    );
    let options = RunOptions { film: FilmMode::Always, out: Some(dir.join("out")) };
    let report = run_named_with(file.to_str().unwrap(), None, &options).unwrap();
    assert!(report.passed(), "{}", report.text());
    assert!(report.films.is_empty());
    assert!(report.film_note.as_deref().is_some_and(|n| n.contains("não filma")), "{:?}", report.film_note);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn film_with_nowhere_to_keep_it_is_refused() {
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_raven-test-studio"))
        .args(["--run", "autoteste", "--film", "always"])
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&run.stderr).contains("--out"));
    let bad = std::process::Command::new(env!("CARGO_BIN_EXE_raven-test-studio"))
        .args(["--run", "autoteste", "--film", "talvez"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
}
