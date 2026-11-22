//! A-04 (#896): the limits of an agent. A build that writes more than the disk allows and one
//! whose agent passes the memory allowed are cancelled saying why; the cores cap the executors and
//! reach the steps.

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

/// Runs `steps` on an agent with `limits`; the build as it ended, and its log.
fn run(name: &str, steps: &[String], limits: &[&str]) -> (raven_ci_core::Build, Vec<String>, Server) {
    let server = Server::start(name, 0, Duration::from_secs(60));
    server.job("apps/site", &pipeline(steps));
    let number = server.request("apps/site");
    let token = server.ci.agent_token(&format!("{name}-1"), &["teste".to_owned()], 1).expect("a token");
    let dir = Folder::new(&format!("{name}-agente"));
    let mut extra = vec!["--vigia-ms", "100"];
    extra.extend_from_slice(limits);
    let _agent = Agent::start(&server, &format!("{name}-1"), &dir.0, "teste", Some(&token), &extra);
    server.wait_end("apps/site", number, Duration::from_secs(60));
    let build = server.ci.build("apps/site", number).expect("build");
    let log = server.log("apps/site", number);
    (build, log, server)
}

#[test]
fn a_build_that_writes_more_than_the_disk_allows_is_cancelled_saying_how_much() {
    let write = if cfg!(windows) {
        "fsutil file createnew grande.bin 3000000".to_owned()
    } else {
        "head -c 3000000 /dev/zero > grande.bin".to_owned()
    };
    let (build, log, _server) = run("limite-disco", &[write, sleep_step(10)], &["--disco", "1M"]);
    assert_eq!(build.status, BuildStatus::Cancelled, "{build:?}\n{log:#?}");
    assert!(
        build.reason.as_deref().is_some_and(|r| r.contains("passou do limite de disco de 1,0 MB")),
        "{build:?}"
    );
    assert!(log.iter().any(|l| l.contains("passou do limite de disco")), "{log:#?}");
}

#[test]
fn a_build_whose_agent_passes_the_memory_allowed_is_cancelled() {
    // The agent alone holds more than 5 MB, so the watch has something to stop at once.
    let (build, log, _server) = run("limite-memoria", &[sleep_step(10)], &["--memoria", "5M"]);
    assert_eq!(build.status, BuildStatus::Cancelled, "{build:?}\n{log:#?}");
    assert!(
        build.reason.as_deref().is_some_and(|r| r.contains("passou do limite de memória de 5,0 MB")),
        "{build:?}"
    );
}

#[test]
fn the_cores_cap_the_executors_and_reach_the_steps() {
    let show = if cfg!(windows) {
        "echo nucleos=%RAVEN_CI_NUCLEOS% cargo=%CARGO_BUILD_JOBS% make=%MAKEFLAGS%".to_owned()
    } else {
        "echo nucleos=$RAVEN_CI_NUCLEOS cargo=$CARGO_BUILD_JOBS make=$MAKEFLAGS".to_owned()
    };
    let (build, log, server) = run("limite-nucleos", &[show], &["--nucleos", "2", "--executores", "4"]);
    assert_eq!(build.status, BuildStatus::Succeeded, "{log:#?}");
    assert!(log.iter().any(|l| l == "nucleos=2 cargo=2 make=-j2"), "{log:#?}");
    let agents = server.ci.agents().expect("agents");
    let hello = agents[0].hello.as_ref().expect("a hello");
    assert_eq!(hello.executors, 2, "no more executors than cores");
    assert_eq!(hello.limits["nucleos"], 2);
}
