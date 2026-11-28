//! The plugin docker (#891) against the machine's real engine: `rust:1` running `cargo
//! --version`, an image that does not exist failing at "buscar imagem", nothing left after a
//! build that fails or is cancelled, the files of the folder owned by the agent, and compose
//! waiting for health.
//!
//! They run only when asked, with `RAVEN_CI_TESTES_DOCKER=1`, so a `cargo xtask pr` never reaches an
//! engine on its own. Asked, each test is still skipped, saying so, when the machine has no
//! engine, or when the engine is another machine's (a context over `ssh://` or `tcp://`): a
//! remote engine does not see this machine's folders, so these run on the machine of the engine.
//!
//! Everything a test makes on the engine has the label `raven-ci-teste=<id>` besides the
//! plugin's own, and the test removes what has it when it ends: the engine may be shared.
#![allow(clippy::expect_used, clippy::print_stderr, reason = "tests fail loudly and say why they skip")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use raven_ci_plugin_api::{BuildContext, BuildScope, MemoryLog, ScopeGuard, enter};
use raven_ci_plugin_docker::{COMPOSE, CONTAINER, Compose, Container, Engine, LABEL, State};
use ravenrs_flows::{CancelToken, Catalog, Executor, FlowDefinition, NodeDef, NodeStatus, Plan, Run};
use serde_json::json;

/// The engine of this machine, or `None`, saying why the test is skipped.
fn engine() -> Option<Engine> {
    if std::env::var_os("RAVEN_CI_TESTES_DOCKER").is_none() {
        eprintln!("aviso: teste pulado, os testes reais de Docker rodam só com RAVEN_CI_TESTES_DOCKER=1");
        return None;
    }
    let engine = match Engine::find("") {
        Ok(engine) => engine,
        Err(why) => {
            eprintln!("aviso: teste pulado, {why}");
            return None;
        }
    };
    let host = std::env::var("DOCKER_HOST").ok().or_else(|| {
        // Read from the configuration only: asking the engine would already reach the other machine.
        engine.lines(["context", "inspect", "--format", "{{.Endpoints.docker.Host}}"]).into_iter().next()
    });
    if let Some(host) = host.filter(|h| h.starts_with("ssh://") || h.starts_with("tcp://")) {
        eprintln!("aviso: teste pulado, o motor {host} é de outra máquina e não vê as pastas desta");
        return None;
    }
    if !engine.ask(["info"]).is_ok_and(|o| o.end.success()) {
        eprintln!("aviso: teste pulado, o {} não responde (o motor está parado?)", engine.name());
        return None;
    }
    Some(engine)
}

struct Build {
    log: Arc<MemoryLog>,
    workspace: PathBuf,
    run: String,
    guard: Option<ScopeGuard>,
    engine: Option<Engine>,
}

/// The label of what a test makes on the engine.
const TEST_LABEL: &str = "raven-ci-teste";

impl Build {
    /// Ends the build: what it asked to undo at its end runs.
    fn end(&mut self) {
        self.guard.take();
    }
}

impl Drop for Build {
    fn drop(&mut self) {
        self.guard.take();
        // Whatever the test made and the plugin did not remove, by the test's label only.
        if let Some(engine) = &self.engine {
            let filter = format!("label={TEST_LABEL}={}", self.run);
            let containers = engine.lines(["ps", "-aq", "--filter", &filter]);
            if !containers.is_empty() {
                let _ = engine.ask(["rm", "-f", "-v"].into_iter().map(str::to_owned).chain(containers));
            }
            for kind in ["network", "volume"] {
                let found = engine.lines([kind, "ls", "-q", "--filter", &filter]);
                if !found.is_empty() {
                    let _ = engine.ask([kind, "rm"].into_iter().map(str::to_owned).chain(found));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

fn build(engine: &Engine, environment: &[(&str, &str)]) -> Build {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let run = format!("docker-teste-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst));
    // Under the home, which Docker Desktop and Podman machine share with their VM; /tmp may not be.
    let base = std::env::var_os("HOME").map_or_else(std::env::temp_dir, PathBuf::from);
    let workspace = base.join(".raven-ci-testes").join(&run);
    std::fs::create_dir_all(&workspace).expect("a folder");
    let log = Arc::new(MemoryLog::new());
    let environment: BTreeMap<String, String> =
        environment.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    let context =
        BuildContext { job: "site".into(), number: 1, workspace: workspace.clone(), run: run.clone() };
    let guard = enter(BuildScope::new(context, log.clone(), environment));
    Build { log, workspace, run, guard: Some(guard), engine: Some(engine.clone()) }
}

/// A step `container` of `build` in `image`, with `args` before the image.
fn container_with(build: &Build, image: &str, script: &str, args: &[&str]) -> NodeDef {
    let mut words = vec!["--label".to_owned(), format!("{TEST_LABEL}={}", build.run)];
    words.extend(args.iter().map(|a| (*a).to_owned()));
    NodeDef::new("passo", CONTAINER)
        .parameter("container", json!({ "image": image, "args": words }))
        .parameter("script", json!(script))
}

fn container(build: &Build, image: &str, script: &str) -> NodeDef {
    container_with(build, image, script, &[])
}

/// A compose file with `services` (YAML, two spaces in, `LABELS` where each service's labels
/// go), labelled for the test: its services and its network.
fn compose_file(build: &Build, services: &str) {
    let label = format!("{TEST_LABEL}: {}", build.run);
    let services = services.replace("LABELS", &format!("labels:\n      {label}"));
    let text = format!("services:\n{services}networks:\n  default:\n    labels:\n      {label}\n");
    std::fs::write(build.workspace.join("compose.yml"), text).expect("the compose file");
}

fn run_in(build: &Build, engine: &Engine, nodes: Vec<NodeDef>, token: Option<CancelToken>) -> Run {
    let state = State::with_engine(engine.clone());
    let mut catalog = Catalog::builtin();
    catalog.add(Container(state.clone()));
    catalog.add(Compose(state));
    let mut flow = FlowDefinition::new("site", "site").node(NodeDef::new("start", "manual"));
    let mut before = "start".to_owned();
    for node in nodes {
        let id = node.id.clone();
        flow = flow.node(node).connect(&before, "main", &id, "main");
        before = id;
    }
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let mut executor = Executor::new(&plan, &catalog).run_id(build.run.clone());
    if let Some(token) = token {
        executor = executor.cancel_cooperatively(token);
    }
    executor.run()
}

/// What the engine still has of `build`: containers and volumes with its label.
fn left(engine: &Engine, build: &Build) -> Vec<String> {
    let filter = format!("label={LABEL}={}", build.run);
    let mut left = engine.lines(["ps", "-aq", "--filter", &filter]);
    left.extend(engine.lines(["volume", "ls", "-q", "--filter", &filter]));
    left.extend(engine.lines(["network", "ls", "-q", "--filter", &filter]));
    left
}

#[test]
fn cargo_version_runs_inside_rust_1() {
    let Some(engine) = engine() else { return };
    let mut build = build(&engine, &[]);
    let run = run_in(&build, &engine, vec![container(&build, "rust:1", "cargo --version")], None);
    assert_eq!(
        run.nodes["passo"].status,
        NodeStatus::Done,
        "{:?}\n{:?}",
        run.nodes["passo"].error,
        build.log.texts()
    );
    assert!(build.log.texts().iter().any(|t| t.starts_with("cargo 1.")), "{:?}", build.log.texts());
    build.end();
    assert!(left(&engine, &build).is_empty(), "{:?}", left(&engine, &build));
}

#[test]
fn an_image_that_does_not_exist_fails_at_fetching_it_with_its_name() {
    let Some(engine) = engine() else { return };
    let build = build(&engine, &[]);
    let image = "raven-ci-teste/nao-existe:0";
    let run = run_in(&build, &engine, vec![container(&build, image, "true")], None);
    let part = &run.nodes["passo"];
    assert_eq!(part.status, NodeStatus::Failed);
    let error = part.error.clone().unwrap_or_default();
    assert!(error.starts_with("buscar imagem:") && error.contains(image), "{error}");
}

#[test]
fn a_cancelled_build_leaves_no_container_nor_volume() {
    let Some(engine) = engine() else { return };
    let mut build = build(&engine, &[]);
    let token = CancelToken::new();
    let cancel = token.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        cancel.cancel("cancelado pelo teste");
    });
    let began = Instant::now();
    // The anonymous volume stands for what a step leaves on the engine.
    let node = container_with(&build, "alpine:3", "sleep 60", &["-v", "/dados"]);
    let run = run_in(&build, &engine, vec![node], Some(token));
    canceller.join().expect("joined");
    assert_eq!(run.nodes["passo"].status, NodeStatus::Cancelled, "{:?}", run.nodes["passo"]);
    assert!(began.elapsed() < Duration::from_secs(40), "took {:?}", began.elapsed());
    build.end();
    assert!(left(&engine, &build).is_empty(), "{:?}", left(&engine, &build));
}

#[test]
fn a_failed_build_leaves_nothing_and_says_the_code() {
    let Some(engine) = engine() else { return };
    let mut build = build(&engine, &[]);
    let run = run_in(&build, &engine, vec![container(&build, "alpine:3", "exit 3")], None);
    assert!(
        run.nodes["passo"].error.as_deref().unwrap_or_default().contains("código 3"),
        "{:?}",
        run.nodes["passo"]
    );
    build.end();
    assert!(left(&engine, &build).is_empty(), "{:?}", left(&engine, &build));
}

#[test]
fn a_file_made_in_the_container_belongs_to_the_agent_and_secrets_stay_out_of_the_arguments() {
    let Some(engine) = engine() else { return };
    let build = build(&engine, &[("TOKEN", "tok-docker-123456")]);
    let run = run_in(
        &build,
        &engine,
        vec![container(
            &build,
            "alpine:3",
            "echo \"$TOKEN\" > feito.txt; id -u; cat /proc/1/cmdline | tr '\\0' ' '",
        )],
        None,
    );
    assert_eq!(
        run.nodes["passo"].status,
        NodeStatus::Done,
        "{:?}\n{:?}",
        run.nodes["passo"].error,
        build.log.texts()
    );
    let made = build.workspace.join("feito.txt");
    assert_eq!(std::fs::read_to_string(&made).expect("the file").trim(), "tok-docker-123456");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let owner = std::fs::metadata(&build.workspace).expect("the folder").uid();
        assert_eq!(std::fs::metadata(&made).expect("the file").uid(), owner, "a file of root in the folder");
    }
    let texts = build.log.texts().join("\n");
    assert!(!texts.contains("TOKEN=tok-docker"), "{texts}");
}

#[test]
fn compose_waits_for_health_and_the_container_reaches_the_service() {
    let Some(engine) = engine() else { return };
    let mut build = build(&engine, &[]);
    compose_file(
        &build,
        "  web:\n    image: busybox:1.36\n    LABELS\n    command: [\"sh\", \"-c\", \"mkdir -p /w && echo ok > /w/index.html && httpd -f -p 8080 -h /w\"]\n    healthcheck:\n      test: [\"CMD\", \"wget\", \"-q\", \"-O-\", \"http://localhost:8080/\"]\n      interval: 1s\n      retries: 20\n",
    );
    let up = NodeDef::new("servicos", COMPOSE).parameter("file", json!("compose.yml"));
    let test = container(&build, "busybox:1.36", "wget -q -O- http://web:8080/");
    let run = run_in(&build, &engine, vec![up, test], None);
    assert_eq!(
        run.nodes["servicos"].status,
        NodeStatus::Done,
        "{:?}\n{:?}",
        run.nodes["servicos"].error,
        build.log.texts()
    );
    assert_eq!(
        run.nodes["passo"].status,
        NodeStatus::Done,
        "{:?}\n{:?}",
        run.nodes["passo"].error,
        build.log.texts()
    );
    assert!(build.log.texts().iter().any(|t| t.trim() == "ok"), "{:?}", build.log.texts());
    build.end();
    let project = raven_ci_plugin_docker::project_name(&build.run);
    let filter = format!("label=com.docker.compose.project={project}");
    assert!(engine.lines(["ps", "-aq", "--filter", &filter]).is_empty(), "the project outlived the build");
}

#[test]
fn a_service_that_falls_as_it_starts_fails_the_wait_with_its_code() {
    let Some(engine) = engine() else { return };
    let mut build = build(&engine, &[]);
    compose_file(
        &build,
        "  web:\n    image: alpine:3\n    LABELS\n    command: [\"sh\", \"-c\", \"exit 127\"]\n",
    );
    let up = NodeDef::new("servicos", COMPOSE).parameter("file", json!("compose.yml"));
    let run = run_in(&build, &engine, vec![up], None);
    let error = run.nodes["servicos"].error.clone().unwrap_or_default();
    assert!(error.contains("o serviço web parou com o código 127"), "{error}\n{:?}", build.log.texts());
    build.end();
}

#[test]
fn an_unhealthy_service_fails_the_wait_with_its_name() {
    let Some(engine) = engine() else { return };
    let mut build = build(&engine, &[]);
    compose_file(
        &build,
        "  db:\n    image: alpine:3\n    LABELS\n    command: [\"sleep\", \"60\"]\n    healthcheck:\n      test: [\"CMD\", \"false\"]\n      interval: 1s\n      retries: 1\n",
    );
    let up = NodeDef::new("servicos", COMPOSE).parameter("file", json!("compose.yml"));
    let run = run_in(&build, &engine, vec![up], None);
    let error = run.nodes["servicos"].error.clone().unwrap_or_default();
    assert!(error.contains("o serviço db ficou unhealthy"), "{error}\n{:?}", build.log.texts());
    build.end();
}

#[test]
#[cfg(unix)]
fn an_agent_comes_up_in_a_container_with_the_words_of_the_contract_and_stop_removes_it() {
    use raven_ci_plugin_api::{AgentLauncher, LaunchRequest};
    use raven_ci_plugin_docker::DockerLauncher;

    let Some(engine) = engine() else { return };
    let build = build(&engine, &[]);
    // A raven-ci-agent stand-in until #893: it says what it got and waits, as an agent would.
    let fake = build.workspace.join("raven-ci-agent");
    std::fs::write(
        &fake,
        "#!/bin/sh\necho \"agente falso: $* token=$RAVEN_CI_AGENT_TOKEN\"\nexec sleep 300\n",
    )
    .expect("the fake");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).expect("runnable");
    }
    let request = LaunchRequest {
        build: Some("site#1".into()),
        labels: vec!["build:site#1".into()],
        settings: json!({
            "image": "alpine:3",
            "agent_binary": fake.display().to_string(),
            "args": ["--label", format!("raven-ci-teste={}", build.run)],
        }),
        server: "http://host.docker.internal:8080".into(),
        token: "tok-uso-unico".into(),
    };
    let agent = DockerLauncher(State::with_engine(engine.clone())).launch(&request).expect("it comes up");
    let name = agent.name();
    assert!(agent.alive(), "the agent {name} is not running");
    let said = engine.lines(["logs", &name]).join("\n");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut said = said;
    while !said.contains("agente falso") && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
        said = engine.ask(["logs", &name]).map(|o| o.stdout.join("\n")).unwrap_or_default();
    }
    let words = format!(
        "--servidor http://host.docker.internal:8080 --nome {name} --rotulos build:site#1 --executores 1"
    );
    assert!(said.contains(&words), "{said}");
    assert!(said.contains("token=tok-uso-unico"), "the token came by the environment: {said}");
    let inspected = engine.lines(["inspect", "--format", "{{json .Args}}", &name]).join(" ");
    assert!(!inspected.contains("tok-uso-unico"), "the token is not in the words: {inspected}");
    agent.stop();
    assert!(!agent.alive());
    assert!(
        engine.lines(["ps", "-aq", "--filter", &format!("name=^{name}$")]).is_empty(),
        "the container outlived stop"
    );
}

/// An agent that dies at once leaves its exit code and its last lines to be read, and nothing of
/// it stays once it is taken down (#934).
#[cfg(unix)]
#[test]
fn an_agent_that_dies_says_why_and_its_container_goes() {
    use raven_ci_plugin_api::{AgentLauncher, LaunchRequest};
    use raven_ci_plugin_docker::DockerLauncher;

    let Some(engine) = engine() else { return };
    let build = build(&engine, &[]);
    let dying = build.workspace.join("raven-ci-agent");
    std::fs::write(&dying, "#!/bin/sh\necho nao-achei-a-glibc >&2\nexit 7\n").expect("the script");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dying, std::fs::Permissions::from_mode(0o755)).expect("runnable");
    }
    let request = LaunchRequest {
        build: Some("site#2".into()),
        labels: vec!["build:site#2".into()],
        settings: json!({
            "image": "alpine:3",
            "agent_binary": dying.display().to_string(),
            "args": ["--label", format!("raven-ci-teste={}", build.run)],
        }),
        server: "http://host.docker.internal:8080".into(),
        token: "tok-uso-unico".into(),
    };
    let agent = DockerLauncher(State::with_engine(engine.clone())).launch(&request).expect("it starts");
    let deadline = Instant::now() + Duration::from_secs(20);
    while agent.alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    let words = agent.last_words().expect("its last words");
    assert!(words.contains("código 7") && words.contains("nao-achei-a-glibc"), "{words}");
    let name = agent.name();
    agent.stop();
    assert!(
        engine.lines(["ps", "-aq", "--filter", &format!("name=^{name}$")]).is_empty(),
        "the container outlived stop"
    );
}
