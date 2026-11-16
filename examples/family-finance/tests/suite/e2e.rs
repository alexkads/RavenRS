//! The app on the test channel, as Raven Test Studio drives it: in this process through the
//! same commands, and as a child process with no window.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::time::Duration;

use family_finance::e2e;
use ravenrs_e2e_wire::{Client, Command, Scenario};

/// The scenarios the app brings, read from the files it embeds; the folder must match.
fn scenarios() -> Vec<Scenario> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    ravenrs_e2e_wire::check_folder(&folder, &e2e::SCENARIOS).unwrap_or_else(|why| panic!("{why}"))
}

/// The commands of a scenario after its Open block, which the tool does itself.
fn commands(scenario: &Scenario) -> Vec<Command> {
    assert_eq!(scenario.app(), e2e::APP_NAME, "{} opens this app", scenario.name);
    scenario.blocks.iter().filter_map(|b| b.command()).collect()
}

#[test]
fn every_scenario_of_tests_e2e_passes_in_this_process() {
    for scenario in scenarios() {
        let mut session = e2e::session().unwrap();
        for command in commands(&scenario) {
            if let Err(f) = command.apply(&mut session) {
                panic!("{}: {command:?} failed: {f}\n{}", scenario.name, f.screen);
            }
        }
    }
}

#[test]
fn a_missing_target_says_so_with_the_screen_of_the_app() {
    let mut session = e2e::session().unwrap();
    let failure = Command::Click { target: "#nao-existe".into() }.apply(&mut session).unwrap_err();
    assert_eq!(failure.code, "E2E_NOT_FOUND");
    assert!(failure.screen.contains("Lançamentos"), "the screen of Raven Finanças: {}", failure.screen);
}

/// A folder of its own, emptied, that stands for the home of the person using the app.
fn empty_home(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("raven-financas-e2e-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn the_studio_drives_the_app_in_a_child_process_without_touching_its_data() {
    let home = empty_home("filho");
    let mut process = Process::new(env!("CARGO_BIN_EXE_family-finance"));
    process
        .arg(e2e::SERVE_FLAG)
        .current_dir(&home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", &home)
        .env("XDG_DATA_HOME", &home);
    let mut app = Client::spawn(process, Duration::from_secs(60)).unwrap();
    assert_eq!(app.app(), e2e::APP_NAME);
    assert_eq!(app.scenarios(), e2e::SCENARIOS, "the hello brings the app's scenarios");
    for scenario in scenarios() {
        for command in commands(&scenario) {
            if let Err(f) = app.send(&command) {
                panic!("{}: {command:?} failed: {f}\n{}", scenario.name, f.screen);
            }
        }
    }
    let missing = app.send(&Command::Click { target: "#nao-existe".into() }).unwrap_err();
    assert_eq!(missing.code, "E2E_NOT_FOUND");
    drop(app);
    let left: Vec<_> = std::fs::read_dir(&home).unwrap().flatten().map(|e| e.path()).collect();
    assert!(left.is_empty(), "the app wrote nothing where the person keeps data: {left:?}");
    let _ = std::fs::remove_dir_all(&home);
}
