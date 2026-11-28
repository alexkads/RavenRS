//! The scenarios of `tests/e2e/`, played in this process on the same session the panel serves
//! with `--e2e-serve`: what Raven Test Studio runs, with no channel and no other process.
#![allow(clippy::panic, reason = "a scenario that fails says why")]
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn every_scenario_of_tests_e2e_passes() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let scenarios = ravenrs_e2e_wire::check_folder(&folder, &raven_ci_painel::e2e::SCENARIOS)
        .unwrap_or_else(|why| panic!("{why}"));
    for (n, scenario) in scenarios.into_iter().enumerate() {
        let dir = raven_ci_painel::e2e::folder(&format!("cenario-{n}"));
        let mut session = raven_ci_painel::e2e::session(&dir).unwrap_or_else(|why| panic!("{why}"));
        scenario.play(&mut session).unwrap_or_else(|why| panic!("{why}"));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The scenarios a plugin brings for its own screens, played as a test of the plugin's crate
/// does it: the demonstration's page Travas and tab Cobertura (#919).
#[test]
fn the_scenarios_a_plugin_brings_play_with_the_plugin_on() {
    let plugin: std::sync::Arc<dyn raven_ci_plugin_api::CiPlugin> =
        std::sync::Arc::new(raven_ci_painel::e2e::Demonstracao);
    let played = raven_ci_painel::e2e::play_plugin_scenarios(&plugin).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(played, 1);
    assert!(
        raven_ci_painel::e2e::all_scenarios().len() > raven_ci_painel::e2e::SCENARIOS.len(),
        "the tool sees the plugin's too"
    );
}
