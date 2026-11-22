//! The scenarios of `tests/e2e/`, played in this process on the same session the tool serves
//! with `--e2e-serve`, and what they saved checked on disk, in the folder of the session.

#[test]
fn every_scenario_of_tests_e2e_passes_and_what_it_saves_is_on_disk() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let scenarios =
        ravenrs_e2e_wire::check_folder(&folder, &raven_code::SCENARIOS).unwrap_or_else(|why| panic!("{why}"));
    for scenario in scenarios {
        let mut session = raven_code::e2e_session("cenario").unwrap_or_else(|why| panic!("{why}"));
        scenario.play(&mut session).unwrap_or_else(|why| panic!("{why}"));
    }
    let saved = std::env::temp_dir()
        .join(format!("raven-code-e2e-{}-cenario", std::process::id()))
        .join("src/main.rs");
    let text = std::fs::read_to_string(&saved).unwrap_or_else(|why| panic!("{}: {why}", saved.display()));
    assert_eq!(text.trim_end(), r#"fn main() { println!("Ola, RTS"); }"#, "Save all wrote the edited text");
}
