//! Films of end-to-end test sessions (#670): made from what the session draws and does, the
//! same bytes on every run, with nothing secret in them, and over the test channel too.

#![allow(clippy::unwrap_used, reason = "tests fail loudly")]

use std::io::Cursor;

use ravenrs_core::Size;
use ravenrs_e2e::E2e;
use ravenrs_e2e_wire::{Command, Hello, Reply, serve_on, serve_with};
use ravenrs_movie::{ChannelFilm, Movie, Recorder, TestFilm};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, TextField, VStack, ViewExt};

fn form() -> impl ravenrs_widgets::View {
    let name = State::new(String::new());
    let secret = State::new(String::new());
    let saved = State::new(0_u32);
    let count = saved.clone();
    VStack::new((
        TextField::new(name.binding()).test_id("nome"),
        TextField::new(secret.binding()).secure().test_id("senha"),
        Button::new("Salvar").on_click(move || count.update(|n| *n += 1)).test_id("salvar"),
    ))
}

fn session() -> E2e {
    E2e::new(form(), Theme::raven_light()).viewport(Size::new(480.0, 320.0), 1.0)
}

/// Films the test: types a name and `secret`, and saves.
fn filmed(secret: &str) -> Movie {
    let mut e2e = session();
    let film = TestFilm::start(&mut e2e);
    e2e.mark_step("Digitar o nome");
    e2e.by_test_id("nome").type_text("Ana").unwrap();
    e2e.mark_step("Digitar a senha");
    e2e.by_test_id("senha").type_text(secret).unwrap();
    e2e.mark_step("Salvar");
    e2e.by_test_id("salvar").click().unwrap();
    film.finish(&mut e2e).unwrap()
}

#[test]
fn a_test_session_becomes_a_film_with_its_steps_clicks_and_keys() {
    let movie = filmed("segredo1");
    let titles: Vec<&str> = movie.chapters().iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["Digitar o nome", "Digitar a senha", "Salvar"]);
    let back = Movie::from_bytes(&movie.to_bytes()).unwrap();
    assert_eq!(back.to_bytes(), movie.to_bytes());
    assert!(movie.click_before(u64::MAX).is_some(), "the clicks are in the film");
}

#[test]
fn two_runs_of_the_same_test_give_the_same_bytes() {
    assert_eq!(filmed("segredo1").to_bytes(), filmed("segredo1").to_bytes());
}

#[test]
fn a_secret_never_reaches_the_film() {
    // The field hides what is typed, so two secrets of the same length film the same.
    let one = filmed("segredo1").to_bytes();
    assert_eq!(one, filmed("outra123").to_bytes());
    assert!(!one.windows(8).any(|w| w == b"segredo1"));
}

#[test]
fn a_session_that_is_not_filmed_runs_as_before() {
    let mut e2e = session();
    e2e.by_test_id("nome").type_text("Ana").unwrap();
    e2e.by_test_id("nome").expect().to_have_value("Ana").unwrap();
    assert!(e2e.stop_observing().is_none());
}

fn lines(commands: &[Command]) -> String {
    commands.iter().map(|c| serde_json::to_string(c).unwrap() + "\n").collect()
}

fn replies(output: &[u8]) -> Vec<Reply> {
    String::from_utf8_lossy(output).lines().skip(1).map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn an_app_served_with_filming_keeps_the_film_where_the_tool_says() {
    let path = std::env::temp_dir()
        .join(format!("ravenrs-movie-canal-{}", std::process::id()))
        .join("cenario.ravenmovie");
    let input = lines(&[
        Command::Film { path: path.display().to_string() },
        Command::Type { target: "#nome".into(), text: "Ana".into() },
        Command::Click { target: "#salvar".into() },
        Command::FilmStop,
        Command::Bye,
    ]);
    let mut output = Vec::new();
    let mut film = ChannelFilm::default();
    serve_with(Hello::of("Formulário"), session(), Cursor::new(input), &mut output, Some(&mut film)).unwrap();
    let answers = replies(&output);
    assert!(answers.iter().all(|r| r.ok), "{answers:?}");
    assert_eq!(answers[3].snapshot.as_deref(), Some(path.display().to_string().as_str()));
    let movie = Movie::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
    assert!(movie.click_before(u64::MAX).is_some());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn an_app_served_without_filming_says_so() {
    let input = lines(&[Command::Film { path: "x.ravenmovie".into() }, Command::FilmStop, Command::Bye]);
    let mut output = Vec::new();
    serve_on(Hello::of("Formulário"), session(), Cursor::new(input), &mut output).unwrap();
    let answers = replies(&output);
    assert_eq!(answers[0].failure.as_ref().map(|f| f.code.as_str()), Some("E2E_NO_FILM"));
    let mut film = ChannelFilm::default();
    let stop = lines(&[Command::FilmStop, Command::Bye]);
    let mut output = Vec::new();
    serve_with(Hello::of("Formulário"), session(), Cursor::new(stop), &mut output, Some(&mut film)).unwrap();
    assert_eq!(replies(&output)[0].failure.as_ref().map(|f| f.code.as_str()), Some("E2E_NO_FILM"));
}

#[test]
fn the_recorder_finds_what_a_test_finds_by_its_id() {
    let mut recorder = Recorder::new(form(), Theme::raven_light(), Size::new(480.0, 320.0));
    recorder.click("#salvar").unwrap();
    let missing = recorder.click("#nada").unwrap_err().to_string();
    assert!(missing.contains("#salvar"), "it lists the ids on the screen: {missing}");
    assert!(recorder.finish().is_ok());
}
