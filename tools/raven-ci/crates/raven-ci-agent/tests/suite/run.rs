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
fn an_agent_at_rest_holds_at_most_15_mb_built_for_release() {
    let server = Server::start("memoria", 0, Duration::from_secs(60));
    let token = server.ci.agent_token("parado-1", &["teste".to_owned()], 2).expect("a token");
    let dir = Folder::new("memoria-agente");
    let agent = Agent::start(&server, "parado-1", &dir.0, "teste", Some(&token), &["--executores", "2"]);
    assert!(server.wait(Duration::from_secs(30), |_| server.connected("parado-1")), "the agent connected");
    // At rest: past the peak of starting, which the system takes a few seconds to give back.
    std::thread::sleep(Duration::from_secs(8));
    // The median of five looks, half a second apart: one look may fall on a hello on its way.
    let mut looks = Vec::new();
    for _ in 0..5 {
        match ravenrs_metrics::rss_bytes_of(agent.pid()) {
            Some(bytes) => looks.push(bytes),
            None => {
                eprintln!("pulado: este sistema não diz a memória de outro processo");
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    looks.sort_unstable();
    let bytes = looks[2];
    eprintln!("as leituras: {:?} MB", looks.iter().map(|b| b / (1024 * 1024)).collect::<Vec<_>>());
    let mb = bytes as f64 / (1024.0 * 1024.0);
    // The goal is the program people run, built for release. A build for debugging holds more
    // code and moves more with the load of the other tests: there the test only guards against a
    // large regression, and says the number.
    let most = if cfg!(debug_assertions) { 25.0 } else { 15.0 };
    eprintln!("o agente parado ocupa {mb:.1} MB (limite deste teste: {most} MB)");
    assert!(mb <= most, "o agente parado ocupa {mb:.1} MB, mais que os {most} MB");
}

/// A credential named by a step reaches the command on the agent through the server, which
/// audits the read, and the log the server keeps hides it (#922).
#[test]
fn a_step_on_the_agent_gets_its_credential_from_the_server_and_the_log_hides_it() {
    let server = Server::with(
        "credencial",
        1,
        Duration::from_secs(60),
        vec![std::sync::Arc::new(raven_ci_plugin_credentials::CredentialsPlugin)],
    );
    server
        .ci
        .save_credential("feed", "token", &serde_json::json!({ "token": "npm_do_agente_123456" }), None, None)
        .expect("saved");
    let pipeline = r#"
[pipeline]
agent = { label = "teste" }

[[stages]]
name = "Usa"
credentials = { TOKEN = "feed" }
steps = ['test "$TOKEN" = npm_do_agente_123456 && echo token $TOKEN']
"#;
    server.job("apps/feed", pipeline);
    let number = server.request("apps/feed");
    let token = server.ci.agent_token("teste-c", &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new("credencial-agente");
    let _agent = Agent::start(&server, "teste-c", &dir.0, "teste", Some(&token), &[]);
    let status = server.wait_end("apps/feed", number, Duration::from_secs(60));
    let log = server.log("apps/feed", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    assert!(log.iter().any(|line| line == "token ****"), "{log:#?}");
    assert!(!log.join("\n").contains("npm_do_agente_123456"), "{log:#?}");
    let audit = server.ci.store().audit_of("agente:teste-c").expect("the audit");
    assert!(audit.iter().any(|entry| entry.what == "ler o segredo feed" && entry.granted), "{audit:#?}");
}
