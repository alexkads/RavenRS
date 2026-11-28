//! A-02 (#894): with a template of `agent-local`, a server with no executor brings an agent up on
//! its own machine for a build, the agent gets in with its token used once, and goes when idle;
//! a template whose user cannot be reached leaves the build in the queue, saying why.

use std::sync::Arc;
use std::time::Duration;

use raven_ci_core::agents::AgentTemplate;
use raven_ci_plugin_agent_local::AgentLocalPlugin;
use raven_ci_plugin_api::BuildStatus;
use serde_json::json;

use crate::common::{Server, program};

const PIPELINE: &str = r#"
[pipeline]
agent = { label = "local" }

[[stages]]
name = "Build"
steps = ["echo feito-no-agente-local"]
"#;

fn template(settings: serde_json::Value) -> AgentTemplate {
    AgentTemplate {
        name: "local".into(),
        labels: vec!["local".into()],
        launcher: "agent-local".into(),
        settings,
        executors: 1,
        max_agents: 1,
        idle_ms: 1_500,
    }
}

#[test]
fn a_template_brings_an_agent_up_here_for_a_build_and_takes_it_down_when_idle() {
    let server = Server::with("local", 0, Duration::from_secs(60), vec![Arc::new(AgentLocalPlugin::new())]);
    server.ci.save_agent_template(&template(json!({ "programa": program() }))).expect("the template");
    server.job("apps/site", PIPELINE);
    let number = server.request("apps/site");
    let status = server.wait_end("apps/site", number, Duration::from_secs(60));
    let log = server.log("apps/site", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}\n{:#?}", server.ci.log().lines());
    assert!(log.iter().any(|line| line == "feito-no-agente-local"), "{log:#?}");
    let agents = server.ci.agents().expect("agents");
    assert_eq!(agents.len(), 1, "{agents:?}");
    assert_eq!(agents[0].launched_by.as_deref(), Some("local"));
    assert!(agents[0].name.starts_with("local-"));

    assert!(
        server.wait(Duration::from_secs(20), |ci| ci.agents().expect("agents").is_empty()),
        "the idle agent was taken down: {:#?}",
        server.ci.log().lines()
    );
    assert!(server.ci.log().lines().iter().any(|line| line.contains("ficou ocioso")));
    // Its folder went with it (#938): a later agent of the template has another name.
    let lines = server.ci.log().lines();
    let said =
        lines.iter().find(|line| line.contains("saiu com ele")).unwrap_or_else(|| panic!("{lines:#?}"));
    let folder =
        said.split("a pasta ").nth(1).and_then(|rest| rest.split(" saiu").next()).expect("the folder");
    assert!(!std::path::Path::new(folder).exists(), "{said}");
}

#[test]
fn a_template_whose_user_cannot_be_reached_leaves_the_build_waiting_and_says_why() {
    let server = Server::with("usuario", 0, Duration::from_secs(60), vec![Arc::new(AgentLocalPlugin::new())]);
    let settings = json!({ "programa": program(), "usuario": "raven-ci-ninguem-assim" });
    server.ci.save_agent_template(&template(settings)).expect("the template");
    server.job("apps/site", PIPELINE);
    let number = server.request("apps/site");
    assert!(
        server.wait(Duration::from_secs(20), |ci| {
            ci.log()
                .lines()
                .iter()
                .any(|line| line.contains("o processo dele acabou") || line.contains("subir um agente"))
        }),
        "{:#?}",
        server.ci.log().lines()
    );
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(server.ci.build("apps/site", number).expect("build").status, BuildStatus::Queued);
    let launches = server.ci.log().lines().iter().filter(|line| line.contains("subindo pelo modelo")).count();
    assert!(launches <= 1, "a failed template waits before it tries again: {launches} launches");
}
