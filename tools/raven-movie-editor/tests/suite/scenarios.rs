//! The scenarios of `tests/e2e/`, played on the tool opened with `--e2e-serve` in a process of
//! its own, as Raven Test Studio plays them, and the film it saved reopened to check it.

use std::time::Duration;

#[test]
fn every_scenario_passes_over_the_channel_and_the_saved_film_reopens() {
    let folder = std::env::temp_dir().join(format!("raven-movie-editor-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let start = || {
        let mut process = std::process::Command::new(env!("CARGO_BIN_EXE_raven-movie-editor"));
        process.arg(ravenrs_e2e_wire::SERVE_FLAG).env("RAVEN_MOVIE_EDITOR_E2E_DIR", &folder);
        ravenrs_e2e_wire::Client::spawn(process, Duration::from_secs(60))
            .unwrap_or_else(|f| panic!("{}", f.message))
    };
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let brought: Vec<String> = start().scenarios().to_vec();
    let texts: Vec<&str> = brought.iter().map(String::as_str).collect();
    let scenarios = ravenrs_e2e_wire::check_folder(&here, &texts).unwrap_or_else(|why| panic!("{why}"));
    for scenario in scenarios {
        let mut client = start();
        scenario.play_served(&mut client).unwrap_or_else(|why| panic!("{why}"));
        client.send(&ravenrs_e2e_wire::Command::Bye).ok();
    }
    let saved = folder.join("filme.ravenmovie");
    let bytes = std::fs::read(&saved).unwrap_or_else(|why| panic!("{}: {why}", saved.display()));
    let film = ravenrs::movie::Movie::from_bytes(&bytes).expect("the saved film reopens");
    assert!(!film.chapters().is_empty(), "the chapter was saved with the film");
    let _ = std::fs::remove_dir_all(&folder);
}
