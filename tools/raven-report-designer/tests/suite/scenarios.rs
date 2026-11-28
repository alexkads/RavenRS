//! The scenarios of `tests/e2e/`, played on the tool opened with `--e2e-serve` in a process of
//! its own, as Raven Test Studio plays them; then the report they saved, reopened from the file
//! and in the designer again (#723).

use std::time::Duration;

/// The text `salvar.raventest` writes into the new label.
const WRITTEN: &str = "Conferido pelo RTS";

#[test]
fn every_scenario_passes_over_the_channel_and_the_saved_report_reopens() {
    let folder = std::env::temp_dir().join(format!("raven-report-designer-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let start = || {
        let mut process = std::process::Command::new(env!("CARGO_BIN_EXE_raven-report-designer"));
        process.arg(ravenrs_e2e_wire::SERVE_FLAG).env("RAVEN_REPORT_DESIGNER_E2E_DIR", &folder);
        ravenrs_e2e_wire::Client::spawn(process, Duration::from_secs(60))
            .unwrap_or_else(|f| panic!("{}", f.message))
    };
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ravenrs_e2e_wire::FOLDER);
    let brought: Vec<String> = start().scenarios().to_vec();
    let texts: Vec<&str> = brought.iter().map(String::as_str).collect();
    let scenarios = ravenrs_e2e_wire::check_folder(&here, &texts).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(scenarios.len(), raven_report_designer::SCENARIOS.len(), "the tool brings every scenario");
    for scenario in scenarios {
        let mut client = start();
        scenario.play_served(&mut client).unwrap_or_else(|why| panic!("{why}"));
        client.send(&ravenrs_e2e_wire::Command::Bye).ok();
    }

    // The file holds the report with the new label, by its name.
    let saved = folder.join("fatura.json");
    let json = std::fs::read_to_string(&saved).unwrap_or_else(|why| panic!("{}: {why}", saved.display()));
    ravenrs_report::Report::from_json(&json).unwrap_or_else(|why| panic!("the saved report reopens: {why}"));
    assert!(json.contains("\"rotulo1\""), "the label was saved with its name");
    assert!(json.contains(WRITTEN), "the label was saved with the text written");

    // Opened again on the same folder, the designer shows the saved copy, label and all.
    let reopen = ravenrs_e2e_wire::Scenario::parse(&format!(
        "scenario: Raven Report Designer: reabrir\n\
         open Raven Report Designer | Abrir o designer de novo\n\
         wait #parte:rotulo1 | O rótulo salvo está na faixa\n\
         click Visualizar | Ver a prévia\n\
         mention #page = {WRITTEN} | A prévia mostra o texto salvo\n"
    ))
    .unwrap_or_else(|why| panic!("{why}"));
    let mut client = start();
    reopen.play_served(&mut client).unwrap_or_else(|why| panic!("{why}"));
    client.send(&ravenrs_e2e_wire::Command::Bye).ok();
    let _ = std::fs::remove_dir_all(&folder);
}
