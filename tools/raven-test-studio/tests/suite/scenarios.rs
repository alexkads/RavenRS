//! The scenarios of `tests/e2e/`, where a Studio tests another Studio, played in this process on
//! the same app the Studio serves with `--e2e-serve`. The Studio brings no scenario on the
//! channel (one installed from scratch has none), so they are read from the folder; the files in
//! `tests/e2e/amostras/` are samples the other tests run, some broken on purpose.

#[test]
fn every_scenario_of_tests_e2e_passes() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let mut files: Vec<_> = std::fs::read_dir(&folder)
        .expect("tests/e2e")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == ravenrs_e2e_wire::EXTENSION))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "{} has no scenario", folder.display());
    for file in files {
        let text = std::fs::read_to_string(&file).expect("the scenario");
        let scenario = ravenrs_e2e_wire::Scenario::parse(&text)
            .unwrap_or_else(|why| panic!("{}: {why}", file.display()));
        let mut session =
            ravenrs_e2e::E2e::new(raven_test_studio::served_app(), ravenrs::prelude::Theme::raven_dark())
                .viewport(ravenrs::prelude::Size::new(1440.0, 900.0), 1.0);
        scenario.play(&mut session).unwrap_or_else(|why| panic!("{}: {why}", file.display()));
    }
}
