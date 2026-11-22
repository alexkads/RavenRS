//! The scenarios of `tests/e2e/`, played on the app opened with `--e2e-serve` in a process of its
//! own, over the test channel, as Raven Test Studio plays them: the app is only a binary, so its
//! tests cannot build its screen here.
#![allow(clippy::panic, clippy::expect_used, reason = "a scenario that fails says why")]

use std::time::Duration;

#[test]
fn every_scenario_the_app_brings_passes_over_the_channel() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let start = || {
        let mut process = std::process::Command::new(env!("CARGO_BIN_EXE_grid-poc"));
        process.arg(ravenrs_e2e_wire::SERVE_FLAG);
        ravenrs_e2e_wire::Client::spawn(process, Duration::from_secs(60))
            .unwrap_or_else(|f| panic!("{}", f.message))
    };
    let brought: Vec<String> = start().scenarios().to_vec();
    let texts: Vec<&str> = brought.iter().map(String::as_str).collect();
    let scenarios = ravenrs_e2e_wire::check_folder(&folder, &texts).unwrap_or_else(|why| panic!("{why}"));
    for scenario in scenarios {
        let mut client = start();
        scenario.play_served(&mut client).unwrap_or_else(|why| panic!("{why}"));
    }
}
