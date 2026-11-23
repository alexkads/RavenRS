//! The step `sh` (#863): each line in the build's log, the code it ends with, cancelling, the
//! secrets hidden and the folder of the build. Each script is said for `sh` and for `cmd`, so
//! the tests run on Linux, macOS and Windows.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use raven_ci_plugin_api::{BuildContext, BuildLog, BuildScope, MemoryLog, enter};
use raven_ci_plugin_shell::{SH, Sh};
use ravenrs_flows::{CancelToken, Catalog, Executor, FlowDefinition, NodeDef, NodeStatus, Plan, Run};
use serde_json::json;

/// `unix` on Linux and macOS, `windows` on Windows.
fn script(unix: &str, windows: &str) -> String {
    if cfg!(windows) { windows.to_owned() } else { unix.to_owned() }
}

struct Build {
    log: Arc<MemoryLog>,
    workspace: PathBuf,
    run: String,
    _guard: raven_ci_plugin_api::ScopeGuard,
}

impl Drop for Build {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

/// A build in scope, with a folder of its own and `environment`.
fn build(environment: &[(&str, &str)]) -> Build {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let run = format!("sh-teste-{}-{n}", std::process::id());
    let workspace = std::env::temp_dir().join(&run);
    std::fs::create_dir_all(&workspace).expect("a folder");
    let log = Arc::new(MemoryLog::new());
    let environment: BTreeMap<String, String> =
        environment.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    let context =
        BuildContext { job: "site".into(), number: 1, workspace: workspace.clone(), run: run.clone() };
    let guard = enter(BuildScope::new(context, log.clone(), environment));
    Build { log, workspace, run, _guard: guard }
}

fn flow(node: NodeDef) -> FlowDefinition {
    FlowDefinition::new("site", "site")
        .node(NodeDef::new("start", "manual"))
        .node(node)
        .connect("start", "main", "sh", "main")
}

fn run_in(build: &Build, node: NodeDef, token: Option<CancelToken>) -> Run {
    let mut catalog = Catalog::builtin();
    catalog.add(Sh::default());
    let flow = flow(node);
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let mut executor = Executor::new(&plan, &catalog).run_id(build.run.clone());
    if let Some(token) = token {
        executor = executor.cancel_cooperatively(token);
    }
    executor.run()
}

fn sh(script: &str) -> NodeDef {
    NodeDef::new("sh", SH).parameter("script", json!(script))
}

#[test]
fn each_line_of_the_output_and_of_the_errors_goes_to_the_log() {
    let build = build(&[]);
    let run = run_in(&build, sh(&script("echo um; echo dois >&2", "echo um & echo dois 1>&2")), None);
    assert_eq!(run.nodes["sh"].status, NodeStatus::Done, "{:?}", run.nodes["sh"].error);
    let texts: Vec<String> = build.log.texts().into_iter().map(|t| t.trim().to_owned()).collect();
    assert!(texts.contains(&"um".to_owned()) && texts.contains(&"dois".to_owned()), "{texts:?}");
    assert!(build.log.lines().iter().all(|(step, _)| step == "sh"));
}

#[test]
fn a_command_that_ends_with_three_fails_saying_the_code() {
    let build = build(&[]);
    let run = run_in(&build, sh(&script("exit 3", "exit /b 3")), None);
    let part = &run.nodes["sh"];
    assert_eq!(part.status, NodeStatus::Failed);
    assert!(part.error.as_deref().unwrap_or_default().contains("o código 3"), "{part:?}");
}

#[test]
fn a_cancelled_build_stops_its_command_in_well_under_five_seconds() {
    let build = build(&[]);
    let token = CancelToken::new();
    let cancel = token.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        cancel.cancel("cancelado pelo teste");
    });
    let began = Instant::now();
    let run = run_in(&build, sh(&script("sleep 30", "ping -n 30 127.0.0.1 >nul")), Some(token));
    canceller.join().expect("joined");
    assert!(began.elapsed() < Duration::from_secs(5), "took {:?}", began.elapsed());
    assert_eq!(run.nodes["sh"].status, NodeStatus::Cancelled, "{:?}", run.nodes["sh"]);
    assert!(build.log.texts().iter().any(|t| t.starts_with("parando o comando")));
}

#[test]
fn a_command_past_its_time_limit_is_stopped() {
    let build = build(&[]);
    let mut node = sh(&script("sleep 30", "ping -n 30 127.0.0.1 >nul"));
    node.timeout_ms = Some(300);
    let began = Instant::now();
    let run = run_in(&build, node, None);
    assert!(began.elapsed() < Duration::from_secs(5), "took {:?}", began.elapsed());
    assert_eq!(run.nodes["sh"].status, NodeStatus::TimedOut, "{:?}", run.nodes["sh"]);
}

#[test]
fn a_secret_the_script_prints_is_hidden_in_the_log() {
    let build = build(&[("TOKEN", "tok-123456")]);
    build.log.hide("tok-123456");
    let run = run_in(&build, sh(&script("echo \"o token: $TOKEN\"", "echo o token: %TOKEN%")), None);
    assert_eq!(run.nodes["sh"].status, NodeStatus::Done, "{:?}", run.nodes["sh"].error);
    let all = build.log.texts().join("\n");
    assert!(all.contains("o token: ****"), "{all}");
    assert!(!all.contains("tok-123456"), "{all}");
}

#[test]
fn the_command_runs_in_the_folder_of_the_build_with_its_variables() {
    let build = build(&[("BUILD_NUMBER", "1")]);
    let run =
        run_in(&build, sh(&script("pwd; echo \"n=$BUILD_NUMBER\"", "cd & echo n=%BUILD_NUMBER%")), None);
    assert_eq!(run.nodes["sh"].status, NodeStatus::Done, "{:?}", run.nodes["sh"].error);
    let texts: Vec<String> = build.log.texts().into_iter().map(|t| t.trim().to_owned()).collect();
    let folder = std::fs::canonicalize(&build.workspace).expect("the folder");
    let said = texts.iter().any(|t| std::fs::canonicalize(t).is_ok_and(|seen| seen == folder));
    assert!(said, "the folder {} in {texts:?}", folder.display());
    assert!(texts.contains(&"n=1".to_owned()), "{texts:?}");
}

#[test]
fn the_server_environment_does_not_reach_the_command() {
    // Cargo gives the tests this variable; the server's environment stands for it here.
    let Ok(leaked) = std::env::var("CARGO_MANIFEST_DIR") else { return };
    let build = build(&[]);
    let run = run_in(
        &build,
        sh(&script("echo \"v=${CARGO_MANIFEST_DIR:-}\"", "echo v=%CARGO_MANIFEST_DIR%")),
        None,
    );
    assert_eq!(run.nodes["sh"].status, NodeStatus::Done, "{:?}", run.nodes["sh"].error);
    assert!(!build.log.texts().join("\n").contains(&leaked), "{:?}", build.log.texts());
}

#[test]
fn a_step_outside_a_build_fails_saying_so() {
    let mut catalog = Catalog::builtin();
    catalog.add(Sh::default());
    let flow = flow(sh("echo oi"));
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    let run = Executor::new(&plan, &catalog).run();
    assert!(run.nodes["sh"].error.as_deref().unwrap_or_default().contains("fora de um build"));
}

#[test]
#[cfg(unix)]
fn nice_runs_the_command_with_less_priority() {
    let build = build(&[]);
    let node = sh("ps -o nice= -p $$").parameter("nice", json!(7));
    let run = run_in(&build, node, None);
    assert_eq!(run.nodes["sh"].status, NodeStatus::Done, "{:?}", run.nodes["sh"].error);
    assert_eq!(build.log.texts().first().map(|t| t.trim().to_owned()).as_deref(), Some("7"));
}
