//! A-01 (#893): a build with a label runs on an agent of verdade, which got in with a token used
//! once, and the server's own executor leaves it; and the agent at rest holds little memory.

use std::time::Duration;

use raven_ci_plugin_api::BuildStatus;

use crate::common::{Agent, Folder, Server};

const PIPELINE: &str = r#"
[pipeline]
agent = { label = "teste" }

[[stages]]
name = "Build"
steps = ["echo um-do-agente", "echo dois-do-agente"]
"#;

#[test]
fn a_build_with_a_label_runs_on_the_agent_and_its_log_reaches_the_server() {
    let server = Server::start("roda", 1, Duration::from_secs(60));
    server.job("apps/site", PIPELINE);
    let number = server.request("apps/site");
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        server.ci.build("apps/site", number).expect("the build").status,
        BuildStatus::Queued,
        "the server's executor leaves a build that names an agent"
    );

    let token = server.ci.agent_token("teste-1", &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new("roda-agente");
    let _agent = Agent::start(&server, "teste-1", &dir.0, "teste", Some(&token), &[]);
    let status = server.wait_end("apps/site", number, Duration::from_secs(60));
    let log = server.log("apps/site", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    assert!(log.iter().any(|line| line == "um-do-agente"), "{log:#?}");
    assert!(log.iter().any(|line| line == "dois-do-agente"), "{log:#?}");
    assert!(
        log.iter().any(|line| line.contains(&dir.0.join("dados").display().to_string())),
        "the folder is the agent's: {log:#?}"
    );
    let stages = server.ci.stages("apps/site", number).expect("stages");
    assert!(stages[0].steps.iter().all(|step| step.status == "done"), "{stages:?}");
    assert!(dir.0.join(raven_ci_agent::KEY_FILE).is_file(), "the agent keeps its key");
    assert!(!server.ci.workspace("apps/site", number).exists(), "the server made no folder for it");

    // The token is spent: another agent with it is refused.
    let refused = server.ci.agent_join(&raven_ci_core::agents::Join {
        name: "teste-1".into(),
        token,
        hello: raven_ci_core::agents::Hello { session: "x".into(), ..Default::default() },
    });
    assert!(refused.is_err());
}

#[test]
fn an_agent_at_rest_holds_at_most_15_mb() {
    let server = Server::start("memoria", 0, Duration::from_secs(60));
    let token = server.ci.agent_token("parado-1", &["teste".to_owned()], 2).expect("a token");
    let dir = Folder::new("memoria-agente");
    let agent = Agent::start(&server, "parado-1", &dir.0, "teste", Some(&token), &["--executores", "2"]);
    assert!(server.wait(Duration::from_secs(30), |_| server.connected("parado-1")), "the agent connected");
    std::thread::sleep(Duration::from_secs(3));
    let Some(bytes) = ravenrs_metrics::rss_bytes_of(agent.pid()) else {
        eprintln!("pulado: este sistema não diz a memória de outro processo");
        return;
    };
    let mb = bytes as f64 / (1024.0 * 1024.0);
    eprintln!("o agente parado ocupa {mb:.1} MB");
    assert!(mb <= 15.0, "o agente parado ocupa {mb:.1} MB, mais que os 15 MB da meta");
}
