//! Handing a scenario to a coding agent, with no agent spent: what it is asked, how its answer is
//! read, and that a missing CLI or an unknown scenario says so instead of running.

use std::path::Path;

use raven_test_studio::delegate::{agent_named, delegate, mission, read_answer};
use ravenrs_agent_cli::{Agent, Cli};

#[test]
fn the_mission_runs_the_studio_reads_only_and_asks_for_json() {
    let asked = mission(Path::new("/bin/raven-test-studio"), "quebrado", Path::new("/src/RavenRS"));
    assert!(asked.contains("\"/bin/raven-test-studio\" --run quebrado --format json --no-cache"), "{asked}");
    assert!(asked.contains("/src/RavenRS"));
    assert!(asked.contains("Não altere nenhum arquivo"));
    assert!(asked.contains("\"verdict\""));
}

#[test]
fn the_answer_is_read_from_a_fence_or_from_words_around_it() {
    let fenced = "Pronto.\n```json\n{\"verdict\": \"failed\", \"code\": \"E2E_NOT_FOUND\"}\n```";
    let said = read_answer(fenced).expect("read");
    assert_eq!(said["code"], "E2E_NOT_FOUND");
    assert_eq!(read_answer("{\"verdict\": \"passed\"}").expect("read")["verdict"], "passed");
    assert!(read_answer("passou, tudo certo").is_none(), "no JSON, no answer");
    assert!(read_answer("{\"verdict\": \"talvez\"}").is_none(), "only passed or failed");
}

#[test]
fn only_codex_and_claude_take_a_scenario() {
    assert_eq!(agent_named("codex"), Some(Agent::Codex));
    assert_eq!(agent_named(" Claude "), Some(Agent::Claude));
    assert_eq!(agent_named("gemini"), None);
}

#[test]
fn a_missing_agent_or_an_unknown_scenario_says_so_and_runs_nothing() {
    let missing = Cli::new(Agent::Codex).program("/nowhere/codex");
    let why = delegate(&missing, "autoteste", Some("gpt-luna".into())).expect_err("no CLI");
    assert!(why.contains("codex") && why.contains("not installed"), "{why}");
    let why = delegate(&missing, "nada-disso", None).expect_err("no scenario");
    assert!(why.contains("autoteste, financas"), "{why}");
}
