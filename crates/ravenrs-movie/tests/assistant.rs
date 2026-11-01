//! `MovieAssistant` and `MovieLibrary`: a question finds the moment and opens it.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessAction, Color, Event, Key, Modifiers, Primitive, Quad, Rect, Role, Size};
use ravenrs_movie::{
    AssistantLabels, FilmBuilder, Hotspot, HotspotKind, MomentKind, Movie, MovieAssistant, MovieCue,
    MovieLibrary, MoviePlayer,
};
use ravenrs_style::Theme;
use ravenrs_widgets::{Ui, View};

fn spot(kind: HotspotKind, title: &str, text: &str, from_ms: u64, until_ms: u64) -> Hotspot {
    Hotspot {
        kind,
        x: 4.0,
        y: 4.0,
        width: 10.0,
        height: 10.0,
        title: title.to_owned(),
        text: text.to_owned(),
        from_ms,
        until_ms,
    }
}

fn film(lessons: &[(&str, &str, u64)], spots: Vec<Hotspot>) -> Movie {
    let mut film = FilmBuilder::new(Size::new(400.0, 300.0), Color::WHITE);
    for i in 0..=20u64 {
        let square = Quad::new(Rect::new(i as f32, 0.0, 4.0, 4.0), Color::BLACK);
        film.frame(i * 1000, &[Primitive::Quad(square)]);
    }
    for (title, text, at) in lessons {
        film.chapter(*title, *text, *at);
    }
    for spot in spots {
        film.hotspot(spot);
    }
    film.finish(20_000).expect("film")
}

fn manual() -> Movie {
    film(
        &[
            ("Lançar uma despesa", "Como registrar o que você gastou.", 0),
            ("Pagar uma conta", "Como dar baixa no que venceu.", 8000),
            ("Guardar seus dados", "Faça um backup fora deste computador.", 15_000),
        ],
        vec![
            spot(HotspotKind::Stop, "Salve", "Confira e clique em Salvar lançamento.", 5000, 5000),
            spot(HotspotKind::Stop, "Pague", "Clique em Pagar na conta que você quitou.", 10_000, 10_000),
            spot(
                HotspotKind::Note,
                "Fazer backup",
                "Salva um arquivo com todos os seus dados.",
                16_000,
                18_000,
            ),
        ],
    )
}

fn library() -> MovieLibrary {
    MovieLibrary::new().film("Manual", manual())
}

fn best(question: &str) -> String {
    library().ask(question, 4).first().map_or_else(|| "nothing".to_owned(), |a| a.moment.title.clone())
}

#[test]
fn a_question_finds_the_moment_in_other_words_and_without_accents() {
    assert_eq!(best("Como pagar uma conta?"), "Pagar uma conta");
    assert_eq!(best("lancar despesa"), "Lançar uma despesa");
    assert_eq!(best("LANÇAMENTO"), "Lançar uma despesa");
    assert_eq!(best("onde faço o backup"), "Fazer backup");
    assert_eq!(best("como quitar"), "Pague");
    assert_eq!(best("despeza"), "Lançar uma despesa");
}

#[test]
fn a_question_nothing_talks_about_has_no_answer() {
    assert!(library().ask("cartão de crédito", 4).is_empty());
    assert!(library().ask("", 4).is_empty());
    assert!(library().ask("   ?! ", 4).is_empty());
}

#[test]
fn it_says_when_and_what_each_answer_is() {
    let answers = library().ask("pagar conta", 2);
    assert_eq!(answers.len(), 2);
    assert_eq!((answers[0].moment.kind, answers[0].moment.at_ms), (MomentKind::Chapter, 8000));
    assert_eq!((answers[1].moment.kind, answers[1].moment.at_ms), (MomentKind::Step, 10_000));
    assert!(answers[0].score > answers[1].score);
}

#[test]
fn it_searches_every_film_and_says_which() {
    let other = film(&[("Importar um extrato", "Traga o arquivo do banco.", 3000)], Vec::new());
    let library = MovieLibrary::new().film("Manual", manual()).film("Banco", other);
    assert_eq!(library.len(), 2);
    let answer = &library.ask("extrato do banco", 1)[0];
    assert_eq!((answer.moment.film, answer.moment.at_ms), (1, 3000));
}

fn ui_of(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_light());
    ui.set_viewport(Size::new(1200.0, 1000.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn names(ui: &mut Ui) -> Vec<String> {
    ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect()
}

fn says(ui: &mut Ui, wanted: &str) -> bool {
    names(ui).iter().any(|name| name.starts_with(wanted))
}

fn click(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)))
        .unwrap_or_else(|| panic!("no {name}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
}

fn press(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    ui.update();
}

fn ask(ui: &mut Ui, question: &str) {
    let tree = ui.access_tree();
    let field = tree.nodes.iter().find(|n| n.role == Role::TextInput).expect("the question field").id;
    ui.perform_action(field, AccessAction::Focus);
    ui.update();
    for character in question.chars() {
        press(ui, Key::Character(character.to_string()));
    }
}

#[test]
fn it_starts_with_the_film_and_no_answers() {
    let mut ui = ui_of(MovieAssistant::new(library()));
    assert!(says(&mut ui, "Play"));
    assert!(!says(&mut ui, "No film talks"));
    assert_eq!(ui.next_wake(), None, "with nothing asked, it sleeps");
}

#[test]
fn choosing_an_answer_opens_the_film_there_and_plays() {
    let mut ui = ui_of(MovieAssistant::new(library()));
    ask(&mut ui, "guardar meus dados");
    assert!(says(&mut ui, "Guardar seus dados"));
    click(&mut ui, "Guardar seus dados");
    assert!(says(&mut ui, "0:15 / 0:20"), "{:?}", names(&mut ui));
    assert!(says(&mut ui, "Pause"), "it plays from there");
}

#[test]
fn an_answer_that_is_a_step_waits_there_for_the_click() {
    let mut ui = ui_of(MovieAssistant::new(library()));
    ask(&mut ui, "quitar");
    click(&mut ui, "Pague");
    assert!(says(&mut ui, "0:10 / 0:20"));
    assert!(says(&mut ui, "Play"), "it waits instead of playing");
    assert!(says(&mut ui, "Continue"), "and says how to go on");
}

#[test]
fn enter_opens_the_best_answer() {
    let mut ui = ui_of(MovieAssistant::new(library()));
    ask(&mut ui, "pagar conta");
    press(&mut ui, Key::Enter);
    assert!(says(&mut ui, "0:08 / 0:20"), "{:?}", names(&mut ui));
}

#[test]
fn the_answers_step_aside_until_the_question_changes() {
    let mut ui = ui_of(MovieAssistant::new(library()));
    ask(&mut ui, "pagar conta");
    assert!(says(&mut ui, "Pague"));
    click(&mut ui, "Pagar uma conta");
    assert!(!says(&mut ui, "Pague"), "the film is what matters now");
    ask(&mut ui, "s");
    assert!(says(&mut ui, "Pague"), "another question, other answers");
}

#[test]
fn a_question_nothing_answers_says_so() {
    let mut ui = ui_of(MovieAssistant::new(library()));
    ask(&mut ui, "cartão de crédito");
    assert!(says(&mut ui, "No film talks about that"));
}

#[test]
fn answers_from_another_film_change_to_that_film() {
    let other = film(&[("Importar um extrato", "Traga o arquivo do banco.", 3000)], Vec::new());
    let library = MovieLibrary::new().film("Manual", manual()).film("Banco", other);
    let mut ui = ui_of(MovieAssistant::new(library));
    ask(&mut ui, "extrato");
    click(&mut ui, "Importar um extrato");
    assert!(says(&mut ui, "0:03 / 0:20"), "{:?}", names(&mut ui));
    assert!(says(&mut ui, "Importar um extrato"));
}

#[test]
fn the_words_can_be_changed() {
    let mut ui = ui_of(MovieAssistant::new(library()).labels(AssistantLabels::pt_br()));
    assert!(says(&mut ui, "Reproduzir"));
    assert!(says(&mut ui, "Pergunte como fazer"));
}

#[test]
fn a_cue_moves_a_player_from_outside() {
    let cue = MovieCue::new();
    let mut ui = ui_of(MoviePlayer::new(manual()).cue(cue.clone()));
    assert!(says(&mut ui, "0:00 / 0:20"));
    cue.go_to(16_000);
    ui.update();
    assert!(says(&mut ui, "0:16 / 0:20"));
    assert!(says(&mut ui, "Fazer backup"), "the note there shows");
}

#[test]
fn an_empty_library_draws_only_the_question() {
    let mut ui = ui_of(MovieAssistant::new(MovieLibrary::new()));
    assert!(!says(&mut ui, "Play"));
    assert!(says(&mut ui, "Ask how to do something"));
}
