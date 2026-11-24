//! A-03 (#895): `agent-remote` brings an agent up through `ssh` (here a fake one that runs the
//! other machine's command line on this one), the token goes on the input and never in an
//! argument, and the build runs on that agent.

use std::sync::Arc;
use std::time::Duration;

use raven_ci_core::agents::AgentTemplate;
use raven_ci_plugin_agent_remote::AgentRemotePlugin;
use raven_ci_plugin_api::BuildStatus;
use serde_json::json;

use crate::common::{Folder, Server, program};

#[test]
fn a_template_of_agent_remote_brings_an_agent_up_through_ssh_and_the_build_runs_there() {
    let server = Server::with("remoto", 0, Duration::from_secs(60), vec![Arc::new(AgentRemotePlugin::new())]);
    let there = Folder::new("remoto-la");
    let template = AgentTemplate {
        name: "mac".into(),
        labels: vec!["remoto".into()],
        launcher: "agent-remote".into(),
        settings: json!({
            "host": "maquina-falsa", "usuario": "ci", "porta": 2222,
            "ssh": env!("CARGO_BIN_EXE_raven-ci-ssh-falso"),
            "programa": program(),
            "pasta": there.0.join("agente").display().to_string(),
        }),
        executors: 1,
        max_agents: 1,
        idle_ms: 60_000,
    };
    server.ci.save_agent_template(&template).expect("the template");
    let pipeline = "[pipeline]\nagent = { label = \"remoto\" }\n\n[[stages]]\nname = \"Build\"\nsteps = [\"echo feito-la\"]\n";
    server.job("apps/site", pipeline);
    let number = server.request("apps/site");
    let status = server.wait_end("apps/site", number, Duration::from_secs(60));
    let log = server.log("apps/site", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}\n{:#?}", server.ci.log().lines());
    assert!(log.iter().any(|line| line == "feito-la"), "{log:#?}");
    assert!(
        there.0.join("agente").join(raven_ci_agent::KEY_FILE).is_file(),
        "the agent's folder is the one there"
    );
    let agents = server.ci.agents().expect("agents");
    assert_eq!(agents[0].launched_by.as_deref(), Some("mac"));
}

/// The same through the citation of `cmd.exe` (#906): a template with `sistema = "windows"` and a
/// folder with a space and a quote in its name reaches the agent whole.
#[test]
fn a_template_for_windows_quotes_for_cmd_and_the_agent_gets_its_words_whole() {
    let server = Server::with("remoto-win", 0, Duration::from_secs(60), vec![Arc::new(AgentRemotePlugin::new())]);
    let there = Folder::new("remoto-win-la");
    let folder = there.0.join("pasta do \"agente\"");
    let template = AgentTemplate {
        name: "win".into(),
        labels: vec!["win".into()],
        launcher: "agent-remote".into(),
        settings: json!({
            "host": "win11", "sistema": "windows",
            "ssh": env!("CARGO_BIN_EXE_raven-ci-ssh-falso"),
            "programa": program(),
            "pasta": folder.display().to_string(),
        }),
        executors: 1,
        max_agents: 1,
        idle_ms: 60_000,
    };
    server.ci.save_agent_template(&template).expect("the template");
    let pipeline = "[pipeline]\nagent = { label = \"win\" }\n\n[[stages]]\nname = \"Build\"\nsteps = [\"echo feito-no-windows\"]\n";
    server.job("apps/win", pipeline);
    let number = server.request("apps/win");
    let status = server.wait_end("apps/win", number, Duration::from_secs(60));
    let log = server.log("apps/win", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}\n{:#?}", server.ci.log().lines());
    assert!(folder.join(raven_ci_agent::KEY_FILE).is_file(), "the folder with a space and a quote, whole");
}
