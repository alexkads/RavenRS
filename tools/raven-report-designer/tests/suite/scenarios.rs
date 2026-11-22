//! The scenarios of `tests/e2e/`, played in this process on the same session the tool serves
//! with `--e2e-serve`: what Raven Test Studio runs, with no channel and no other process.

#[test]
fn every_scenario_of_tests_e2e_passes() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let scenarios = ravenrs_e2e_wire::check_folder(&folder, &raven_report_designer::SCENARIOS)
        .unwrap_or_else(|why| panic!("{why}"));
    for scenario in scenarios {
        let mut session =
            ravenrs_e2e::E2e::new(raven_report_designer::app(), ravenrs::prelude::Theme::raven_dark())
                .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
        scenario.play(&mut session).unwrap_or_else(|why| panic!("{why}"));
    }
}
