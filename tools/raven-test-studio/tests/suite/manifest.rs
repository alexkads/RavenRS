//! `tool.json` is what the RavenRS MCP server tells an AI about the Studio. These tests keep it
//! true: every scenario, block kind, flag and error code it names exists, and none is missing.

use serde_json::Value;

fn manifest() -> Value {
    serde_json::from_str(include_str!("../../tool.json")).expect("tool.json is JSON")
}

fn names<'a>(list: &'a Value, key: &str) -> Vec<&'a str> {
    list.as_array().expect("a list").iter().map(|item| item[key].as_str().expect("a text")).collect()
}

#[test]
fn the_manifest_names_every_scenario_and_kind_the_studio_has() {
    let tool = manifest();
    assert_eq!(tool["package"], "raven-test-studio");
    assert_eq!(tool["version"], raven_test_studio::VERSION, "tool.json follows the version in Cargo.toml");
    let bundled: Vec<&str> = raven_test_studio::SCENARIOS.iter().map(|(key, _)| *key).collect();
    assert_eq!(names(&tool["scenarios"], "name"), bundled);
    for (scenario, (_, title)) in
        tool["scenarios"].as_array().unwrap().iter().zip(raven_test_studio::SCENARIOS)
    {
        assert_eq!(scenario["title"], title);
    }
    let kinds: Vec<&str> = raven_test_studio::KINDS.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(names(&tool["blocks"], "kind"), kinds);
}

#[test]
fn every_flag_in_the_manifest_is_one_the_program_reads() {
    let main = include_str!("../../src/main.rs");
    let tool = manifest();
    for command in names(&tool["commands"], "command") {
        for flag in command.split_whitespace().filter(|word| word.starts_with("--") && word.len() > 2) {
            assert!(main.contains(&format!("\"{flag}\"")), "main.rs does not read {flag}");
        }
    }
}

#[test]
fn every_error_code_of_ravenrs_e2e_and_of_the_test_channel_is_explained() {
    let error = include_str!("../../../../crates/ravenrs-e2e/src/error.rs");
    let channel = include_str!("../../../../crates/ravenrs-e2e-wire/src/lib.rs");
    let codes: Vec<&str> = [error, channel]
        .into_iter()
        .flat_map(|source| source.split('"'))
        .filter(|part| part.starts_with("E2E_") && !part.contains(' '))
        .collect();
    assert!(!codes.is_empty());
    let tool = manifest();
    let explained = names(&tool["error_codes"], "code");
    for code in &codes {
        assert!(explained.contains(code), "tool.json does not explain {code}");
    }
    for code in &explained {
        assert!(codes.contains(code), "{code} is not a code of ravenrs-e2e or of the test channel");
    }
}

#[test]
fn the_report_fields_in_the_manifest_are_the_ones_the_report_writes() {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/e2e/quebrado.raventest");
    let report = raven_test_studio::run_named(file, None).expect("a scenario file");
    let json = report.json();
    let tool = manifest();
    for field in tool["report"]["fields"].as_array().unwrap() {
        assert!(json.get(field.as_str().unwrap()).is_some(), "the report has no {field}");
    }
    let block = &json["blocks"][0];
    for field in tool["report"]["block_fields"].as_array().unwrap() {
        assert!(block.get(field.as_str().unwrap()).is_some(), "a block has no {field}");
    }
}
