//! `MoviePlayer`: plays by the clock, stops where it teaches, and jumps around.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, AccessNode, Color, Point, Primitive, Quad, Rect, Role, Size};
use ravenrs_movie::{FilmBuilder, Hotspot, HotspotKind, Movie, MovieLabels, MoviePlayer, Narration};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{AnyView, SoundClip, SoundProvider, Text, Ui, View, responsive};

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
    assert!(has(&mut ui, "Second part What comes next"), "the chapter says what it teaches");
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

/// What a player asked the sound system to do.
#[derive(Clone, Default)]
struct Heard(Rc<RefCell<Vec<String>>>);

impl SoundProvider for Heard {
    fn play(&self, clip: &SoundClip) {
        let name = if *clip == SoundClip::click() { "click" } else { "key" };
        self.0.borrow_mut().push(name.to_owned());
    }

    fn speak(&self, text: &str, language: &str) {
        self.0.borrow_mut().push(format!("{language}: {text}"));
    }

    fn stop_speaking(&self) {
        self.0.borrow_mut().push("quiet".to_owned());
    }

    fn speak_clip(&self, _clip: &SoundClip) {
        self.0.borrow_mut().push("recording".to_owned());
    }
}

impl Heard {
    fn count(&self, what: &str) -> usize {
        self.0.borrow().iter().filter(|heard| heard.as_str() == what).count()
    }

    fn said(&self, what: &str) -> bool {
        self.0.borrow().iter().any(|heard| heard.contains(what))
    }
}

fn noisy_film() -> Movie {
    let mut film = FilmBuilder::new(Size::new(W as f32, H as f32), Color::WHITE);
    for i in 0..=10u64 {
        let square = Quad::new(Rect::new(i as f32, 0.0, 4.0, 4.0), Color::BLACK);
        film.frame(i * 1000, &[Primitive::Quad(square)]);
    }
    film.click(1000, Point::new(1.0, 1.0));
    film.key(2000);
    film.key(2100);
    film.click(7000, Point::new(1.0, 1.0));
    film.hotspot(spot(HotspotKind::Note, "A note", 3000, 4000));
    film.finish(10_000).expect("film")
}

fn heard_player(movie: Movie) -> (Ui, Heard) {
    let heard = Heard::default();
    let mut ui = player(MoviePlayer::new(movie));
    ui.set_sound(heard.clone());
    ui.update();
    (ui, heard)
}

#[test]
fn it_plays_each_click_and_key_once_as_the_film_goes_past() {
    let (mut ui, heard) = heard_player(noisy_film());
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 2.5);
    assert_eq!(heard.count("click"), 1);
    assert_eq!(heard.count("key"), 2);
    run_to(&mut ui, 2.5, 7.5);
    assert_eq!(heard.count("click"), 2);
}

#[test]
fn jumping_over_clicks_makes_no_sound_and_a_switch_silences_them() {
    let (mut ui, heard) = heard_player(noisy_film());
    click(&mut ui, "Play");
    ui.set_time(5.0);
    ui.update();
    assert_eq!(heard.count("click"), 0, "a late frame does not play what it skipped");

    let (mut ui, heard) = heard_player(noisy_film());
    click(&mut ui, "Sound");
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 2.5);
    assert_eq!(heard.count("click"), 0);
}

#[test]
fn the_voice_says_each_note_once_in_the_language_of_the_labels() {
    let (mut ui, heard) = heard_player(noisy_film());
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 3.8);
    assert!(heard.said("en-US: A note. A note text"));
    let times = heard.0.borrow().iter().filter(|h| h.contains("A note text")).count();
    assert_eq!(times, 1);
}

#[test]
fn the_voice_stops_on_a_pause_on_turning_it_off_and_on_leaving() {
    let (mut ui, heard) = heard_player(noisy_film());
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 3.5);
    click(&mut ui, "Pause");
    assert!(heard.count("quiet") >= 1, "pausing stops it");

    let heard = Heard::default();
    let on_screen = State::new(true);
    let (shown, movie) = (on_screen.clone(), noisy_film());
    let view = responsive(move |_| -> AnyView {
        if shown.get() { Box::new(MoviePlayer::new(movie.clone())) } else { Box::new(Text::new("Elsewhere")) }
    });
    let mut ui = Ui::new(view, Theme::raven_light());
    ui.set_sound(heard.clone());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    let before = heard.count("quiet");
    on_screen.set(false);
    ui.update();
    assert!(heard.count("quiet") > before, "leaving the screen stops it");
}

#[test]
fn a_step_says_what_to_do_and_a_click_goes_past_its_reading_time() {
    let mut film = FilmBuilder::new(Size::new(W as f32, H as f32), Color::WHITE);
    for i in 0..=10u64 {
        film.frame(i * 1000, &[Primitive::Quad(Quad::new(Rect::new(i as f32, 0.0, 4.0, 4.0), Color::BLACK))]);
    }
    film.hotspot(spot(HotspotKind::Stop, "Click here", 2000, 5000));
    let film = film.finish(10_000).expect("film");

    let (mut ui, heard) = heard_player(film.clone());
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 3.0);
    assert!(says_time(&mut ui, "0:02"), "it waits at the start of the step");
    assert!(heard.said("Click here text"), "and the voice says what to do");
    click(&mut ui, "Click here");
    run_to(&mut ui, 3.0, 3.2);
    assert!(says_time(&mut ui, "0:05"), "a click skips the time meant for reading");

    let (mut ui, heard) = heard_player(film);
    click(&mut ui, "Stop at each step");
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 3.0);
    assert!(says_time(&mut ui, "0:03"), "with the steps off it runs on");
    assert!(has(&mut ui, "Click here text"), "with its box up while there is time to read");
    assert!(heard.said("Click here text"));
}

#[test]
fn a_box_about_the_whole_screen_has_no_mark_and_a_small_film_says_it_below() {
    let mut film = FilmBuilder::new(Size::new(W as f32, H as f32), Color::WHITE);
    film.frame(0, &[]);
    let general = Hotspot { width: 0.0, height: 0.0, ..spot(HotspotKind::Note, "All of it", 0, 5000) };
    assert!(general.is_general());
    film.hotspot(general);
    let film = film.finish(10_000).expect("film");
    let mut ui = player(MoviePlayer::new(film.clone()));
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 1.0);
    assert!(has(&mut ui, "All of it text"));
    let tree = ui.access_tree();
    assert!(!tree.nodes.iter().any(|n| n.role == Role::Button && n.name.as_deref() == Some("All of it")));

    let mut ui = player(MoviePlayer::new(film).width(400.0));
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 1.0);
    assert!(has(&mut ui, "All of it text"), "too small for a box, it goes under the film");
}

fn recorded_player(narration: Narration) -> (Ui, Heard) {
    let heard = Heard::default();
    let mut ui = Ui::new(MoviePlayer::new(noisy_film()).narration(narration), Theme::raven_light());
    ui.set_sound(heard.clone());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    (ui, heard)
}

fn narration_of(text: &str) -> Narration {
    let bytes = Narration::pack(&[(text.to_owned(), vec![0.1; 1600])], 16_000);
    Narration::new(Box::leak(bytes.into_boxed_slice()))
}

#[test]
fn a_recording_is_said_in_place_of_the_system_voice_and_stops_with_the_film() {
    let (mut ui, heard) = recorded_player(narration_of("A note. A note text"));
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 3.5);
    assert_eq!(heard.count("recording"), 1);
    assert!(!heard.said("en-US: A note. A note text"), "the system stays quiet");
    click(&mut ui, "Pause");
    assert!(heard.count("quiet") >= 1, "pausing stops it");
}

#[test]
fn what_has_no_recording_is_said_by_the_system_voice() {
    let (mut ui, heard) = recorded_player(narration_of("Something else"));
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 3.8);
    assert_eq!(heard.count("recording"), 0);
    assert!(heard.said("en-US: A note. A note text"));
}

#[test]
fn the_end_offers_to_watch_again_and_what_comes_next() {
    let wanted = std::rc::Rc::new(std::cell::Cell::new(false));
    let told = wanted.clone();
    let mut ui = player(MoviePlayer::new(film()).next("Parte dois", move || told.set(true)));
    click(&mut ui, "Stop at each step");
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 11.0);
    assert!(has(&mut ui, "You finished this film"));
    assert!(has(&mut ui, "Watch again"));
    click(&mut ui, "Next: Parte dois");
    assert!(wanted.get());
}

#[test]
fn a_faster_speed_gets_there_sooner() {
    let mut ui = player(MoviePlayer::new(film()));
    click(&mut ui, "Stop at each step");
    click(&mut ui, "1×");
    assert!(has(&mut ui, "1,25×"));
    click(&mut ui, "1,25×");
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 2.0);
    assert!(says_time(&mut ui, "0:03"), "two seconds at one and a half times");
}

fn opened_and_closed(film: &Movie) -> Movie {
    use ravenrs_movie::CardLook;
    let mut project = film.project().clone();
    let end = project.duration_ms();
    project.insert_card(end, "Até logo", "Você já sabe o caminho", 3000, CardLook::Closing);
    project.insert_card(0, "Welcome", "Part one", 3000, CardLook::Opening);
    film.with_project(project)
}

#[test]
fn an_opening_says_its_words_once_they_come_in_and_the_poster_shows_them() {
    let movie = opened_and_closed(&noisy_film());
    assert_eq!(movie.poster_ms(), ravenrs_movie::CardLook::Opening.words_in_ms());
    let heard = Heard::default();
    let mut ui =
        Ui::new(MoviePlayer::new(movie).narration(narration_of("Welcome. Part one")), Theme::raven_light());
    ui.set_sound(heard.clone());
    ui.set_viewport(Size::new(1200.0, 900.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, 0.8);
    assert_eq!(heard.count("recording"), 0, "the logo plays first");
    run_to(&mut ui, 0.8, 2.0);
    assert_eq!(heard.count("recording"), 1, "then the words are said, once");
    assert!(!heard.said("en-US: Welcome. Part one"), "never by the system's voice");
}

#[test]
fn a_farewell_with_no_recording_stays_quiet() {
    let movie = opened_and_closed(&noisy_film());
    let end = movie.duration_ms();
    let (mut ui, heard) = heard_player(movie);
    click(&mut ui, "Stop at each step");
    click(&mut ui, "Play");
    run_to(&mut ui, 0.0, end as f64 / 1000.0 - 0.5);
    assert!(!heard.said("Até logo"), "a second voice would not match the film's");
}

#[test]
fn a_recorded_box_lasts_as_long_as_its_recording() {
    use ravenrs_movie::Recorder;
    let three_seconds = Narration::pack(&[("Hi. There".to_owned(), vec![0.1; 48_000])], 16_000);
    let narration = Narration::new(Box::leak(three_seconds.into_boxed_slice()));
    let mut recorder =
        Recorder::new(Text::new("App"), Theme::raven_light(), Size::new(400.0, 300.0)).narration(narration);
    recorder.intro("Hi", "There").intro("Not", "recorded");
    let movie = recorder.finish().expect("film");
    let spots = movie.hotspots();
    let lasts = |i: usize| spots[i].until_ms - spots[i].from_ms;
    assert!((3_550..=3_650).contains(&lasts(0)), "its recording and a breath: {}", lasts(0));
    assert!((2_000..=2_400).contains(&lasts(1)), "as long as reading it: {}", lasts(1));
    assert!(movie.duration_ms() >= spots[1].until_ms, "the film lasts until the last box ends");
}
