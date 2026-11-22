//! The step `kubectl` (#892) against a real cluster: a deploy that rolls out, and the messages
//! of one that does not (ImagePullBackOff, OOMKilled, Pending with no resource, 403).
//!
//! They run only when asked, with `RAVEN_CI_TESTES_KUBERNETES=1` and the cluster's context in
//! `RAVEN_CI_TESTE_CONTEXTO`, so a `check.sh` never reaches a cluster on its own. Each test
//! works in a namespace of its own, `raven-ci-teste-<id>`, deleted when it ends, and touches no
//! other.
#![allow(clippy::expect_used, clippy::print_stderr, reason = "tests fail loudly and say why they skip")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use raven_ci_plugin_api::testing::FakeHost;
use raven_ci_plugin_api::{BuildContext, BuildScope, Host, MemoryLog, Permission, ScopeGuard, enter};
use raven_ci_plugin_kubernetes::{KUBECTL, Kubectl, State};
use ravenrs_flows::{Catalog, Executor, FlowDefinition, NodeDef, NodeStatus, Plan, Run};
use ravenrs_process::Spec;
use serde_json::{Value, json};

struct Cluster {
    kubectl: PathBuf,
    context: String,
}

/// The cluster asked for, or `None`, saying why the test is skipped.
fn cluster() -> Option<Cluster> {
    if std::env::var_os("RAVEN_CI_TESTES_KUBERNETES").is_none() {
        eprintln!("aviso: teste pulado, os testes num cluster rodam só com RAVEN_CI_TESTES_KUBERNETES=1");
        return None;
    }
    let Some(context) = std::env::var("RAVEN_CI_TESTE_CONTEXTO").ok().filter(|c| !c.is_empty()) else {
        eprintln!("aviso: teste pulado, falta o contexto do cluster em RAVEN_CI_TESTE_CONTEXTO");
        return None;
    };
    let Some(kubectl) = ravenrs_process::resolve("kubectl") else {
        eprintln!("aviso: teste pulado, não há kubectl no PATH");
        return None;
    };
    Some(Cluster { kubectl, context })
}

impl Cluster {
    /// Runs kubectl with `args` on the cluster, giving its output and whether it ended well.
    fn kubectl(&self, args: &[&str]) -> (bool, String) {
        let output = Spec::new(&self.kubectl)
            .allow_env("KUBECONFIG")
            .arg("--context")
            .arg(&self.context)
            .args(args.iter().copied())
            .timeout(std::time::Duration::from_secs(120))
            .output()
            .expect("kubectl runs");
        (output.end.success(), output.stdout.join("\n") + &output.stderr.join("\n"))
    }
}

struct Build {
    log: Arc<MemoryLog>,
    dir: PathBuf,
    run: String,
    namespace: String,
    cluster: Cluster,
    _guard: ScopeGuard,
}

impl Drop for Build {
    fn drop(&mut self) {
        let _ = self.cluster.kubectl(&["delete", "namespace", &self.namespace, "--wait=false"]);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A build in scope with a namespace of its own on the cluster, and `manifest` in its folder.
fn build(cluster: Cluster, manifest: &str) -> Build {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst));
    let run = format!("kube-cluster-{id}");
    let namespace = format!("raven-ci-teste-{id}");
    let (made, said) = cluster.kubectl(&["create", "namespace", &namespace]);
    assert!(made, "the namespace: {said}");
    let _ = cluster.kubectl(&["label", "namespace", &namespace, &format!("raven-ci-teste={id}")]);
    let dir = std::env::temp_dir().join(&run);
    std::fs::create_dir_all(&dir).expect("a folder");
    std::fs::write(dir.join("site.yml"), manifest).expect("the manifest");
    let log = Arc::new(MemoryLog::new());
    let context = BuildContext { job: "site".into(), number: 1, workspace: dir.clone(), run: run.clone() };
    let guard = enter(BuildScope::new(context, log.clone(), BTreeMap::new()));
    Build { log, dir, run, namespace, cluster, _guard: guard }
}

fn run(build: &Build, mut parameters: Value, credentials: &[(&str, &str)], timeout_ms: u64) -> Run {
    let mut host = FakeHost::new("kubernetes", &[Permission::Process, Permission::Secrets]);
    for (name, value) in credentials {
        host = host.with_secret(name, value);
    }
    let host: Arc<dyn Host> = Arc::new(host);
    let mut catalog = Catalog::builtin();
    catalog.add(Kubectl(State::with(build.cluster.kubectl.clone(), Some(host))));
    parameters["namespace"] = json!(build.namespace);
    if parameters.get("context").is_none() {
        parameters["context"] = json!(build.cluster.context);
    }
    let mut node = NodeDef::new("deploy", KUBECTL);
    for (key, value) in parameters.as_object().expect("an object") {
        node = node.parameter(key.clone(), value.clone());
    }
    node.timeout_ms = Some(timeout_ms);
    let flow = FlowDefinition::new("site", "site")
        .node(NodeDef::new("start", "manual"))
        .node(node)
        .connect("start", "main", "deploy", "main");
    let plan = Plan::compile(&flow, &catalog).expect("compiles");
    Executor::new(&plan, &catalog).run_id(build.run.clone()).run()
}

/// A deployment `site` of one pod with `container` (YAML of the container, six spaces in).
fn deployment(container: &str) -> String {
    format!(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: site\nspec:\n  replicas: 1\n  selector:\n    matchLabels:\n      app: site\n  template:\n    metadata:\n      labels:\n        app: site\n    spec:\n      terminationGracePeriodSeconds: 0\n      containers:\n{container}"
    )
}

fn deploy() -> Value {
    json!({ "apply": "site.yml", "rollout": "deployment/site" })
}

fn error(run: &Run) -> String {
    run.nodes["deploy"].error.clone().unwrap_or_default()
}

#[test]
fn a_deploy_rolls_out_in_its_namespace() {
    let Some(cluster) = cluster() else { return };
    let build = build(
        cluster,
        &deployment(
            "      - name: site\n        image: busybox:1.36\n        command: [\"sleep\", \"3600\"]\n",
        ),
    );
    let run = run(&build, deploy(), &[], 120_000);
    assert_eq!(run.nodes["deploy"].status, NodeStatus::Done, "{}\n{:?}", error(&run), build.log.texts());
    assert!(
        build.log.texts().iter().any(|t| t.contains("successfully rolled out")),
        "{:?}",
        build.log.texts()
    );
}

#[test]
fn an_image_that_does_not_exist_is_said_by_its_name() {
    let Some(cluster) = cluster() else { return };
    let build = build(
        cluster,
        &deployment("      - name: site\n        image: ghcr.io/raven-ci-teste/nao-existe:0\n"),
    );
    let run = run(&build, deploy(), &[], 45_000);
    let error = error(&run);
    assert!(error.contains("a imagem ghcr.io/raven-ci-teste/nao-existe:0 do pod site-"), "{error}");
    assert!(error.contains("não pôde ser baixada"), "{error}");
}

#[test]
fn a_container_out_of_memory_says_oomkilled_and_its_limit() {
    let Some(cluster) = cluster() else { return };
    // A pod is ready before it is killed: minReadySeconds makes the rollout wait past that.
    let manifest = deployment(
        "      - name: site\n        image: busybox:1.36\n        command: [\"sh\", \"-c\", \"a=$(head -c 64m /dev/zero | tr '\\\\000' x); sleep 3600\"]\n        resources:\n          limits:\n            memory: 16Mi\n",
    )
    .replace("  replicas: 1\n", "  replicas: 1\n  minReadySeconds: 20\n");
    let build = build(cluster, &manifest);
    let run = run(&build, deploy(), &[], 60_000);
    let error = error(&run);
    assert!(error.contains("OOMKilled), com o limite de 16Mi"), "{error}");
}

#[test]
fn a_pod_no_node_can_hold_is_no_resource_in_the_cluster() {
    let Some(cluster) = cluster() else { return };
    let build = build(
        cluster,
        &deployment(
            "      - name: site\n        image: busybox:1.36\n        command: [\"sleep\", \"3600\"]\n        resources:\n          requests:\n            cpu: \"500\"\n",
        ),
    );
    let run = run(&build, deploy(), &[], 20_000);
    let error = error(&run);
    assert!(error.contains("sem recurso no cluster: o pod site-"), "{error}");
}

#[test]
fn a_kubeconfig_without_permission_is_a_403_explained() {
    let Some(cluster) = cluster() else { return };
    let build = build(cluster, &deployment("      - name: site\n        image: busybox:1.36\n"));
    let ns = build.namespace.clone();
    let (made, said) = build.cluster.kubectl(&["-n", &ns, "create", "serviceaccount", "leitor"]);
    assert!(made, "{said}");
    let (made, token) = build.cluster.kubectl(&["-n", &ns, "create", "token", "leitor", "--duration=10m"]);
    assert!(made, "{token}");
    let (_, view) = build.cluster.kubectl(&["config", "view", "--minify", "--raw", "-o", "json"]);
    let view: Value = serde_json::from_str(&view).expect("the configuration");
    let server = view["clusters"][0]["cluster"]["server"].as_str().expect("the server");
    let ca = view["clusters"][0]["cluster"]["certificate-authority-data"].as_str().unwrap_or_default();
    let kubeconfig = format!(
        "apiVersion: v1\nkind: Config\nclusters:\n- name: c\n  cluster:\n    server: {server}\n    certificate-authority-data: {ca}\nusers:\n- name: leitor\n  user:\n    token: {}\ncontexts:\n- name: leitor\n  context:\n    cluster: c\n    user: leitor\ncurrent-context: leitor\n",
        token.trim()
    );
    let run = run(
        &build,
        json!({ "apply": "site.yml", "context": "leitor", "credential": "kube-leitor" }),
        &[("kube-leitor", &kubeconfig)],
        60_000,
    );
    let error = error(&run);
    assert!(error.starts_with("o kubeconfig não tem permissão (403)"), "{error}");
    assert!(error.contains(&format!("system:serviceaccount:{ns}:leitor")), "{error}");
    assert!(!build.log.texts().join("\n").contains(token.trim()), "the token reached the log");
}

/// A request for an agent in a pod of the test's namespace, whose agent is a stand-in until
/// #893: busybox saying what it got and waiting.
fn agent_request(build: &Build, extra: Value) -> raven_ci_plugin_api::LaunchRequest {
    let mut settings = json!({
        "image": "busybox:1.36",
        "namespace": build.namespace,
        "context": build.cluster.context,
        "command": ["sh", "-c", "echo \"agente falso: $* token=$RAVEN_CI_AGENT_TOKEN\"; exec sleep 300", "raven-ci-agent"],
        "pending_timeout_s": 60,
    });
    if let Value::Object(more) = extra {
        for (k, v) in more {
            settings[k] = v;
        }
    }
    raven_ci_plugin_api::LaunchRequest {
        build: Some("site#1".into()),
        labels: vec!["build:site#1".into()],
        settings,
        server: "https://ci.casa".into(),
        token: "tok-uso-unico".into(),
    }
}

fn launcher(build: &Build) -> raven_ci_plugin_kubernetes::KubernetesLauncher {
    raven_ci_plugin_kubernetes::KubernetesLauncher(State::with(build.cluster.kubectl.clone(), None))
}

#[test]
fn an_agent_comes_up_in_a_pod_and_its_pod_goes_when_it_stops() {
    use raven_ci_plugin_api::AgentLauncher;
    let Some(cluster) = cluster() else { return };
    let build = build(cluster, "");
    let agent = launcher(&build).launch(&agent_request(&build, json!({}))).expect("it comes up");
    let name = agent.name();
    assert!(agent.alive(), "the pod {name} is not running");
    let (_, said) = build.cluster.kubectl(&["-n", &build.namespace, "logs", &name, "-c", "agente"]);
    assert!(said.contains(&format!("--nome {name} --rotulos build:site#1")), "{said}");
    assert!(said.contains("token=tok-uso-unico"), "the token came by the Secret: {said}");
    let (_, spec) = build.cluster.kubectl(&["-n", &build.namespace, "get", "pod", &name, "-o", "json"]);
    assert!(!spec.contains("tok-uso-unico"), "the token is not in the pod's spec");
    agent.stop();
    let label = format!("raven-ci.agent={name}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let (_, left) =
            build.cluster.kubectl(&["-n", &build.namespace, "get", "pods", "-l", &label, "-o", "name"]);
        if left.trim().is_empty() || left.contains("No resources found") {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the pod outlived stop: {left}");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    let (_, secrets) =
        build.cluster.kubectl(&["-n", &build.namespace, "get", "secrets", "-l", &label, "-o", "name"]);
    assert!(!secrets.contains("secret/"), "the token's Secret outlived stop: {secrets}");
}

#[test]
fn a_pod_of_an_agent_no_node_takes_is_deleted_with_no_resource_in_the_cluster() {
    use raven_ci_plugin_api::AgentLauncher;
    let Some(cluster) = cluster() else { return };
    let build = build(cluster, "");
    let request = agent_request(
        &build,
        json!({ "resources": { "requests": { "cpu": "500" } }, "pending_timeout_s": 8 }),
    );
    let refused = match launcher(&build).launch(&request) {
        Ok(agent) => panic!("the agent {} came up", agent.name()),
        Err(why) => why.to_string(),
    };
    assert!(refused.contains("sem recurso no cluster") && refused.contains("foi apagado"), "{refused}");
    let (_, left) =
        build.cluster.kubectl(&["-n", &build.namespace, "get", "pods", "-l", "raven-ci.agent", "-o", "name"]);
    assert!(!left.contains("pod/"), "{left}");
}
