//! Apps opened apart, in a child process over the test channel, such as Raven Finanças.

use std::path::PathBuf;

use raven_test_studio::apps::{is_older, open_finance_from, pick_installed};
use raven_test_studio::report::Report;
use raven_test_studio::runner::{Step, step_of};
use raven_test_studio::sample;

#[test]
fn an_installed_copy_from_before_the_channel_is_never_opened() {
    let copy = |path: &str, version: Option<&str>| (PathBuf::from(path), version.map(str::to_owned));
    // 0.1.38 would read --e2e-serve as the path of a database and open its window.
    assert_eq!(pick_installed([copy("/velho", Some("0.1.38"))], "0.1.39"), Err(Some("0.1.38".to_owned())));
    assert_eq!(
        pick_installed([copy("/velho", Some("0.1.38")), copy("/novo", Some("0.1.40"))], "0.1.39"),
        Ok(PathBuf::from("/novo"))
    );
    // A copy that does not say its version is not trusted with the option either.
    assert_eq!(pick_installed([copy("/mudo", None)], "0.1.39"), Err(None));
    assert!(is_older("0.1.9", "0.1.39") && !is_older("0.2.0", "0.1.39") && !is_older("0.1.39", "0.1.39"));
}

#[test]
fn the_finance_scenario_opens_apart_and_every_block_is_a_step() {
    let graph = sample::finance_scenario();
    assert!(sample::opens_apart(&graph));
    assert!(!sample::opens_apart(&sample::self_test()), "the self-test runs compiled in");
    let steps: Vec<Step> = graph.nodes.iter().map(|n| step_of(n).expect("a step")).collect();
    assert_eq!(steps.len(), 9);
    assert_eq!(steps[0], Step::Open(raven_test_studio::apps::FINANCE_APP.to_owned()));
}

#[test]
fn something_that_is_not_the_app_fails_to_open_with_a_reason() {
    // This test binary does not speak the channel: it rejects the option and ends.
    let this = std::env::current_exe().expect("this binary");
    let why = open_finance_from(&this).expect_err("not an app on the channel");
    assert!(why.starts_with("[E2E_APP_START]"), "{why}");
}

#[test]
fn a_report_of_an_app_apart_says_what_that_did_not_check() {
    let json = serde_json::json!({ "scenario": "x", "blocks": [], "total": 0, "opened_apart": true });
    let report = Report::from_json(&json).expect("a report");
    assert!(report.not_verified()[0].contains("à parte"), "{:?}", report.not_verified());
    assert_eq!(report.json()["opened_apart"], true);
}
