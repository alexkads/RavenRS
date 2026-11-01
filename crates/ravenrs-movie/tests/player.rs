//! `MoviePlayer`: plays by the clock, stops where it teaches, and jumps around.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{AccessAction, AccessNode, Color, Primitive, Quad, Rect, Role, Size};
use ravenrs_movie::{FilmBuilder, Hotspot, HotspotKind, Movie, MovieLabels, MoviePlayer};
use ravenrs_style::Theme;
use ravenrs_widgets::{Ui, View};

const W: u32 = 400;
const H: u32 = 300;

fn spot(kind: HotspotKind, title: &str, from_ms: u64, until_ms: u64) -> Hotspot {
    Hotspot {
        kind,
        x: 4.0,
        y: 4.0,
        width: 10.0,
        height: 10.0,
        title: title.to_owned(),
        text: format!("{title} text"),
        from_ms,
        until_ms,
    }
}

fn film() -> Movie {
    let mut film = FilmBuilder::new(Size::new(W as f32, H as f32), Color::WHITE);
    for i in 0..=10u64 {
        let square = Quad::new(Rect::new(i as f32, 0.0, 4.0, 4.0), Color::BLACK);
        film.frame(i * 1000, &[Primitive::Quad(square)]);
    }
    film.chapter("Opening", "What it is", 0);
    film.chapter("Second part", "What comes next", 6000);
    film.hotspot(spot(HotspotKind::Note, "A note", 1000, 3000));
    film.hotspot(spot(HotspotKind::Stop, "Click here", 5000, 5000));
    film.finish(10_000).expect("film")
}

fn player(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_light());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn find<'a>(nodes: &'a [AccessNode], name: &str) -> Option<&'a AccessNode> {
    nodes.iter().find(|n| n.name.as_deref() == Some(name))
}

fn has(ui: &mut Ui, name: &str) -> bool {
    find(&ui.access_tree().nodes, name).is_some()
}

fn click(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some(name))
        .or_else(|| tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name))))
        .unwrap_or_else(|| panic!("no {name}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
}

/// Runs the clock to `seconds`, in small steps, as frames would.
fn run_to(ui: &mut Ui, from: f64, seconds: f64) {
    let mut now = from;
    while now < seconds {
        now = (now + 0.1).min(seconds);
        ui.set_time(now);
        ui.update();
    }
}

fn says_time(ui: &mut Ui, time: &str) -> bool {
    let tree = ui.access_tree();
    let found = tree.nodes.iter().any(|n| n.name.as_deref().is_some_and(|t| t.starts_with(time)));
    if !found {
        let names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
        eprintln!("looking for {time} in {names:?}");
    }
    found
}

#[test]
fn it_starts_stopped_on_the_first_picture_and_does_not_ask_for_frames() {
    let mut ui = player(MoviePlayer::new(film()));
    assert!(has(&mut ui, "Play"));
    assert!(says_time(&mut ui, "0:00 / 0:10"));
    assert_eq!(ui.next_wake(), None, "stopped, it sleeps");
}

#[test]
fn it_plays_by_the_clock_and_shows_a_note_while_it_lasts() {
    let mut ui = player(MoviePlayer::new(film()));
    click(&mut ui, "Play");
    assert!(has(&mut ui, "Pause"));
    assert!(ui.next_wake().is_some(), "playing, it asks for the next picture");
    run_to(&mut ui, 0.0, 2.0);
    assert!(says_time(&mut ui, "0:02"));
    assert!(has(&mut ui, "A note"), "the mark is on the film");
    assert!(has(&mut ui, "A note text"), "and its text under it");
    run_to(&mut ui, 2.0, 4.0);
    assert!(!has(&mut ui, "A note"), "it goes away when it ends");
}

#[test]
fn it_waits_at_a_step_until_the_spot_is_clicked() {
    let mut ui = player(MoviePlayer::new(film()));
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 8.0);
    assert!(says_time(&mut ui, "0:05"), "it holds at the step, however long the clock runs");
    assert!(has(&mut ui, "Click here"));
    assert!(has(&mut ui, "Play"), "and it counts as stopped");
    assert_eq!(ui.next_wake(), None);

    click(&mut ui, "Click here");
    run_to(&mut ui, 8.0, 9.5);
    assert!(says_time(&mut ui, "0:06"), "a click on the spot goes on");
    assert!(!has(&mut ui, "Click here"));
}

#[test]
fn the_step_can_be_passed_with_a_button_or_turned_off() {
    let mut ui = player(MoviePlayer::new(film()));
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 6.0);
    click(&mut ui, "Continue");
    run_to(&mut ui, 6.0, 7.5);
    assert!(says_time(&mut ui, "0:06"));

    let mut ui = player(MoviePlayer::new(film()));
    click(&mut ui, "Stop at each step");
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 7.5);
    assert!(says_time(&mut ui, "0:07"), "with the steps off it runs through");
}

#[test]
fn chapters_jump_and_the_end_stops_and_plays_again() {
    let mut ui = player(MoviePlayer::new(film()));
    click(&mut ui, "Second part");
    assert!(says_time(&mut ui, "0:06"));
    assert!(has(&mut ui, "What comes next"));
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 5.0);
    assert!(says_time(&mut ui, "0:10"));
    assert!(has(&mut ui, "Play"), "stopped at the end");
    click(&mut ui, "Play");
    run_to(&mut ui, 5.0, 5.5);
    assert!(says_time(&mut ui, "0:00"), "and starts over");
}

#[test]
fn a_note_clicked_again_brings_its_text_back_and_words_can_be_changed() {
    let mut ui = player(MoviePlayer::new(film()).labels(MovieLabels::pt_br()));
    click(&mut ui, "Reproduzir");
    run_to(&mut ui, 0.0, 2.0);
    click(&mut ui, "Pausar");
    click(&mut ui, "A note");
    assert!(has(&mut ui, "A note text"));
    assert!(ui.access_tree().nodes.iter().any(|n| n.role == Role::Image));
}
