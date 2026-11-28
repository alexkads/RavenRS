//! No agent is left running on its own (#908): an agent dropped by its test, even while it tries
//! to reach a server that is gone, is gone when the drop returns.

use std::time::{Duration, Instant};

use crate::common::{Agent, Folder, Server};

/// Whether the process `pid` still runs.
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().is_ok_and(|s| s.success())
}

#[cfg(unix)]
#[test]
fn an_agent_trying_to_reach_a_server_that_is_gone_is_gone_once_dropped() {
    let server = Server::start("orfao", 1, Duration::from_secs(60));
    let token = server.ci.agent_token("orfao-1", &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new("orfao-agente");
    let agent = Agent::start(&server, "orfao-1", &dir.0, "teste", Some(&token), &[]);
    let until = Instant::now() + Duration::from_secs(30);
    while !server.connected("orfao-1") {
        assert!(Instant::now() < until, "the agent connected");
        std::thread::sleep(Duration::from_millis(50));
    }
    let pid = agent.pid();
    // The server goes first: the agent now tries for ever to reach it again.
    drop(server);
    std::thread::sleep(Duration::from_millis(300));
    assert!(alive(pid), "still running, trying to reconnect");
    drop(agent);
    assert!(!alive(pid), "the drop waited for the agent to be gone");
}
