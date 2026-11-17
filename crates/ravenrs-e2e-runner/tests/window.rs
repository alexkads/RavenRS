//! Running with the window (#729): the choices, their order of precedence, and the control a
//! person watching has: pause, exactly one more block, carry on and stop.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ravenrs_e2e::E2e;
use ravenrs_e2e_runner::config::Config;
use ravenrs_e2e_runner::run::{RunOptions, run_with};
use ravenrs_e2e_runner::session::{Failure, Session};
use ravenrs_e2e_runner::window::{Ask, Control, KeepOpen, Window, WindowMode, graphic_session_in, steer};
use ravenrs_e2e_wire::Scenario;
use ravenrs_style::Theme;
use ravenrs_widgets::{Text, ViewExt};

#[test]
fn the_command_line_comes_before_the_file() {
    let file = Config::parse("window = \"headed\"\nslow_mo = 300\nkeep_open = \"on-failure\"\n").unwrap();
    let (window, control) = file.window_from(&Ask::default()).unwrap();
    assert_eq!(window.mode, WindowMode::Headed);
    assert_eq!(window.slow_mo, Duration::from_millis(300));
    assert_eq!(window.keep_open, KeepOpen::OnFailure);
    assert!(control.is_some(), "a run with the window can be steered");

    let ask = Ask { slow_mo: Some("50".into()), keep_open: Some("never".into()), ..Ask::default() };
    let (window, _) = file.window_from(&ask).unwrap();
    assert_eq!((window.slow_mo, window.keep_open), (Duration::from_millis(50), KeepOpen::Never));

    let (window, control) = file.window_from(&Ask { headless: true, ..Ask::default() }).unwrap();
    assert_eq!(window, Window::default(), "--headless wins over the file, and drops its pace");
    assert!(control.is_none());
}

#[test]
fn no_window_is_the_default_and_its_options_need_the_window() {
    let none = Config::default();
    assert_eq!(none.window_from(&Ask::default()).unwrap().0, Window::default());
    for ask in [
        Ask { slow_mo: Some("500".into()), ..Ask::default() },
        Ask { keep_open: Some("always".into()), ..Ask::default() },
        Ask { step: true, ..Ask::default() },
        Ask { headed: true, headless: true, ..Ask::default() },
        Ask { headed: true, slow_mo: Some("meio segundo".into()), ..Ask::default() },
        Ask { headed: true, keep_open: Some("talvez".into()), ..Ask::default() },
    ] {
        assert!(none.window_from(&ask).is_err(), "{ask:?}");
    }
    assert!(Config::parse("window = \"janela\"").is_err());
    assert!(Config::parse("slow_mo = \"500\"").is_err());
    let (_, control) = none.window_from(&Ask { headed: true, step: true, ..Ask::default() }).unwrap();
    assert!(control.unwrap().is_paused(), "--step starts paused");
}

#[test]
fn a_computer_with_no_screen_is_told_so() {
    let none = graphic_session_in(|_| false);
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        assert!(none.is_ok());
    } else {
        assert!(none.unwrap_err().contains("sessão gráfica"));
        assert!(graphic_session_in(|name| name == "WAYLAND_DISPLAY").is_ok());
    }
}

#[test]
fn next_runs_exactly_one_block_and_stop_ends_the_run() {
    let control = Control::default();
    control.pause();
    control.next();
    assert!(control.turn(), "the one block asked for");
    let waiting = control.clone();
    let blocked = std::thread::spawn(move || waiting.turn());
    std::thread::sleep(Duration::from_millis(100));
    assert!(!blocked.is_finished(), "paused, no other block runs");
    control.stop();
    assert!(!blocked.join().unwrap(), "stopping ends the wait with no block");
    assert_eq!(steer(&control, "talvez"), None);
}

#[test]
fn slow_mo_is_cut_short_by_a_stop() {
    let control = Control::new(Duration::from_secs(30));
    let stopper = control.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        stopper.stop();
    });
    let started = std::time::Instant::now();
    control.present();
    assert!(started.elapsed() < Duration::from_secs(5));
}

static OPENED: AtomicUsize = AtomicUsize::new(0);

fn open(_: &str) -> Option<Result<Session, Box<Failure>>> {
    OPENED.fetch_add(1, Ordering::SeqCst);
    let screen = Text::new("Pronto").test_id("aviso");
    Some(Ok(Session::Here(Box::new(E2e::new(screen, Theme::raven_light())))))
}

#[test]
fn a_stopped_run_reports_what_did_not_run() {
    let scenario = Scenario::parse(
        "cenário: Parar\nabrir Tela | Abrir\nconferir #aviso = Pronto | Conferir\nconferir #aviso = Pronto | De novo\n",
    )
    .unwrap();
    let control = Control::default();
    control.pause();
    control.next();
    let stopper = control.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        stopper.stop();
    });
    let options = RunOptions { control: Some(control), ..RunOptions::default() };
    let report = run_with("Parar", std::slice::from_ref(&scenario), open, &options);
    assert_eq!(report.outcomes.len(), 1, "only the block the person let run");
    assert!(report.stopped && !report.passed());
    assert_eq!(report.not_run, vec!["Conferir".to_owned(), "De novo".to_owned()]);
    assert!(report.text().contains("parado por quem acompanhava"));
    assert_eq!(report.json()["stopped"], true);
    assert_eq!(report.json()["window"], "headless");
}
