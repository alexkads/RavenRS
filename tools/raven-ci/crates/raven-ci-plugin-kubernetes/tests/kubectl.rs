//! The step `kubectl` (#892) against `kubectl-falso`, a kubectl written in Rust that answers what
//! each test tells it, so the tests run on Linux, macOS and Windows with no cluster: the
//! arguments it gets, the kubeconfig of a moment, and the messages of a stuck rollout.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use raven_ci_plugin_api::testing::FakeHost;
use raven_ci_plugin_api::{BuildContext, BuildScope, Host, MemoryLog, Permission, ScopeGuard, enter};
use raven_ci_plugin_kubernetes::{KUBECTL, Kubectl, State};
use ravenrs_flows::{Catalog, Executor, FlowDefinition, NodeDef, NodeStatus, Plan, Run};
use serde_json::{Value, json};

/// A kubeconfig whose token must never reach the log.
const KUBECONFIG: &str =
    "apiVersion: v1\nkind: Config\nusers:\n- name: ci\n  user:\n    token: tok-kube-0123456789\n";

struct Build {
    log: Arc<MemoryLog>,
    dir: PathBuf,
    run: String,
    _guard: ScopeGuard,
}

impl Drop for Build {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Build {
    /// What kubectl-falso was called with, one line per call.
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("falso").join("chamadas.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

/// A build in scope whose kubectl-falso answers `answers`.
fn build(answers: Value) -> Build {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let run = format!("kube-teste-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst));
    let dir = std::env::temp_dir().join(&run);
    let fake = dir.join("falso");
    std::fs::create_dir_all(&fake).expect("a folder");
    std::fs::create_dir_all(dir.join("ws")).expect("a folder");
    std::fs::write(fake.join("respostas.json"), answers.to_string()).expect("the answers");
    let log = Arc::new(MemoryLog::new());
    let environment = BTreeMap::from([("KUBECTL_FALSO".to_owned(), fake.display().to_string())]);
    let context = BuildContext { job: "site".into(), number: 1, workspace: dir.join("ws"), run: run.clone() };
    let guard = enter(BuildScope::new(context, log.clone(), environment));
    Build { log, dir, run, _guard: guard }
}

fn compile(parameters: &Value) -> (FlowDefinition, Catalog) {
    let host: Arc<dyn Host> = Arc::new(
        FakeHost::new("kubernetes", &[Permission::Process, Permission::Secrets])
            .with_secret("kube", KUBECONFIG),
    );
    let state = State::with(PathBuf::from(env!("CARGO_BIN_EXE_kubectl-falso")), Some(host));
    let mut catalog = Catalog::builtin();
    catalog.add(Kubectl(state));
    let mut node = NodeDef::new("deploy", KUBECTL);
    for (key, value) in parameters.as_object().expect("an object") {
        node = node.parameter(key.clone(), value.clone());
    }
    let flow = FlowDefinition::new("site", "site")
        .node(NodeDef::new("start", "manual"))
        .node(node)
        .connect("start", "main", "deploy", "main");
    (flow, catalog)
}

fn run(build: &Build, parameters: Value) -> Run {
    let (flow, catalog) = compile(&parameters);
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    Executor::new(&plan, &catalog).run_id(build.run.clone()).run()
}

fn deploy() -> Value {
    json!({
        "namespace": "prod",
        "context": "k3s-teste",
        "credential": "kube",
        "apply": "k8s/",
        "rollout": "deployment/site",
    })
}

/// The answers of a rollout that does not end, whose pods are `pods`.
fn stuck(pods: Value) -> Value {
    json!([
        { "se": "rollout status", "erro": "error: timed out waiting for the condition", "codigo": 1 },
        { "se": "get deployment/site -o json", "saida": json!({ "spec": { "selector": { "matchLabels": { "app": "site" } } } }).to_string() },
        { "se": "get pods -l app=site -o json", "saida": json!({ "items": pods }).to_string() },
    ])
}

fn error(run: &Run) -> String {
    run.nodes["deploy"].error.clone().unwrap_or_default()
}

#[test]
fn a_deploy_applies_and_waits_with_the_cluster_declared_and_the_kubeconfig_of_a_moment() {
    let build = build(json!([
        { "se": "apply -f k8s/", "saida": "deployment.apps/site configured" },
        { "se": "rollout status", "saida": "deployment \"site\" successfully rolled out" },
    ]));
    let run = run(&build, deploy());
    assert_eq!(run.nodes["deploy"].status, NodeStatus::Done, "{:?}", run.nodes["deploy"].error);
    let calls = build.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    for call in &calls {
        assert!(call.contains("--context k3s-teste --namespace prod"), "{call}");
        assert!(call.starts_with("--kubeconfig "), "{call}");
    }
    assert!(calls[0].ends_with("apply -f k8s/"), "{calls:?}");
    assert!(calls[1].contains("rollout status deployment/site --timeout=300s"), "{calls:?}");
    let texts = build.log.texts().join("\n");
    assert!(texts.contains("deployment.apps/site configured"), "{texts}");
    assert!(texts.contains("successfully rolled out"), "{texts}");
    assert!(!texts.contains("tok-kube-0123456789"), "{texts}");
    let kubeconfig = calls[0].split_whitespace().nth(1).expect("the path");
    assert!(!std::path::Path::new(kubeconfig).exists(), "the kubeconfig {kubeconfig} outlived the step");
}

#[test]
fn a_step_without_a_namespace_is_refused_before_kubectl_runs() {
    let build = build(json!([]));
    let mut parameters = deploy();
    parameters.as_object_mut().expect("an object").remove("namespace");
    let (flow, catalog) = compile(&parameters);
    let refused = Plan::compile(&flow, &catalog).expect_err("refused before the build runs");
    assert!(format!("{refused:?}").contains("namespace"), "{refused:?}");
    // And the step itself refuses an empty one, with the reason.
    parameters["namespace"] = json!(" ");
    let run = run(&build, parameters);
    assert_eq!(run.nodes["deploy"].status, NodeStatus::Failed);
    assert!(error(&run).contains("precisa de namespace"), "{}", error(&run));
    assert!(build.calls().is_empty(), "{:?}", build.calls());
}

#[test]
fn an_image_that_does_not_download_is_said_by_its_name() {
    let build = build(stuck(json!([{
        "metadata": { "name": "site-7d9f" },
        "status": { "phase": "Pending", "containerStatuses": [{
            "name": "site", "image": "ghcr.io/eu/site:nao-existe",
            "state": { "waiting": { "reason": "ImagePullBackOff", "message": "Back-off pulling image" } }
        }]}
    }])));
    let run = run(&build, deploy());
    assert_eq!(run.nodes["deploy"].status, NodeStatus::Failed);
    let error = error(&run);
    assert!(
        error.contains(
            "a imagem ghcr.io/eu/site:nao-existe do pod site-7d9f não pôde ser baixada (ImagePullBackOff)"
        ),
        "{error}"
    );
}

#[test]
fn a_container_killed_for_memory_says_its_limit() {
    let build = build(stuck(json!([{
        "metadata": { "name": "site-1" },
        "spec": { "containers": [{ "name": "site", "resources": { "limits": { "memory": "64Mi" } } }] },
        "status": { "phase": "Running", "containerStatuses": [{
            "name": "site", "image": "site:1",
            "state": { "waiting": { "reason": "CrashLoopBackOff" } },
            "lastState": { "terminated": { "reason": "OOMKilled", "exitCode": 137 } }
        }]}
    }])));
    let run = run(&build, deploy());
    let error = error(&run);
    assert!(error.contains("foi morto por falta de memória (OOMKilled), com o limite de 64Mi"), "{error}");
}

#[test]
fn a_pod_no_node_accepts_is_no_resource_in_the_cluster() {
    let build = build(stuck(json!([{
        "metadata": { "name": "site-2" },
        "status": { "phase": "Pending", "conditions": [{
            "type": "PodScheduled", "status": "False", "reason": "Unschedulable",
            "message": "0/1 nodes are available: 1 Insufficient cpu."
        }]}
    }])));
    let run = run(&build, deploy());
    let error = error(&run);
    assert!(error.contains("sem recurso no cluster: o pod site-2 está em Pending"), "{error}");
    assert!(error.contains("Insufficient cpu"), "{error}");
}

#[test]
fn a_kubeconfig_without_permission_is_a_403_explained() {
    let build = build(json!([{
        "se": "apply",
        "erro": "Error from server (Forbidden): error when creating \"k8s/site.yml\": deployments.apps is forbidden: User \"system:serviceaccount:ci:leitor\" cannot create resource \"deployments\" in API group \"apps\" in the namespace \"prod\"",
        "codigo": 1
    }]));
    let run = run(&build, deploy());
    let error = error(&run);
    assert!(
        error.contains("o kubeconfig não tem permissão (403): o usuário system:serviceaccount:ci:leitor não pode create deployments no namespace prod"),
        "{error}"
    );
}
