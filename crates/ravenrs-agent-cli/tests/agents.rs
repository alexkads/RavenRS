//! The agent nodes through a fake CLI that spends nothing: the answer and its events, reading
//! only by default, a missing CLI, a limit the installed one does not keep refused before
//! running, a secret taken out, a session resumed, and a node of a flow.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use ravenrs_agent_cli::{
    Agent, AgentError, AgentEvent, Cli, Policy, Probe, cheapest_codex_model_in, cheapest_model, with_agents,
};
use ravenrs_flows::{Catalog, Executor, FlowDefinition, NodeDef, Plan};
use serde_json::json;

const FAKE: &str = env!("CARGO_BIN_EXE_ravenrs-agent-cli-fake");

fn never() -> bool {
    false
}

#[test]
fn codex_answers_with_its_final_message_and_reads_only_with_no_setup() {
    let cli = Cli::new(Agent::Codex).program(FAKE);
    assert!(matches!(cli.probe(), Probe::Ready { ref version, .. } if version == "fake-agent 1.2.3"));
    let answer = cli.run("liste os arquivos", &Policy::default(), None, &never).expect("runs");
    assert_eq!(answer.final_message.as_deref(), Some("Done: liste os arquivos"));
    assert_eq!(answer.session.as_deref(), Some("t-1"));
    assert!(answer.success);
    let said = format!("{:?}", answer.events);
    assert!(said.contains("--sandbox read-only"), "reading only by default: {said}");
    assert!(answer.events.contains(&AgentEvent::Tool { name: "command".into(), detail: "ls".into() }));
    assert_eq!(answer.cost_usd, None, "Codex does not say what it cost: unknown, not zero");
}

#[test]
fn claude_answers_with_its_result_cost_and_session_and_resumes_it() {
    let cli = Cli::new(Agent::Claude).program(FAKE);
    let answer = cli.run("revise", &Policy::default(), Some("c-0"), &never).expect("runs");
    assert_eq!(answer.final_message.as_deref(), Some("Done: revise"));
    assert_eq!(answer.cost_usd, Some(0.0123));
    let said = format!("{:?}", answer.events);
    assert!(said.contains("--permission-mode plan"), "{said}");
    assert!(said.contains("--resume c-0"), "the session resumed");
}

#[test]
fn a_missing_cli_says_so_and_a_limit_an_old_one_does_not_keep_is_refused_before_running() {
    let missing = Cli::new(Agent::Codex).program("/nowhere/codex");
    assert_eq!(missing.probe(), Probe::Missing);
    assert_eq!(missing.run("x", &Policy::default(), None, &never), Err(AgentError::Missing(Agent::Codex)));
    assert!(AgentError::Missing(Agent::Codex).to_string().contains("not installed"));

    let old = Cli::new(Agent::Codex).program(FAKE).env("FAKE_OLD", "1");
    let refused = old.run("x", &Policy::default(), None, &never);
    assert!(matches!(refused, Err(AgentError::Refused(ref why)) if why.contains("sandbox")), "{refused:?}");
    let tools = Policy { allowed_tools: vec!["Read".into()], ..Policy::default() };
    let codex = Cli::new(Agent::Codex).program(FAKE);
    assert!(
        matches!(codex.run("x", &tools, None, &never), Err(AgentError::Refused(_))),
        "Codex keeps no list of tools"
    );
}

#[test]
fn a_model_reaches_the_cli_and_an_old_one_that_cannot_choose_refuses_it() {
    let cheap = Policy { model: Some("gpt-luna".into()), ..Policy::default() };
    let codex = Cli::new(Agent::Codex).program(FAKE);
    let said = format!("{:?}", codex.run("x", &cheap, None, &never).expect("runs").events);
    assert!(said.contains("--model gpt-luna"), "{said}");
    let claude = Cli::new(Agent::Claude).program(FAKE);
    let said = format!("{:?}", claude.run("x", &cheap, None, &never).expect("runs").events);
    assert!(said.contains("--model gpt-luna"), "{said}");

    let old = Cli::new(Agent::Claude).program(FAKE).env("FAKE_OLD", "1");
    let refused = old.run("x", &cheap, None, &never);
    assert!(matches!(refused, Err(AgentError::Refused(_))), "{refused:?}");
}

#[test]
fn the_cheapest_model_is_read_from_the_list_codex_keeps() {
    let list = json!({ "models": [
        { "slug": "frontier", "description": "Frontier intelligence.", "visibility": "list", "priority": 1 },
        { "slug": "hidden-cheap", "description": "Fast and affordable.", "visibility": "hide", "priority": 2 },
        { "slug": "old-efficient", "description": "Older fast and efficient model.", "visibility": "list", "priority": 9 },
        { "slug": "luna", "description": "Fast and affordable model for easier tasks.", "visibility": "list", "priority": 6 },
    ] });
    assert_eq!(cheapest_codex_model_in(&list.to_string()).as_deref(), Some("luna"));
    let no_word = json!({ "models": [ { "slug": "only", "description": "Frontier.", "priority": 1 } ] });
    assert_eq!(cheapest_codex_model_in(&no_word.to_string()), None);
    assert_eq!(cheapest_codex_model_in("not json"), None);
    assert_eq!(cheapest_model(Agent::Claude).as_deref(), Some("haiku"));
}

#[test]
fn a_secret_given_to_the_cli_never_shows_in_the_events_kept() {
    let cli = Cli::new(Agent::Codex).program(FAKE).env("API_TOKEN_TEST", "s3cr3t-value-123");
    let answer = cli.run("x", &Policy::default(), None, &never).expect("runs");
    let kept = serde_json::to_string(&answer).expect("JSON");
    assert!(!kept.contains("s3cr3t-value-123"), "{kept}");
    assert!(kept.contains("token: •••"), "taken out, where it was");
}

#[test]
fn a_node_of_a_flow_calls_the_agent_with_its_prompt_and_gives_the_answer() {
    let fake = || Cli::new(Agent::Codex).program(FAKE);
    let catalog = with_agents(Catalog::builtin(), fake(), Cli::new(Agent::Claude).program(FAKE), fake());
    let flow = FlowDefinition::new("f", "Agente")
        .node(NodeDef::new("inicio", "manual").parameter("items", json!([{ "pasta": "src" }])))
        .node(NodeDef::new("listar", "codex").parameter("prompt", json!("Liste {{ pasta }}")))
        .node(
            NodeDef::new("revisar", "code_review")
                .parameter("prompt", json!("o diff"))
                .parameter("read_only", json!(false)),
        )
        .connect("inicio", "main", "listar", "main")
        .connect("listar", "main", "revisar", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).run();
    let listed = &run.output("listar", "main")[0].json;
    assert_eq!(listed["final_message"], json!("Done: Liste src"));
    let reviewed = &run.output("revisar", "main")[0].json;
    assert!(reviewed["final_message"].as_str().is_some_and(|m| m.contains("Não altere nenhum arquivo")));
    assert!(reviewed.to_string().contains("--sandbox read-only"), "a review reads only, whatever it is told");

    let failing = FlowDefinition::new("g", "Falha")
        .node(NodeDef::new("inicio", "manual"))
        .node(NodeDef::new("a", "codex").parameter("prompt", json!("FAIL")))
        .connect("inicio", "main", "a", "main");
    let plan = Plan::compile(&failing, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).run();
    assert!(!run.output("a", "main").is_empty(), "it answered");
    assert_eq!(run.output("a", "main")[0].json["success"], json!(false), "with the failure in its events");
}
