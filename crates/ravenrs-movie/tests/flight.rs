//! `FlightRecorder` and `BugReporter`: what led to a bug, seen before it goes.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Event, Point, PointerButton, Size};
use ravenrs_movie::{BugReport, BugReporter, FlightRecorder};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn app(
    recorder: &FlightRecorder,
    count: &State<u32>,
    reporter: Option<BugReporter>,
) -> impl View + 'static + use<> {
    let (shown, more) = (count.clone(), count.clone());
    VStack::new((
        recorder.clone(),
        Text::bound(&shown, |n| format!("Clicked {n}")),
        Button::new("More").on_click(move || more.update(|n| *n += 1)),
        reporter,
    ))
}

fn run(ui: &mut Ui, from: f64, to: f64) -> f64 {
    let mut now = from;
    while now < to {
        now += 0.1;
        ui.set_time(now);
        ui.update();
    }
    now
}

fn click(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let found =
        tree.nodes.iter().find(|n| n.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("no {name}"));
    let at = found.bounds.center();
    ui.handle_event(&Event::PointerMove { position: at });
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    ui.update();
}

fn ui_of(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_light());
    ui.set_viewport(Size::new(1000.0, 800.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn has(ui: &mut Ui, name: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)))
}

#[test]
fn it_keeps_what_happened_with_the_pointer_and_clicks() {
    let (recorder, count) = (FlightRecorder::new(60.0), State::new(0));
    let mut ui = ui_of(app(&recorder, &count, None));
    run(&mut ui, 0.0, 1.0);
    click(&mut ui, "More");
    let now = run(&mut ui, 1.0, 2.0);
    click(&mut ui, "More");
    run(&mut ui, now, 3.0);

    let film = recorder.film().expect("a film");
    assert!(film.duration_ms() >= 1900, "up to the last thing that happened: {}", film.duration_ms());
    assert!(film.click_before(film.duration_ms()).is_some());
    assert!(film.pointer_at(film.duration_ms()).is_some());
    let bytes = film.to_bytes();
    assert!(bytes.len() < 64 * 1024, "a few seconds of a small screen: {} bytes", bytes.len());
}

#[test]
fn it_only_keeps_the_last_stretch() {
    let (recorder, count) = (FlightRecorder::new(10.0), State::new(0));
    let mut ui = ui_of(app(&recorder, &count, None));
    let mut now = 0.0;
    for _ in 0..30 {
        click(&mut ui, "More");
        now = run(&mut ui, now, now + 1.0);
    }
    let film = recorder.film().expect("a film");
    let seconds = film.duration_ms() / 1000;
    assert!((4..=11).contains(&seconds), "{seconds} s kept of 30");
}

#[test]
fn a_report_shows_the_film_first_and_goes_only_when_sent() {
    let sent: Rc<RefCell<Vec<BugReport>>> = Rc::default();
    let inbox = sent.clone();
    let (recorder, count) = (FlightRecorder::new(60.0), State::new(0));
    let reporter = BugReporter::new(recorder.clone(), move |report| inbox.borrow_mut().push(report))
        .details(|| vec![("version".to_owned(), "1.2.3".to_owned())]);
    let mut ui = ui_of(app(&recorder, &count, Some(reporter.clone())));
    let now = run(&mut ui, 0.0, 1.0);
    click(&mut ui, "More");
    run(&mut ui, now, 2.0);

    reporter.open();
    ui.update();
    assert!(has(&mut ui, "Report a problem"));
    assert!(has(&mut ui, "The film shows what was on your screen"));
    assert!(sent.borrow().is_empty(), "nothing goes by itself");

    let tree = ui.access_tree();
    let field = tree.nodes.iter().find(|n| n.role == ravenrs_core::Role::TextInput).expect("the field").id;
    ui.perform_action(field, AccessAction::Focus);
    ui.update();
    for letter in "It broke".chars() {
        ui.handle_event(&Event::KeyDown {
            key: ravenrs_core::Key::Character(letter.to_string()),
            modifiers: ravenrs_core::Modifiers::default(),
        });
    }
    ui.update();
    let tree = ui.access_tree();
    let send = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Send")).expect("send").id;
    ui.perform_action(send, AccessAction::Click);
    ui.update();

    let report = sent.borrow()[0].clone();
    assert_eq!(report.description, "It broke");
    assert_eq!(report.details[0].1, "1.2.3");
    assert!(report.film.is_some());
    let back = BugReport::from_bytes(&report.to_bytes()).expect("read back");
    assert_eq!(back.description, "It broke");
    assert!(back.film.is_some());
}

#[test]
fn the_film_can_be_left_out_and_cancelling_sends_nothing() {
    let sent: Rc<RefCell<Vec<BugReport>>> = Rc::default();
    let inbox = sent.clone();
    let (recorder, count) = (FlightRecorder::new(60.0), State::new(0));
    let reporter = BugReporter::new(recorder.clone(), move |report| inbox.borrow_mut().push(report));
    let mut ui = ui_of(app(&recorder, &count, Some(reporter.clone())));
    run(&mut ui, 0.0, 1.0);
    reporter.open();
    ui.update();
    click(&mut ui, "Cancel");
    assert!(!has(&mut ui, "Report a problem"));
    assert!(sent.borrow().is_empty());

    let report = BugReport { description: "x".to_owned(), ..BugReport::default() };
    assert!(BugReport::from_bytes(&report.to_bytes()).expect("read back").film.is_none());
    assert!(BugReport::from_bytes(b"nope").is_err());
}

fn drag(ui: &mut Ui, from: Point, to: Point) {
    ui.handle_event(&Event::PointerMove { position: from });
    ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerMove {
        position: Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0),
    });
    ui.handle_event(&Event::PointerMove { position: to });
    ui.update();
    ui.handle_event(&Event::PointerUp { position: to, button: PointerButton::Primary });
    ui.update();
    ui.update();
}

#[test]
fn a_box_around_the_problem_names_what_is_under_it_and_lights_it_in_the_film() {
    let recorder = FlightRecorder::new(60.0);
    let count = State::new(0);
    let sent = Rc::new(RefCell::new(None::<BugReport>));
    let keep = sent.clone();
    let reporter = BugReporter::new(recorder.clone(), move |report| *keep.borrow_mut() = Some(report))
        .errors(|| vec!["could not save".to_owned()]);
    let opener = reporter.clone();
    let mut ui = ui_of(app(&recorder, &count, Some(reporter)));
    let now = run(&mut ui, 0.0, 1.0);
    let more =
        ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some("More")).expect("button").bounds;
    opener.open();
    ui.update();
    click(&mut ui, "Mark the problem on screen");
    run(&mut ui, now, now + 0.3);
    let page = ui.access_tree();
    assert!(page.nodes.iter().any(|n| n.name.as_deref() == Some("Drag a box around the problem.")));

    drag(
        &mut ui,
        Point::new((more.x() - 4.0).max(1.0), (more.y() - 4.0).max(1.0)),
        Point::new(more.x() + more.width() + 4.0, more.y() + more.height() + 4.0),
    );
    let names: Vec<_> = ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
    assert!(names.iter().any(|n| n == "Mark it again"), "back in the dialog, at {more:?}: {names:?}");

    let tree = ui.access_tree();
    let field = tree.nodes.iter().find(|n| n.role == ravenrs_core::Role::TextInput).expect("the field").id;
    ui.perform_action(field, AccessAction::Focus);
    ui.update();
    for letter in "It broke".chars() {
        ui.handle_event(&Event::KeyDown {
            key: ravenrs_core::Key::Character(letter.to_string()),
            modifiers: ravenrs_core::Modifiers::default(),
        });
    }
    ui.update();
    let tree = ui.access_tree();
    let send = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Send")).expect("send").id;
    ui.perform_action(send, AccessAction::Click);
    ui.update();

    let report = sent.borrow().clone().expect("sent");
    assert_eq!(report.kind, "Something is wrong");
    assert!(report.area.is_some_and(|area| area.width >= more.width()));
    assert_eq!(report.elements, ["More"], "the panel around it does not count");
    assert_eq!(report.errors, ["could not save"]);
    let film = report.film.as_ref().expect("the film");
    assert!(
        film.project()
            .overlays
            .iter()
            .any(|o| matches!(o.kind, ravenrs_movie::OverlayKind::Spotlight { .. })),
        "the film lights the box"
    );

    let back = BugReport::from_bytes(&report.to_bytes()).expect("read back");
    assert_eq!(
        (back.kind, back.elements, back.errors),
        (report.kind.clone(), report.elements.clone(), report.errors.clone())
    );
    assert_eq!(back.area, report.area);
}

#[test]
fn a_slipped_click_is_not_a_box_and_escape_leaves_without_marking() {
    let recorder = FlightRecorder::new(60.0);
    let count = State::new(0);
    let sent = Rc::new(RefCell::new(None::<BugReport>));
    let keep = sent.clone();
    let reporter = BugReporter::new(recorder.clone(), move |report| *keep.borrow_mut() = Some(report));
    let opener = reporter.clone();
    let mut ui = ui_of(app(&recorder, &count, Some(reporter)));
    let now = run(&mut ui, 0.0, 1.0);
    opener.open();
    ui.update();
    click(&mut ui, "Mark the problem on screen");
    run(&mut ui, now, now + 0.3);

    drag(&mut ui, Point::new(300.0, 300.0), Point::new(303.0, 302.0));
    assert!(
        ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Drag a box around the problem.")),
        "still marking"
    );

    ui.handle_event(&Event::KeyDown {
        key: ravenrs_core::Key::Escape,
        modifiers: ravenrs_core::Modifiers::default(),
    });
    ui.update();
    let tree = ui.access_tree();
    assert!(
        tree.nodes.iter().any(|n| n.name.as_deref() == Some("Mark the problem on screen")),
        "back, with nothing marked"
    );
}
