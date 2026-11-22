//! The launcher `kubernetes` (#892) against `kubectl-falso`: the pod made from the job's model,
//! the limit of pods in the cluster, a pod no node takes deleted with "sem recurso no cluster",
//! and `stop` deleting the pod, on Linux, macOS and Windows with no cluster.
#![allow(clippy::expect_used, reason = "tests fail loudly")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use raven_ci_plugin_api::{AgentLauncher, CiError, LaunchRequest, LaunchedAgent};
use raven_ci_plugin_kubernetes::{KubernetesLauncher, State};
use serde_json::{Value, json};

struct Fake {
    dir: PathBuf,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Fake {
    fn new(answers: Value) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "kube-agente-teste-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).expect("a folder");
        std::fs::write(dir.join("respostas.json"), answers.to_string()).expect("the answers");
        Self { dir }
    }

    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("chamadas.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn launcher(&self) -> KubernetesLauncher {
        KubernetesLauncher(State::with(PathBuf::from(env!("CARGO_BIN_EXE_kubectl-falso")), None))
    }

    /// A request whose context is the fake's folder: the launcher's kubectl gets only the
    /// server's variables, so kubectl-falso finds its answers by `--context`.
    fn request(&self, settings: Value) -> LaunchRequest {
        let mut settings = settings;
        settings["image"] = json!("ghcr.io/eu/agente:1");
        settings["namespace"] = json!("ci");
        settings["context"] = json!(self.dir.display().to_string());
        LaunchRequest {
            build: Some("apps/site#12".into()),
            labels: vec!["build:apps/site#12".into()],
            settings,
            server: "https://ci.casa".into(),
            token: "tok-uso-unico".into(),
        }
    }
}

/// The refusal of a launch that had to fail.
fn refused(launched: Result<Box<dyn LaunchedAgent>, CiError>) -> CiError {
    match launched {
        Ok(agent) => panic!("the agent {} came up", agent.name()),
        Err(why) => why,
    }
}

fn pod(phase: &str, extra: Value) -> String {
    let mut status = json!({ "phase": phase });
    if let Value::Object(more) = extra {
        for (k, v) in more {
            status[k] = v;
        }
    }
    json!({ "metadata": { "name": "p" }, "status": status }).to_string()
}

#[test]
fn an_agent_comes_up_in_a_pod_and_stop_deletes_the_pod() {
    let fake = Fake::new(json!([
        { "se": "get pods -A", "saida": "" },
        { "se": "get pod raven-ci-agente", "saida": pod("Running", json!({})) },
    ]));
    let agent = fake.launcher().launch(&fake.request(json!({}))).expect("it comes up");
    let name = agent.name();
    assert!(name.starts_with("raven-ci-agente-apps-site-12-"), "{name}");
    assert!(agent.alive());
    agent.stop();
    let calls = fake.calls();
    assert!(
        calls.iter().any(|c| c.contains("--namespace ci create -f ") && c.ends_with("pod.json")),
        "{calls:?}"
    );
    assert!(calls.iter().any(|c| c.contains(&format!("delete pod {name} --ignore-not-found"))), "{calls:?}");
    assert!(calls.iter().any(|c| c.contains(&format!("delete secret {name}-token"))), "{calls:?}");
}

#[test]
fn the_limit_of_pods_in_the_cluster_holds_the_build_back() {
    let fake = Fake::new(json!([{ "se": "get pods -A", "saida": "pod/a\npod/b" }]));
    let refused = refused(fake.launcher().launch(&fake.request(json!({ "max_pods": 2 }))));
    assert!(
        refused.to_string().contains("o limite de 2 pods de agente no cluster foi atingido (2 rodando)"),
        "{refused}"
    );
    assert!(!fake.calls().iter().any(|c| c.contains("create")), "{:?}", fake.calls());
}

#[test]
fn a_pod_no_node_takes_is_deleted_with_no_resource_in_the_cluster() {
    let fake = Fake::new(json!([
        { "se": "get pods -A", "saida": "" },
        { "se": "get pod raven-ci-agente", "saida": pod("Pending", json!({ "conditions": [{
            "type": "PodScheduled", "status": "False", "reason": "Unschedulable",
            "message": "0/1 nodes are available: 1 Insufficient memory."
        }]})) },
    ]));
    let refused = refused(fake.launcher().launch(&fake.request(json!({ "pending_timeout_s": 1 }))));
    let said = refused.to_string();
    assert!(said.contains("sem recurso no cluster") && said.contains("Insufficient memory"), "{said}");
    assert!(said.contains("foi apagado"), "{said}");
    assert!(fake.calls().iter().any(|c| c.contains("delete pod raven-ci-agente")), "{:?}", fake.calls());
}

#[test]
fn an_image_that_does_not_come_ends_the_launch_at_once() {
    let fake = Fake::new(json!([
        { "se": "get pods -A", "saida": "" },
        { "se": "get pod raven-ci-agente", "saida": pod("Pending", json!({ "containerStatuses": [{
            "name": "agente", "image": "ghcr.io/eu/agente:1",
            "state": { "waiting": { "reason": "ErrImagePull" } }
        }]})) },
    ]));
    let refused = refused(fake.launcher().launch(&fake.request(json!({}))));
    assert!(refused.to_string().contains("a imagem ghcr.io/eu/agente:1"), "{refused}");
    assert!(fake.calls().iter().any(|c| c.contains("delete pod")), "{:?}", fake.calls());
}

#[test]
fn a_kubeconfig_that_cannot_create_pods_is_a_403_explained() {
    let fake = Fake::new(json!([
        { "se": "get pods -A", "saida": "" },
        { "se": "create -f", "erro": "Error from server (Forbidden): error when creating \"pod.json\": pods is forbidden: User \"system:serviceaccount:ci:leitor\" cannot create resource \"pods\" in API group \"\" in the namespace \"ci\"", "codigo": 1 },
    ]));
    let refused = refused(fake.launcher().launch(&fake.request(json!({}))));
    assert!(
        refused.to_string().contains("o kubeconfig não tem permissão (403): o usuário system:serviceaccount:ci:leitor não pode create pods no namespace ci"),
        "{refused}"
    );
}
