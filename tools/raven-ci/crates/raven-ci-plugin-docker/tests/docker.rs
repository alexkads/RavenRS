//! The plugin docker (#891) against the machine's real engine: `rust:1` running `cargo
//! --version`, an image that does not exist failing at "buscar imagem", nothing left after a
//! build that fails or is cancelled, the files of the folder owned by the agent, and compose
//! waiting for health.
//!
//! Each test is skipped, saying so, when the machine has no engine, or when the engine is
//! another machine's (a context over `ssh://` or `tcp://`): a remote engine does not see this
//! machine's folders. `RAVEN_CI_TESTE_DOCKER_REMOTO=1` runs them against it anyway.
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
    let engine = match Engine::find("") {
        Ok(engine) => engine,
        Err(why) => {
            eprintln!("aviso: teste pulado, {why}");
            return None;
        }
    };
    if std::env::var_os("RAVEN_CI_TESTE_DOCKER_REMOTO").is_none() {
        let host = std::env::var("DOCKER_HOST").ok().or_else(|| {
            // Read from the configuration only: asking the engine would already reach the other machine.
            engine.lines(["context", "inspect", "--format", "{{.Endpoints.docker.Host}}"]).into_iter().next()
        });
        if let Some(host) = host.filter(|h| h.starts_with("ssh://") || h.starts_with("tcp://")) {
            eprintln!("aviso: teste pulado, o motor {host} é de outra máquina e não vê as pastas desta");
            return None;
        }
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
}

impl Build {
    /// Ends the build: what it asked to undo at its end runs.
    fn end(&mut self) {
        self.guard.take();
    }
}

impl Drop for Build {
    fn drop(&mut self) {
        self.guard.take();
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

fn build(environment: &[(&str, &str)]) -> Build {
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
    Build { log, workspace, run, guard: Some(guard) }
}

fn container(image: &str, script: &str) -> NodeDef {
    NodeDef::new("passo", CONTAINER)
        .parameter("container", json!({ "image": image }))
        .parameter("script", json!(script))
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
    let mut build = build(&[]);
    let run = run_in(&build, &engine, vec![container("rust:1", "cargo --version")], None);
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
    let build = build(&[]);
    let image = "raven-ci-teste/nao-existe:0";
    let run = run_in(&build, &engine, vec![container(image, "true")], None);
    let part = &run.nodes["passo"];
    assert_eq!(part.status, NodeStatus::Failed);
    let error = part.error.clone().unwrap_or_default();
    assert!(error.starts_with("buscar imagem:") && error.contains(image), "{error}");
}

#[test]
fn a_cancelled_build_leaves_no_container_nor_volume() {
    let Some(engine) = engine() else { return };
    let mut build = build(&[]);
    let token = CancelToken::new();
    let cancel = token.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        cancel.cancel("cancelado pelo teste");
    });
    let began = Instant::now();
    // The anonymous volume stands for what a step leaves on the engine.
    let mut node = container("alpine:3", "sleep 60");
    node.parameters.insert("container".into(), json!({ "image": "alpine:3", "args": ["-v", "/dados"] }));
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
    let mut build = build(&[]);
    let run = run_in(&build, &engine, vec![container("alpine:3", "exit 3")], None);
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
    let build = build(&[("TOKEN", "tok-docker-123456")]);
    let run = run_in(
        &build,
        &engine,
        vec![container("alpine:3", "echo \"$TOKEN\" > feito.txt; id -u; cat /proc/1/cmdline | tr '\\0' ' '")],
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
    let mut build = build(&[]);
    std::fs::write(
        build.workspace.join("compose.yml"),
        "services:\n  web:\n    image: alpine:3\n    command: [\"sh\", \"-c\", \"mkdir -p /w && echo ok > /w/index.html && httpd -f -p 8080 -h /w\"]\n    healthcheck:\n      test: [\"CMD\", \"wget\", \"-q\", \"-O-\", \"http://localhost:8080/\"]\n      interval: 1s\n      retries: 20\n",
    )
    .expect("the compose file");
    let up = NodeDef::new("servicos", COMPOSE).parameter("file", json!("compose.yml"));
    let test = container("alpine:3", "wget -q -O- http://web:8080/");
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
fn an_unhealthy_service_fails_the_wait_with_its_name() {
    let Some(engine) = engine() else { return };
    let mut build = build(&[]);
    std::fs::write(
        build.workspace.join("compose.yml"),
        "services:\n  db:\n    image: alpine:3\n    command: [\"sleep\", \"60\"]\n    healthcheck:\n      test: [\"CMD\", \"false\"]\n      interval: 1s\n      retries: 1\n",
    )
    .expect("the compose file");
    let up = NodeDef::new("servicos", COMPOSE).parameter("file", json!("compose.yml"));
    let run = run_in(&build, &engine, vec![up], None);
    let error = run.nodes["servicos"].error.clone().unwrap_or_default();
    assert!(error.contains("o serviço db ficou unhealthy"), "{error}\n{:?}", build.log.texts());
    build.end();
}
