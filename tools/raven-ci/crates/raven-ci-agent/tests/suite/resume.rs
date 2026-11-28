//! A-03 (#895): an agent killed in the middle of a step comes back with the same name and resumes
//! its build from the checkpoint of the queue; a server that stops answering for a while loses no
//! line; and an agent that does not come back leaves its build interrupted, and forgets it when
//! it comes back too late.

use std::time::Duration;

use raven_ci_plugin_api::BuildStatus;

use crate::common::{Agent, Folder, Server, sleep_step};

fn pipeline(steps: &[String]) -> String {
    let steps: Vec<String> = steps.iter().map(|s| format!("{s:?}")).collect();
    format!(
        "[pipeline]\nagent = {{ label = \"teste\" }}\n\n[[stages]]\nname = \"Build\"\nsteps = [{}]\n",
        steps.join(", ")
    )
}

fn step_done(server: &Server, job: &str, number: u64, step: usize) -> bool {
    server
        .ci
        .stages(job, number)
        .is_ok_and(|stages| stages[0].steps.get(step).is_some_and(|s| s.status == "done"))
}

#[test]
fn an_agent_killed_in_a_step_comes_back_and_resumes_its_build_without_running_what_ended() {
    let server = Server::start("retoma", 0, Duration::from_secs(60));
    let steps = ["echo primeiro-passo".to_owned(), sleep_step(4), "echo terceiro-passo".to_owned()];
    server.job("apps/site", &pipeline(&steps));
    let number = server.request("apps/site");
    let token = server.ci.agent_token("cai-1", &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new("retoma-agente");
    let mut agent = Agent::start(&server, "cai-1", &dir.0, "teste", Some(&token), &[]);
    assert!(
        server.wait(Duration::from_secs(30), |_| step_done(&server, "apps/site", number, 0)),
        "the first step ended"
    );
    std::thread::sleep(Duration::from_millis(700));
    agent.kill();
    assert_eq!(server.ci.build("apps/site", number).expect("build").status, BuildStatus::Running);

    // The same agent comes back: with its key now, no token.
    let _back = Agent::start(&server, "cai-1", &dir.0, "teste", None, &[]);
    let status = server.wait_end("apps/site", number, Duration::from_secs(60));
    let log = server.log("apps/site", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    assert_eq!(
        log.iter().filter(|l| *l == "primeiro-passo").count(),
        1,
        "what ended did not run again: {log:#?}"
    );
    assert!(log.iter().any(|l| l == "terceiro-passo"), "{log:#?}");
    assert!(log.iter().any(|l| l.starts_with("retomado em")), "{log:#?}");
}

#[test]
fn a_server_that_stops_answering_for_a_while_loses_no_line_and_repeats_none() {
    let mut server = Server::start("rede", 0, Duration::from_secs(60));
    let lines = if cfg!(windows) {
        "for /L %i in (1,1,6) do @(echo linha-%i & ping -n 2 127.0.0.1 > NUL)".to_owned()
    } else {
        "for i in 1 2 3 4 5 6; do echo linha-$i; sleep 1; done".to_owned()
    };
    server.job("apps/site", &pipeline(&[lines]));
    let number = server.request("apps/site");
    let token = server.ci.agent_token("rede-1", &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new("rede-agente");
    let _agent = Agent::start(&server, "rede-1", &dir.0, "teste", Some(&token), &[]);
    assert!(
        server.wait(Duration::from_secs(30), |ci| ci
            .build_log("apps/site", number, 0)
            .is_ok_and(|log| log.iter().any(|l| l.text == "linha-1"))),
        "the build started"
    );
    server.pause(Duration::from_secs(3));
    let status = server.wait_end("apps/site", number, Duration::from_secs(60));
    let log = server.log("apps/site", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    for n in 1..=6 {
        let line = format!("linha-{n}");
        assert_eq!(log.iter().filter(|l| **l == line).count(), 1, "{line} once: {log:#?}");
    }
}

#[test]
fn an_agent_that_does_not_come_back_leaves_its_build_interrupted_and_forgets_it_later() {
    let server = Server::start("some", 0, Duration::from_secs(2));
    server.job("apps/site", &pipeline(&["echo comecou".to_owned(), sleep_step(30)]));
    let number = server.request("apps/site");
    let token = server.ci.agent_token("some-1", &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new("some-agente");
    let mut agent = Agent::start(&server, "some-1", &dir.0, "teste", Some(&token), &[]);
    assert!(
        server.wait(Duration::from_secs(30), |_| step_done(&server, "apps/site", number, 0)),
        "it started"
    );
    agent.kill();
    let status = server.wait_end("apps/site", number, Duration::from_secs(30));
    let build = server.ci.build("apps/site", number).expect("build");
    assert_eq!(status, BuildStatus::Interrupted, "{build:?}");
    assert!(build.reason.as_deref().is_some_and(|r| r.contains("o agente some-1 sumiu")), "{build:?}");
    let workspace =
        dir.0.join("dados").join(raven_ci_core::WORKSPACES_DIR).join("apps/site").join(number.to_string());
    assert!(workspace.is_dir(), "the folder is still there while the agent is away");

    let _back = Agent::start(&server, "some-1", &dir.0, "teste", None, &[]);
    assert!(
        server.wait(Duration::from_secs(30), |_| !workspace.exists()),
        "the agent that came back forgot the build and removed its folder"
    );
    assert_eq!(
        server.ci.build("apps/site", number).expect("build").status,
        status,
        "and it stays as it ended"
    );
}

/// The lines an agent wrote while the server did not answer, and that were still waiting when
/// it was killed, reach the build's log when it comes back, once each (#966).
#[test]
fn the_lines_waiting_when_an_agent_was_killed_arrive_when_it_comes_back() {
    let mut server = Server::start("diario", 0, Duration::from_secs(60));
    // The first life writes v1 lines and sleeps; the one that takes the build up again finds the
    // mark in the folder and writes a v2 line.
    let step = if cfg!(windows) {
        "if exist marca (echo v2-linha-1) else (type nul > marca & echo v1-linha-1 & ping -n 3 127.0.0.1 > NUL & echo v1-linha-2 & echo v1-linha-3 & ping -n 60 127.0.0.1 > NUL)".to_owned()
    } else {
        "if [ -f marca ]; then echo v2-linha-1; else touch marca; echo v1-linha-1; sleep 2; echo v1-linha-2; echo v1-linha-3; sleep 60; fi".to_owned()
    };
    server.job("apps/site", &pipeline(&[step]));
    let number = server.request("apps/site");
    let token = server.ci.agent_token("diario-1", &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new("diario-agente");
    let mut agent = Agent::start(&server, "diario-1", &dir.0, "teste", Some(&token), &[]);
    assert!(
        server.wait(Duration::from_secs(30), |ci| ci
            .build_log("apps/site", number, 0)
            .is_ok_and(|log| log.iter().any(|l| l.text == "v1-linha-1"))),
        "the build started"
    );
    // The server stops answering; the next lines wait on the agent, which is killed with them.
    server.stop_serving();
    std::thread::sleep(Duration::from_secs(5));
    agent.kill();
    server.serve_again();

    let _back = Agent::start(&server, "diario-1", &dir.0, "teste", None, &[]);
    let status = server.wait_end("apps/site", number, Duration::from_secs(90));
    let log = server.log("apps/site", number);
    assert_eq!(status, BuildStatus::Succeeded, "{log:#?}");
    for line in ["v1-linha-1", "v1-linha-2", "v1-linha-3", "v2-linha-1"] {
        assert_eq!(log.iter().filter(|l| *l == line).count(), 1, "{line} once: {log:#?}");
    }
    assert!(
        log.iter().any(|l| l.contains("linhas que o agente não tinha mandado antes de cair chegaram agora")),
        "{log:#?}"
    );
}
