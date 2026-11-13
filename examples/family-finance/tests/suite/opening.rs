//! The window from its first frame: the splash screen, and what comes after it.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use family_finance::ui::Opening;
use family_finance::update::Updater;
use ravenrs::movie::RecentErrors;
use ravenrs::prelude::*;

fn today() -> Date {
    Date::new(2026, 11, 15).expect("valid date")
}

/// A folder of its own for each test, emptied before use.
fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("raven-financas-abertura-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    folder
}

fn opening(path: &Path) -> Ui {
    let root = Opening::new(
        path.to_path_buf(),
        today(),
        Updater::idle(),
        Diagnostics::default(),
        RecentErrors::default(),
    );
    let mut ui = Ui::new(root, Theme::raven_light());
    ui.set_viewport(Size::new(1360.0, 860.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

fn says(ui: &mut Ui, text: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| {
        [&n.name, &n.value, &n.description]
            .iter()
            .any(|field| field.as_deref().is_some_and(|t| t.contains(text)))
    })
}

fn has_splash(ui: &mut Ui) -> bool {
    ui.access_tree()
        .nodes
        .iter()
        .any(|n| n.role == Role::Dialog && n.name.as_deref() == Some("Raven Finanças"))
}

/// Rebuilds, on a clock standing still, until the thread has finished.
fn wait_for(ui: &mut Ui, what: &str, done: impl Fn(&mut Ui) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done(ui) {
        assert!(Instant::now() < deadline, "never got to: {what}");
        std::thread::sleep(Duration::from_millis(5));
        ui.update();
    }
}

#[test]
fn the_splash_comes_first_and_gives_way_to_the_app() {
    let path = folder("splash").join("financas.db");
    let mut ui = opening(&path);
    assert!(has_splash(&mut ui), "on screen from the first frame");

    // The thread finishes, but nothing heavy happens while the logo plays.
    wait_for(&mut ui, "the copy of the day", |ui| says(ui, "Guardando a cópia do dia"));
    std::thread::sleep(Duration::from_millis(300));
    ui.update();
    assert!(!says(&mut ui, "Preparando as telas"), "the screens wait for the logo to play");
    assert!(path.exists(), "the thread created the database");

    // The app is built under the splash, which still has its time on screen.
    ui.set_time(2.0);
    wait_for(&mut ui, "the last step", |ui| says(ui, "Preparando as telas"));
    assert!(has_splash(&mut ui), "a fast computer shows a name to read, not a flash");

    ui.set_time(2.3);
    ui.update();
    ui.set_time(3.0);
    ui.update();
    ui.update();
    assert!(!has_splash(&mut ui), "gone once its time is up");
    assert!(says(&mut ui, "Lançamentos"), "the app, under what covered it");
    // The dashboard's charts draw their data in, for about a second.
    let mut time = 3.0;
    while time < 5.0 {
        time += 1.0 / 30.0;
        ui.set_time(time);
        ui.update();
    }
    assert!(ui.next_wake().is_none(), "an idle window stays asleep, got {:?}", ui.next_wake());
}

#[test]
fn the_animated_logo_plays_whole_in_our_engine() {
    let logo =
        LottieAnimation::from_json(include_bytes!("../../assets/logo-animada.json")).expect("the logo");
    assert_eq!(logo.unsupported(), Vec::<String>::new(), "nothing in it is left out");
    assert!((logo.duration() - 1.9).abs() < 0.01, "as long as the splash expects: {}", logo.duration());
    let solid = |paths: &[ravenrs::core::PathFill]| paths.iter().filter(|p| p.opacity > 0.99).count();
    let (first, last) = (logo.paths_at(0.0, 1.0), logo.paths_at(logo.duration(), 1.0));
    assert!(solid(&last) > solid(&first) + 3, "it ends on more than it starts from");
}

#[test]
fn a_database_that_does_not_open_is_said_in_the_window() {
    let file = folder("falha").join("um-arquivo");
    std::fs::write(&file, b"").expect("file");
    // A folder cannot be made inside a file.
    let mut ui = opening(&file.join("financas.db"));

    wait_for(&mut ui, "the failure", |ui| says(ui, "Não foi possível abrir o banco de dados"));
    ui.set_time(3.0);
    ui.update();
    ui.update();
    assert!(!has_splash(&mut ui), "nothing is left to wait for");
}
