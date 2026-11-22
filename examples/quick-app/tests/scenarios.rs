//! The scenarios of `tests/e2e/`, played in this process on the same session the app serves
//! with `--e2e-serve`: what Raven Test Studio runs, with no channel and no other process.
#![allow(clippy::panic, reason = "a scenario that fails says why")]
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn every_scenario_of_tests_e2e_passes() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let scenarios = ravenrs_e2e_wire::check_folder(&folder, &quick_app::e2e::SCENARIOS)
        .unwrap_or_else(|why| panic!("{why}"));
    for scenario in scenarios {
        let mut session = quick_app::e2e::session();
        scenario.play(&mut session).unwrap_or_else(|why| panic!("{why}"));
    }
}
